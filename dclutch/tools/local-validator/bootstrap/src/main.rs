//! Keyless localhost bootstrap for the real committed Pyth provider ELFs.

use std::{
    collections::BTreeMap, env, error::Error, fmt, fs::OpenOptions, io::Write, net::IpAddr,
    path::PathBuf, str::FromStr, thread, time::Duration,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_pyth_svm::{FullPriceUpdateV2, PostUpdateParamsView};
use dclutch_rent_contract::{
    CreateRentCreditV1, RENT_CREDIT_BYTES_V1, RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority,
    WithdrawRentCreditV1,
};
use reqwest::{Url, blocking::Client, redirect::Policy};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_sdk::{
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};

mod source;

const RECEIVER_ID: &str = "rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp";
const ROUTER_ID: &str = "HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL";
const RECEIVER_PROGRAMDATA: &str = "3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX";
const ROUTER_PROGRAMDATA: &str = "9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x";
const UPGRADEABLE_LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
const CLOCK_SYSVAR: &str = "SysvarC1ock11111111111111111111111111111111";
const RENT_SYSVAR: &str = "SysvarRent111111111111111111111111111111111";
const COMPUTE_BUDGET_PROGRAM: &str = "ComputeBudget111111111111111111111111111111";
const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const TOKEN_2022_PROGRAM: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
const DCLUTCH_PROGRAM: &str = "5oEzAP4izB65uRm2yDAEf9oALGwHpWkDfyKb8zBY3euC";
const ENCODED_VAA_HEADER_BYTES: usize = 46;
const FULL_PRICE_UPDATE_BYTES: usize = 134;
const WRITE_CHUNK_BYTES: usize = 600;
const FIXTURE_PUBLISH_TIME: i64 = 1_787_431_680;
const AIRDROP_LAMPORTS: u64 = 10_000_000_000;
const RENT_CREDIT_DEPOSIT_LAMPORTS: u64 = 1_000_000;
const RENT_CREDIT_WITHDRAW_LAMPORTS: u64 = 400_000;

const PROVENANCE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/PROVENANCE.md");
const UPSTREAM_LICENSE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/UPSTREAM_LICENSE");
const RECEIVER_ELF: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver.so");
const ROUTER_ELF: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/router.so");
const ROUTER_INITIALIZE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/router-initialize.data");
const RECEIVER_INITIALIZE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-initialize.data");
const RECEIVER_CONFIG: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-config.account");
const SIGNED_VAA: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/signed.vaa");
const RECEIVER_POST_UPDATE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data");
const PRICE_UPDATE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/price-update.account");

type AnyResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
struct BootstrapError(String);

impl fmt::Display for BootstrapError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for BootstrapError {}

fn fail<T>(message: impl Into<String>) -> AnyResult<T> {
    Err(Box::new(BootstrapError(message.into())))
}

#[derive(Debug)]
struct Args {
    rpc_url: Url,
    evidence: Option<PathBuf>,
    reclaim: bool,
    dclutch: Option<DclutchPin>,
}

#[derive(Debug)]
struct DclutchPin {
    program_id: Pubkey,
    elf_sha256: String,
    source_commit: String,
    source_archive_sha256: String,
}

#[derive(Clone, Debug)]
struct RpcAccount {
    lamports: u64,
    owner: Pubkey,
    executable: bool,
    rent_epoch: u64,
    data: Vec<u8>,
}

struct ProgramPin {
    name: &'static str,
    program_id: Pubkey,
    programdata_id: Pubkey,
    elf: &'static [u8],
    elf_sha256: &'static str,
    captured_program_sha256: &'static str,
    captured_programdata_sha256: &'static str,
}

#[derive(Serialize)]
struct Evidence {
    schema: &'static str,
    evidence_class: &'static str,
    rpc_url: String,
    genesis_hash: String,
    validator_version: Value,
    fixture: FixtureEvidence,
    loader_time_boundary: LoaderTimeBoundary,
    programs: Vec<ProgramEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dclutch_program: Option<RuntimeProgramEvidence>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    token_programs: Vec<RuntimeProgramEvidence>,
    provider_accounts: BTreeMap<String, AccountEvidence>,
    semantic_checks: SemanticEvidence,
    transactions: Vec<TransactionEvidence>,
    payer: String,
    encoded_vaa: String,
    price_update: String,
    price_update_reclaimed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    dclutch_lifecycle: Option<DclutchLifecycleEvidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dclutch_source: Option<source::SourceBootstrapEvidence>,
    provider_state_initialized: bool,
    captured_release_identity_claimed: bool,
    dclutch_lifecycle_executed: bool,
    dclutch_resolution_executed: bool,
}

#[derive(Serialize)]
struct RuntimeProgramEvidence {
    name: &'static str,
    program_id: String,
    programdata_id: String,
    canonical_programdata_pda: bool,
    program: AccountEvidence,
    programdata: AccountEvidence,
    observed_deployment_slot: u64,
    observed_upgrade_authority: Option<String>,
    observed_upgrade_authority_effectively_disabled: bool,
    elf_tail_sha256: String,
    expected_elf_tail_sha256: Option<String>,
    elf_tail_matches_expected: Option<bool>,
}

#[derive(Serialize)]
struct DclutchLifecycleEvidence {
    source_commit: String,
    source_archive_sha256: String,
    lifecycle: &'static str,
    authority: String,
    rent_credit: String,
    pda_bump: u8,
    rent_floor_lamports: u64,
    deposited_lamports: u64,
    withdrawn_lamports: u64,
    final_surplus_lamports: u64,
    exact_state_matches_contract: bool,
    after_create: AccountEvidence,
    after_fund: AccountEvidence,
    after_withdraw: AccountEvidence,
    resolution_executed: bool,
    resolution_boundary: &'static str,
}

#[derive(Serialize)]
struct FixtureEvidence {
    source_repository: &'static str,
    source_commit: &'static str,
    source_license: &'static str,
    captured_network: &'static str,
    synthetic_signed_vaa: bool,
    hashes_sha256: BTreeMap<&'static str, String>,
}

#[derive(Serialize)]
struct LoaderTimeBoundary {
    profile_class: &'static str,
    release_bound_provider_loader_evidence: bool,
    captured_receiver_deployment_slot: u64,
    captured_router_deployment_slot: u64,
    observed_validator_slot: u64,
    observed_validator_clock_unix_timestamp: i64,
    fixture_publish_time: i64,
    fixture_publish_time_matches_validator_clock: bool,
    fixture_clock_delta_seconds: u64,
    fixture_within_reference_60_second_window: bool,
    reason: &'static str,
}

#[derive(Serialize)]
struct ProgramEvidence {
    name: &'static str,
    program_id: String,
    programdata_id: String,
    canonical_programdata_pda: bool,
    program: AccountEvidence,
    program_header_sha256: String,
    captured_program_body_sha256: &'static str,
    program_body_matches_captured: bool,
    observed_programdata_link: String,
    programdata: AccountEvidence,
    programdata_header_sha256: String,
    captured_programdata_body_sha256: &'static str,
    programdata_body_matches_captured: bool,
    observed_deployment_slot: u64,
    observed_upgrade_authority: Option<String>,
    observed_upgrade_authority_effectively_disabled: bool,
    elf_tail_sha256: String,
    expected_elf_tail_sha256: &'static str,
}

#[derive(Serialize)]
struct AccountEvidence {
    address: String,
    owner: String,
    executable: bool,
    lamports: u64,
    rent_epoch: u64,
    data_len: usize,
    data_sha256: String,
}

#[derive(Serialize)]
struct SemanticEvidence {
    receiver_config_exact_fixture: bool,
    encoded_vaa_exact_fixture: bool,
    encoded_vaa_status: u8,
    encoded_vaa_version: u8,
    signed_vaa_version: u8,
    signed_vaa_guardian_set_index: u32,
    signed_vaa_signature_count: u8,
    signed_vaa_body_sha256: String,
    post_update_message_len: usize,
    post_update_message_sha256: String,
    post_update_proof_count: u32,
    post_update_treasury_id: u8,
    price_update_stable_body_matches_fixture: bool,
    price_update_write_authority: String,
    price_update_feed_id_hex: String,
    price: i64,
    confidence: u64,
    exponent: i32,
    publish_time: i64,
    prev_publish_time: i64,
    ema_price: i64,
    ema_confidence: u64,
    posted_slot: u64,
}

#[derive(Serialize)]
struct TransactionEvidence {
    stage: String,
    signature: String,
    slot: u64,
    block_time: Option<i64>,
    fee: Option<u64>,
    compute_units_consumed: Option<u64>,
    log_messages: Vec<String>,
}

struct Rpc {
    url: Url,
    client: Client,
    next_id: u64,
}

impl Rpc {
    fn new(url: Url) -> AnyResult<Self> {
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(15))
            .build()?;
        Ok(Self {
            url,
            client,
            next_id: 1,
        })
    }

    fn call(&mut self, method: &str, params: Value) -> AnyResult<Value> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| BootstrapError("RPC request ID overflow".into()))?;
        let response = self
            .client
            .post(self.url.clone())
            .json(&json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params}))
            .send()?;
        if !response.status().is_success() {
            return fail(format!("RPC {method} returned HTTP {}", response.status()));
        }
        let body: Value = response.json()?;
        if let Some(error) = body.get("error") {
            return fail(format!("RPC {method} failed: {error}"));
        }
        body.get("result")
            .cloned()
            .ok_or_else(|| Box::new(BootstrapError(format!("RPC {method} omitted result"))) as _)
    }

    fn account(&mut self, address: Pubkey) -> AnyResult<Option<RpcAccount>> {
        let result = self.call(
            "getAccountInfo",
            json!([address.to_string(), {"encoding":"base64", "commitment":"confirmed"}]),
        )?;
        let Some(value) = result.get("value").filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        decode_rpc_account(value).map(Some)
    }

    fn finalized_accounts(
        &mut self,
        addresses: &[Pubkey],
        minimum_slot: u64,
    ) -> AnyResult<(u64, Vec<Option<RpcAccount>>)> {
        let keys = addresses
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for _ in 0..150 {
            match self.call(
                "getMultipleAccounts",
                json!([keys, {
                    "encoding":"base64",
                    "commitment":"finalized",
                    "minContextSlot":minimum_slot
                }]),
            ) {
                Ok(result) => {
                    let slot = result
                        .pointer("/context/slot")
                        .and_then(Value::as_u64)
                        .ok_or_else(|| {
                            BootstrapError("getMultipleAccounts omitted context slot".into())
                        })?;
                    let values =
                        result
                            .get("value")
                            .and_then(Value::as_array)
                            .ok_or_else(|| {
                                BootstrapError("getMultipleAccounts omitted value array".into())
                            })?;
                    if values.len() != addresses.len() {
                        return fail("getMultipleAccounts returned the wrong account count");
                    }
                    let accounts = values
                        .iter()
                        .map(|value| {
                            if value.is_null() {
                                Ok(None)
                            } else {
                                decode_rpc_account(value).map(Some)
                            }
                        })
                        .collect::<AnyResult<Vec<_>>>()?;
                    return Ok((slot, accounts));
                }
                Err(error)
                    if error
                        .to_string()
                        .to_ascii_lowercase()
                        .contains("minimum context slot") =>
                {
                    thread::sleep(Duration::from_millis(100));
                }
                Err(error) => return Err(error),
            }
        }
        fail(format!(
            "finalized snapshot did not reach minimum slot {minimum_slot}"
        ))
    }

    fn required_account(&mut self, address: Pubkey, label: &str) -> AnyResult<RpcAccount> {
        self.account(address)?.ok_or_else(|| {
            Box::new(BootstrapError(format!("missing {label} account {address}"))) as _
        })
    }

    fn latest_blockhash(&mut self) -> AnyResult<Hash> {
        let result = self.call("getLatestBlockhash", json!([{"commitment":"confirmed"}]))?;
        let text = result
            .pointer("/value/blockhash")
            .and_then(Value::as_str)
            .ok_or_else(|| BootstrapError("getLatestBlockhash omitted blockhash".into()))?;
        Ok(Hash::from_str(text)?)
    }

    fn minimum_balance(&mut self, space: usize) -> AnyResult<u64> {
        let value = self.call(
            "getMinimumBalanceForRentExemption",
            json!([space, {"commitment":"confirmed"}]),
        )?;
        value
            .as_u64()
            .ok_or_else(|| Box::new(BootstrapError("rent result was not u64".into())) as _)
    }

    fn confirm(&mut self, signature: &str) -> AnyResult<()> {
        for _ in 0..150 {
            let result = self.call(
                "getSignatureStatuses",
                json!([[signature], {"searchTransactionHistory":true}]),
            )?;
            let status = result
                .get("value")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .filter(|value| !value.is_null());
            if let Some(status) = status {
                if !status.get("err").is_none_or(Value::is_null) {
                    return fail(format!("transaction {signature} failed: {}", status["err"]));
                }
                if matches!(
                    status.get("confirmationStatus").and_then(Value::as_str),
                    Some("confirmed" | "finalized")
                ) {
                    return Ok(());
                }
            }
            thread::sleep(Duration::from_millis(100));
        }
        fail(format!("timed out confirming transaction {signature}"))
    }

    fn transaction_evidence(
        &mut self,
        stage: String,
        signature: String,
    ) -> AnyResult<TransactionEvidence> {
        for _ in 0..50 {
            let result = self.call(
                "getTransaction",
                json!([signature, {"encoding":"json", "commitment":"confirmed", "maxSupportedTransactionVersion":0}]),
            )?;
            if !result.is_null() {
                let meta = result
                    .get("meta")
                    .ok_or_else(|| BootstrapError("getTransaction omitted meta".into()))?;
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
                return Ok(TransactionEvidence {
                    stage,
                    signature,
                    slot: json_u64(&result, "slot")?,
                    block_time: result.get("blockTime").and_then(Value::as_i64),
                    fee: meta.get("fee").and_then(Value::as_u64),
                    compute_units_consumed: meta
                        .get("computeUnitsConsumed")
                        .and_then(Value::as_u64),
                    log_messages: logs,
                });
            }
            thread::sleep(Duration::from_millis(100));
        }
        fail(format!(
            "getTransaction never exposed confirmed transaction {signature}"
        ))
    }

    fn send_transaction(
        &mut self,
        stage: impl Into<String>,
        instructions: &[Instruction],
        payer: &Keypair,
        additional_signers: &[&Keypair],
    ) -> AnyResult<TransactionEvidence> {
        let blockhash = self.latest_blockhash()?;
        let mut signers: Vec<&dyn Signer> = Vec::with_capacity(1 + additional_signers.len());
        signers.push(payer);
        signers.extend(
            additional_signers
                .iter()
                .copied()
                .map(|value| value as &dyn Signer),
        );
        let transaction = Transaction::new_signed_with_payer(
            instructions,
            Some(&payer.pubkey()),
            &signers,
            blockhash,
        );
        let bytes = bincode::serialize(&transaction)?;
        let result = self.call(
            "sendTransaction",
            json!([BASE64.encode(bytes), {
                "encoding":"base64",
                "skipPreflight":false,
                "preflightCommitment":"confirmed",
                "maxRetries":3
            }]),
        )?;
        let signature = result
            .as_str()
            .ok_or_else(|| BootstrapError("sendTransaction result was not a signature".into()))?
            .to_owned();
        self.confirm(&signature)?;
        self.transaction_evidence(stage.into(), signature)
    }

    fn airdrop(&mut self, payer: Pubkey) -> AnyResult<TransactionEvidence> {
        let result = self.call(
            "requestAirdrop",
            json!([payer.to_string(), AIRDROP_LAMPORTS, {"commitment":"confirmed"}]),
        )?;
        let signature = result
            .as_str()
            .ok_or_else(|| BootstrapError("requestAirdrop result was not a signature".into()))?
            .to_owned();
        self.confirm(&signature)?;
        self.transaction_evidence("airdrop_ephemeral_payer".into(), signature)
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("dclutch-local-provider-bootstrap: {error}");
        std::process::exit(1);
    }
}

fn run() -> AnyResult<()> {
    let args = parse_args(env::args().skip(1))?;
    if let Some(path) = &args.evidence
        && path.try_exists()?
    {
        return fail("--evidence path already exists");
    }
    let fixture_hashes = verify_fixtures()?;
    let mut rpc = Rpc::new(args.rpc_url.clone())?;
    let health = rpc.call("getHealth", json!([]))?;
    if health.as_str() != Some("ok") {
        return fail(format!("local RPC is not healthy: {health}"));
    }
    let validator_version = rpc.call("getVersion", json!([]))?;
    let genesis_hash = rpc
        .call("getGenesisHash", json!([]))?
        .as_str()
        .ok_or_else(|| BootstrapError("getGenesisHash result was not a string".into()))?
        .to_owned();

    let receiver = pubkey(RECEIVER_ID)?;
    let receiver_programdata = pubkey(RECEIVER_PROGRAMDATA)?;
    let router = pubkey(ROUTER_ID)?;
    let router_programdata = pubkey(ROUTER_PROGRAMDATA)?;
    let loader = pubkey(UPGRADEABLE_LOADER)?;
    let system = pubkey(SYSTEM_PROGRAM)?;
    let clock = pubkey(CLOCK_SYSVAR)?;
    let rent = pubkey(RENT_SYSVAR)?;
    let compute_budget = pubkey(COMPUTE_BUDGET_PROGRAM)?;
    let token = pubkey(TOKEN_PROGRAM)?;
    let token_2022 = pubkey(TOKEN_2022_PROGRAM)?;

    let programs = vec![
        inspect_program(
            &mut rpc,
            ProgramPin {
                name: "pyth-receiver",
                program_id: receiver,
                programdata_id: receiver_programdata,
                elf: RECEIVER_ELF,
                elf_sha256: "c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64",
                captured_program_sha256: "ef37dd1cee22d731902a8c04ed2e13136a2b8aa7068d9db3aff2ed1ec7b634e5",
                captured_programdata_sha256: "7122abc6b5e78d30bf88c869cb5d8783adaf897369d04eca827d3af8ffe18e5d",
            },
            loader,
        )?,
        inspect_program(
            &mut rpc,
            ProgramPin {
                name: "pyth-router",
                program_id: router,
                programdata_id: router_programdata,
                elf: ROUTER_ELF,
                elf_sha256: "f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb",
                captured_program_sha256: "1ee590ae23d5ecbf775aba910f06a993dee8f77bfd7028790dbd349651c8034b",
                captured_programdata_sha256: "f26f4b53b0f980455886116f500fa74ba475e51b1acb7f486b18afa9d73d948f",
            },
            loader,
        )?,
    ];

    let (dclutch_program, token_programs) = if let Some(pin) = args.dclutch.as_ref() {
        if pin.program_id != pubkey(DCLUTCH_PROGRAM)? {
            return fail(format!(
                "integrated profile requires the committed dClutch program ID {DCLUTCH_PROGRAM}"
            ));
        }
        let dclutch_program = inspect_runtime_program(
            &mut rpc,
            "dclutch",
            pin.program_id,
            loader,
            Some(&pin.elf_sha256),
        )?;
        let token_programs = vec![
            inspect_runtime_program(&mut rpc, "spl-token", token, loader, None)?,
            inspect_runtime_program(&mut rpc, "spl-token-2022", token_2022, loader, None)?,
        ];
        (Some(dclutch_program), token_programs)
    } else {
        (None, Vec::new())
    };

    let config = Pubkey::find_program_address(&[b"config"], &receiver).0;
    let guardian_set =
        Pubkey::find_program_address(&[b"GuardianSet", &0_u32.to_be_bytes()], &router).0;
    let treasury = Pubkey::find_program_address(&[b"treasury", &[0]], &receiver).0;
    let bridge = Pubkey::find_program_address(&[b"Bridge"], &router).0;
    let fee_collector = Pubkey::find_program_address(&[b"fee_collector"], &router).0;
    for (label, address) in [
        ("router bridge", bridge),
        ("router guardian set", guardian_set),
        ("receiver config", config),
    ] {
        if rpc.account(address)?.is_some() {
            return fail(format!(
                "{label} {address} already exists; bootstrap requires a fresh validator"
            ));
        }
    }

    let payer = Keypair::new();
    let encoded_vaa = Keypair::new();
    let update = Keypair::new();
    let mut transactions = vec![rpc.airdrop(payer.pubkey())?];

    transactions.push(rpc.send_transaction(
        "initialize_real_router",
        &[Instruction {
            program_id: router,
            accounts: vec![
                AccountMeta::new(bridge, false),
                AccountMeta::new(guardian_set, false),
                AccountMeta::new(fee_collector, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(clock, false),
                AccountMeta::new_readonly(rent, false),
                AccountMeta::new_readonly(system, false),
            ],
            data: ROUTER_INITIALIZE.to_vec(),
        }],
        &payer,
        &[],
    )?);
    transactions.push(rpc.send_transaction(
        "initialize_real_receiver",
        &[Instruction {
            program_id: receiver,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(config, false),
                AccountMeta::new_readonly(system, false),
            ],
            data: RECEIVER_INITIALIZE.to_vec(),
        }],
        &payer,
        &[],
    )?);

    let config_account = rpc.required_account(config, "receiver config")?;
    require_account(
        &config_account,
        receiver,
        false,
        Some(RECEIVER_CONFIG),
        "receiver config",
    )?;

    let encoded_size = ENCODED_VAA_HEADER_BYTES
        .checked_add(SIGNED_VAA.len())
        .ok_or_else(|| BootstrapError("EncodedVAA size overflow".into()))?;
    let encoded_rent = rpc.minimum_balance(encoded_size)?;
    transactions.push(rpc.send_transaction(
        "create_encoded_vaa",
        &[system_create_account(
            payer.pubkey(),
            encoded_vaa.pubkey(),
            encoded_rent,
            encoded_size,
            router,
            system,
        )?],
        &payer,
        &[&encoded_vaa],
    )?);
    transactions.push(rpc.send_transaction(
        "initialize_encoded_vaa",
        &[Instruction {
            program_id: router,
            accounts: vec![
                AccountMeta::new_readonly(payer.pubkey(), true),
                AccountMeta::new(encoded_vaa.pubkey(), false),
            ],
            data: anchor_discriminator(b"global:init_encoded_vaa").to_vec(),
        }],
        &payer,
        &[],
    )?);
    for (chunk_index, chunk) in SIGNED_VAA.chunks(WRITE_CHUNK_BYTES).enumerate() {
        let offset = chunk_index
            .checked_mul(WRITE_CHUNK_BYTES)
            .ok_or_else(|| BootstrapError("VAA write offset overflow".into()))?;
        transactions.push(rpc.send_transaction(
            format!("write_encoded_vaa_{chunk_index}"),
            &[write_encoded_vaa(
                router,
                payer.pubkey(),
                encoded_vaa.pubkey(),
                offset,
                chunk,
            )?],
            &payer,
            &[],
        )?);
    }
    transactions.push(rpc.send_transaction(
        "verify_encoded_vaa_v1",
        &[
            set_compute_unit_limit(compute_budget, 1_400_000),
            Instruction {
                program_id: router,
                accounts: vec![
                    AccountMeta::new_readonly(payer.pubkey(), true),
                    AccountMeta::new(encoded_vaa.pubkey(), false),
                    AccountMeta::new_readonly(guardian_set, false),
                ],
                data: anchor_discriminator(b"global:verify_encoded_vaa_v1").to_vec(),
            },
        ],
        &payer,
        &[],
    )?);

    let encoded_account = rpc.required_account(encoded_vaa.pubkey(), "verified EncodedVAA")?;
    require_account(&encoded_account, router, false, None, "verified EncodedVAA")?;
    verify_encoded_vaa(&encoded_account.data, payer.pubkey())?;

    transactions.push(rpc.send_transaction(
        "post_price_update",
        &[Instruction {
            program_id: receiver,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(encoded_vaa.pubkey(), false),
                AccountMeta::new_readonly(config, false),
                AccountMeta::new(treasury, false),
                AccountMeta::new(update.pubkey(), true),
                AccountMeta::new_readonly(system, false),
                AccountMeta::new_readonly(payer.pubkey(), true),
            ],
            data: RECEIVER_POST_UPDATE.to_vec(),
        }],
        &payer,
        &[&update],
    )?);

    let update_account = rpc.required_account(update.pubkey(), "PriceUpdateV2")?;
    require_account(&update_account, receiver, false, None, "PriceUpdateV2")?;
    let semantic_checks = verify_semantics(
        &config_account,
        &encoded_account,
        payer.pubkey(),
        &update_account,
    )?;
    let update_evidence = account_evidence(update.pubkey(), &update_account);

    let mut provider_accounts = BTreeMap::new();
    for (name, address) in [
        ("router_bridge", bridge),
        ("router_guardian_set", guardian_set),
        ("router_fee_collector", fee_collector),
        ("receiver_config", config),
        ("receiver_treasury", treasury),
    ] {
        let account = rpc.required_account(address, name)?;
        provider_accounts.insert(name.into(), account_evidence(address, &account));
    }
    provider_accounts.insert(
        "encoded_vaa".into(),
        account_evidence(encoded_vaa.pubkey(), &encoded_account),
    );
    provider_accounts.insert("price_update_before_reclaim".into(), update_evidence);

    let dclutch_lifecycle = args
        .dclutch
        .as_ref()
        .map(|pin| {
            execute_dclutch_lifecycle(&mut rpc, pin, &payer, system, rent, &mut transactions)
        })
        .transpose()?;

    let dclutch_source = args
        .dclutch
        .as_ref()
        .map(|pin| {
            source::execute_integrated_source(
                &mut rpc,
                pin.program_id,
                &payer,
                loader,
                receiver,
                receiver_programdata,
                config,
                encoded_vaa.pubkey(),
                router,
                router_programdata,
                treasury,
                token,
                system,
                rent,
                clock,
                compute_budget,
                FIXTURE_PUBLISH_TIME,
                &RECEIVER_POST_UPDATE[8..],
                &mut transactions,
            )
        })
        .transpose()?;

    let price_update_reclaimed = if args.reclaim {
        transactions.push(rpc.send_transaction(
            "reclaim_price_update_rent",
            &[Instruction {
                program_id: receiver,
                accounts: vec![
                    AccountMeta::new(payer.pubkey(), true),
                    AccountMeta::new(update.pubkey(), false),
                ],
                data: anchor_discriminator(b"global:reclaim_rent").to_vec(),
            }],
            &payer,
            &[],
        )?);
        if rpc.account(update.pubkey())?.is_some() {
            return fail("PriceUpdateV2 still exists after ReclaimRent");
        }
        true
    } else {
        false
    };

    let observed_slot = rpc
        .call("getSlot", json!([{"commitment":"confirmed"}]))?
        .as_u64()
        .ok_or_else(|| BootstrapError("getSlot result was not u64".into()))?;
    let clock_account = rpc.required_account(clock, "Clock sysvar")?;
    let validator_clock = i64_le(&clock_account.data, 32, "Clock.unix_timestamp")?;
    let fixture_clock_delta = validator_clock.abs_diff(FIXTURE_PUBLISH_TIME);

    let evidence = Evidence {
        schema: if dclutch_source.is_some() {
            "dclutch-integrated-local-source-bootstrap-evidence-v2"
        } else {
            "dclutch-local-provider-bootstrap-evidence-v1"
        },
        evidence_class: if dclutch_lifecycle.is_some() {
            "local-validator-real-provider-and-dclutch-execution"
        } else {
            "local-validator-real-provider-execution"
        },
        rpc_url: args.rpc_url.to_string(),
        genesis_hash,
        validator_version,
        fixture: FixtureEvidence {
            source_repository: "pyth-network/pyth-crosschain",
            source_commit: "f50a3faf9fc5a223a22889799b2f778900f186b3",
            source_license: "Apache-2.0",
            captured_network: "Solana devnet bounded local clone captured 2026-08-22",
            synthetic_signed_vaa: true,
            hashes_sha256: fixture_hashes,
        },
        loader_time_boundary: LoaderTimeBoundary {
            profile_class: "local-elf-tail-execution-with-real-provider-state",
            release_bound_provider_loader_evidence: false,
            captured_receiver_deployment_slot: 460_336_311,
            captured_router_deployment_slot: 460_336_290,
            observed_validator_slot: observed_slot,
            observed_validator_clock_unix_timestamp: validator_clock,
            fixture_publish_time: FIXTURE_PUBLISH_TIME,
            fixture_publish_time_matches_validator_clock: validator_clock == FIXTURE_PUBLISH_TIME,
            fixture_clock_delta_seconds: fixture_clock_delta,
            fixture_within_reference_60_second_window: fixture_clock_delta <= 60,
            reason: "test-validator regenerated Loader V3 headers/slots/authority and uses its current clock; exact ELF execution and provider state do not prove the captured release identity or captured-clock equivalence",
        },
        programs,
        dclutch_program,
        token_programs,
        provider_accounts,
        semantic_checks,
        transactions,
        payer: payer.pubkey().to_string(),
        encoded_vaa: encoded_vaa.pubkey().to_string(),
        price_update: update.pubkey().to_string(),
        price_update_reclaimed,
        dclutch_lifecycle,
        dclutch_resolution_executed: dclutch_source.as_ref().is_some_and(|evidence| {
            evidence.market_resolved && evidence.source_terminal && evidence.source_update_reclaimed
        }),
        dclutch_source,
        provider_state_initialized: true,
        captured_release_identity_claimed: false,
        dclutch_lifecycle_executed: args.dclutch.is_some(),
    };
    let output = serde_json::to_vec_pretty(&evidence)?;
    if let Some(path) = args.evidence {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(&output)?;
        file.write_all(b"\n")?;
        eprintln!("wrote {}", path.display());
    } else {
        std::io::stdout().write_all(&output)?;
        std::io::stdout().write_all(b"\n")?;
    }
    Ok(())
}

fn parse_args(arguments: impl Iterator<Item = String>) -> AnyResult<Args> {
    let mut rpc_url = None;
    let mut evidence = None;
    let mut reclaim = false;
    let mut dclutch_program_id = None;
    let mut dclutch_elf_sha256 = None;
    let mut dclutch_source_commit = None;
    let mut dclutch_source_archive_sha256 = None;
    let mut values = arguments.peekable();
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--rpc-url" => {
                let value = values
                    .next()
                    .ok_or_else(|| BootstrapError("--rpc-url requires a value".into()))?;
                if rpc_url.is_some() {
                    return fail("--rpc-url may be supplied only once");
                }
                rpc_url = Some(validate_rpc_url(&value)?);
            }
            "--evidence" => {
                let value = values
                    .next()
                    .ok_or_else(|| BootstrapError("--evidence requires a value".into()))?;
                if evidence.is_some() {
                    return fail("--evidence may be supplied only once");
                }
                let path = PathBuf::from(value);
                if !path.is_absolute() {
                    return fail("--evidence must be an absolute path");
                }
                evidence = Some(path);
            }
            "--reclaim" => reclaim = true,
            "--dclutch-program-id" => {
                let value = values.next().ok_or_else(|| {
                    BootstrapError("--dclutch-program-id requires a value".into())
                })?;
                if dclutch_program_id.is_some() {
                    return fail("--dclutch-program-id may be supplied only once");
                }
                dclutch_program_id = Some(pubkey(&value)?);
            }
            "--dclutch-elf-sha256" => {
                let value = values.next().ok_or_else(|| {
                    BootstrapError("--dclutch-elf-sha256 requires a value".into())
                })?;
                require_lower_hex(&value, 64, "--dclutch-elf-sha256")?;
                dclutch_elf_sha256 = Some(value);
            }
            "--dclutch-source-commit" => {
                let value = values.next().ok_or_else(|| {
                    BootstrapError("--dclutch-source-commit requires a value".into())
                })?;
                require_lower_hex(&value, 40, "--dclutch-source-commit")?;
                dclutch_source_commit = Some(value);
            }
            "--dclutch-source-archive-sha256" => {
                let value = values.next().ok_or_else(|| {
                    BootstrapError("--dclutch-source-archive-sha256 requires a value".into())
                })?;
                require_lower_hex(&value, 64, "--dclutch-source-archive-sha256")?;
                dclutch_source_archive_sha256 = Some(value);
            }
            "-h" | "--help" => {
                println!(
                    "Usage: dclutch-local-provider-bootstrap --rpc-url LOOPBACK_HTTP_URL [--evidence ABSOLUTE_NEW_FILE] [--reclaim] [--dclutch-program-id PUBKEY --dclutch-elf-sha256 SHA256 --dclutch-source-commit GIT_COMMIT --dclutch-source-archive-sha256 SHA256]"
                );
                std::process::exit(0);
            }
            _ => return fail(format!("unknown argument: {argument}")),
        }
    }
    let dclutch = match (
        dclutch_program_id,
        dclutch_elf_sha256,
        dclutch_source_commit,
        dclutch_source_archive_sha256,
    ) {
        (None, None, None, None) => None,
        (Some(program_id), Some(elf_sha256), Some(source_commit), Some(source_archive_sha256)) => {
            Some(DclutchPin {
                program_id,
                elf_sha256,
                source_commit,
                source_archive_sha256,
            })
        }
        _ => {
            return fail(
                "integrated dClutch mode requires all four --dclutch-* provenance arguments",
            );
        }
    };
    Ok(Args {
        rpc_url: rpc_url.ok_or_else(|| BootstrapError("--rpc-url is required".into()))?,
        evidence,
        reclaim,
        dclutch,
    })
}

fn require_lower_hex(value: &str, length: usize, label: &str) -> AnyResult<()> {
    if value.len() != length
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return fail(format!(
            "{label} must be exactly {length} lowercase hex characters"
        ));
    }
    Ok(())
}

fn validate_rpc_url(value: &str) -> AnyResult<Url> {
    let url = Url::parse(value)?;
    if url.scheme() != "http" {
        return fail("RPC URL must use http on the local execution profile");
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return fail("RPC URL must not contain credentials, query, or fragment");
    }
    if url.path() != "/" && !url.path().is_empty() {
        return fail("RPC URL must not contain a path");
    }
    let host = url
        .host_str()
        .ok_or_else(|| BootstrapError("RPC URL omitted host".into()))?;
    let normalized_host = host.trim_start_matches('[').trim_end_matches(']');
    let loopback = normalized_host.eq_ignore_ascii_case("localhost")
        || normalized_host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !loopback {
        return fail("RPC URL host is not loopback");
    }
    if url.port().is_none() {
        return fail("RPC URL must name an explicit local port");
    }
    Ok(url)
}

fn verify_fixtures() -> AnyResult<BTreeMap<&'static str, String>> {
    let fixtures = [
        (
            "PROVENANCE.md",
            PROVENANCE,
            "636e590b02585c98e55ad8603bf06d03c7df2426a1816958f8eae2dffca2fd87",
        ),
        (
            "UPSTREAM_LICENSE",
            UPSTREAM_LICENSE,
            "814162e3e1ec1c02ab68400bf98859ad73af3d67e19c026e98426a91085973a1",
        ),
        (
            "receiver.so",
            RECEIVER_ELF,
            "c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64",
        ),
        (
            "router.so",
            ROUTER_ELF,
            "f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb",
        ),
        (
            "router-initialize.data",
            ROUTER_INITIALIZE,
            "3667940a4428a8f2411a0ff11157ecc4ba1076c3c61273a108da6405c51e0b0b",
        ),
        (
            "receiver-initialize.data",
            RECEIVER_INITIALIZE,
            "d9c80906af92f99a0c8441f4463186056b1c12cb990999acfa198a46ec62729f",
        ),
        (
            "receiver-config.account",
            RECEIVER_CONFIG,
            "05038cf707afceac3df1aae735b096344ad639506b00f1db0ac1c084d6b645aa",
        ),
        (
            "signed.vaa",
            SIGNED_VAA,
            "ed8b973f36a932b9ec88659953859c8096f14e5aebd085bbe32b22c41a142c0d",
        ),
        (
            "receiver-post-update.data",
            RECEIVER_POST_UPDATE,
            "3bf9188bd6183155ea30738c3ab9da706ea7013bf5a7887a531e90b9bea85e1d",
        ),
        (
            "price-update.account",
            PRICE_UPDATE,
            "e5435e5b2e54d6083a9d1230e33f0635f6c74eb9db62899cfbb559f99c798a2b",
        ),
    ];
    let mut output = BTreeMap::new();
    for (name, bytes, expected) in fixtures {
        let observed = sha256(bytes);
        if observed != expected {
            return fail(format!("fixture hash mismatch for {name}: {observed}"));
        }
        output.insert(name, observed);
    }
    Ok(output)
}

fn inspect_program(rpc: &mut Rpc, pin: ProgramPin, loader: Pubkey) -> AnyResult<ProgramEvidence> {
    let ProgramPin {
        name,
        program_id,
        programdata_id,
        elf: expected_elf,
        elf_sha256: expected_elf_hash,
        captured_program_sha256: captured_program_hash,
        captured_programdata_sha256: captured_programdata_hash,
    } = pin;
    let derived = Pubkey::find_program_address(&[program_id.as_ref()], &loader).0;
    if derived != programdata_id {
        return fail(format!("{name} pinned ProgramData is not canonical PDA"));
    }
    let program = rpc.required_account(program_id, name)?;
    require_account(&program, loader, true, None, name)?;
    if program.data.len() != 36 || u32_le(&program.data, 0, "Program tag")? != 2 {
        return fail(format!("{name} has invalid Loader V3 Program body"));
    }
    let observed_link = Pubkey::new_from_array(array_32(&program.data, 4, "ProgramData link")?);
    if observed_link != programdata_id {
        return fail(format!("{name} ProgramData link mismatch"));
    }
    let observed_program_hash = sha256(&program.data);
    if observed_program_hash != captured_program_hash {
        return fail(format!(
            "{name} Program body differs from captured linkage body"
        ));
    }
    let programdata = rpc.required_account(programdata_id, "ProgramData")?;
    require_account(&programdata, loader, false, None, "ProgramData")?;
    if programdata.data.len() < 45 || u32_le(&programdata.data, 0, "ProgramData tag")? != 3 {
        return fail(format!("{name} has invalid Loader V3 ProgramData body"));
    }
    let deployment_slot = u64_le(&programdata.data, 4, "ProgramData slot")?;
    let authority = match byte(&programdata.data, 12, "ProgramData authority tag")? {
        0 => {
            if programdata.data.get(13..45) != Some(&[0_u8; 32]) {
                return fail(format!("{name} has noncanonical None authority padding"));
            }
            None
        }
        1 => Some(
            Pubkey::new_from_array(array_32(&programdata.data, 13, "upgrade authority")?)
                .to_string(),
        ),
        tag => {
            return fail(format!(
                "{name} has invalid upgrade authority option tag {tag}"
            ));
        }
    };
    let authority_effectively_disabled = authority
        .as_deref()
        .is_none_or(|value| value == Pubkey::default().to_string());
    if !authority_effectively_disabled {
        return fail(format!(
            "{name} local profile unexpectedly remains upgradeable"
        ));
    }
    let elf = programdata
        .data
        .get(45..)
        .ok_or_else(|| BootstrapError(format!("{name} omitted ELF tail")))?;
    if elf != expected_elf || sha256(elf) != expected_elf_hash {
        return fail(format!("{name} ELF tail differs from pinned fixture"));
    }
    Ok(ProgramEvidence {
        name,
        program_id: program_id.to_string(),
        programdata_id: programdata_id.to_string(),
        canonical_programdata_pda: true,
        program: account_evidence(program_id, &program),
        program_header_sha256: observed_program_hash,
        captured_program_body_sha256: captured_program_hash,
        program_body_matches_captured: true,
        observed_programdata_link: observed_link.to_string(),
        programdata: account_evidence(programdata_id, &programdata),
        programdata_header_sha256: sha256(&programdata.data[..45]),
        captured_programdata_body_sha256: captured_programdata_hash,
        programdata_body_matches_captured: sha256(&programdata.data) == captured_programdata_hash,
        observed_deployment_slot: deployment_slot,
        observed_upgrade_authority: authority,
        observed_upgrade_authority_effectively_disabled: authority_effectively_disabled,
        elf_tail_sha256: sha256(elf),
        expected_elf_tail_sha256: expected_elf_hash,
    })
}

fn inspect_runtime_program(
    rpc: &mut Rpc,
    name: &'static str,
    program_id: Pubkey,
    loader: Pubkey,
    expected_elf_hash: Option<&str>,
) -> AnyResult<RuntimeProgramEvidence> {
    let canonical_programdata = Pubkey::find_program_address(&[program_id.as_ref()], &loader).0;
    let program = rpc.required_account(program_id, name)?;
    require_account(&program, loader, true, None, name)?;
    if program.data.len() != 36 || u32_le(&program.data, 0, "Program tag")? != 2 {
        return fail(format!("{name} has invalid Loader V3 Program body"));
    }
    let programdata_id = Pubkey::new_from_array(array_32(&program.data, 4, "ProgramData link")?);
    if programdata_id != canonical_programdata {
        return fail(format!("{name} ProgramData link is not its canonical PDA"));
    }
    let programdata = rpc.required_account(programdata_id, "ProgramData")?;
    require_account(&programdata, loader, false, None, "ProgramData")?;
    if programdata.data.len() < 49 || u32_le(&programdata.data, 0, "ProgramData tag")? != 3 {
        return fail(format!("{name} has invalid Loader V3 ProgramData body"));
    }
    let deployment_slot = u64_le(&programdata.data, 4, "ProgramData slot")?;
    let authority = match byte(&programdata.data, 12, "ProgramData authority tag")? {
        0 => {
            if programdata.data.get(13..45) != Some(&[0_u8; 32]) {
                return fail(format!("{name} has noncanonical None authority padding"));
            }
            None
        }
        1 => Some(
            Pubkey::new_from_array(array_32(&programdata.data, 13, "upgrade authority")?)
                .to_string(),
        ),
        tag => return fail(format!("{name} has invalid upgrade authority tag {tag}")),
    };
    let authority_effectively_disabled = authority
        .as_deref()
        .is_none_or(|value| value == Pubkey::default().to_string());
    if !authority_effectively_disabled {
        return fail(format!(
            "{name} unexpectedly has an effective local upgrade authority"
        ));
    }
    let elf = programdata
        .data
        .get(45..)
        .ok_or_else(|| BootstrapError(format!("{name} omitted ELF tail")))?;
    if !elf.starts_with(b"\x7fELF") {
        return fail(format!("{name} ProgramData tail is not an ELF"));
    }
    let elf_hash = sha256(elf);
    let hash_match = expected_elf_hash.map(|expected| elf_hash == expected);
    if hash_match == Some(false) {
        return fail(format!(
            "{name} ELF tail hash {elf_hash} differs from expected {}",
            expected_elf_hash.unwrap_or_default()
        ));
    }
    Ok(RuntimeProgramEvidence {
        name,
        program_id: program_id.to_string(),
        programdata_id: programdata_id.to_string(),
        canonical_programdata_pda: true,
        program: account_evidence(program_id, &program),
        programdata: account_evidence(programdata_id, &programdata),
        observed_deployment_slot: deployment_slot,
        observed_upgrade_authority: authority,
        observed_upgrade_authority_effectively_disabled: authority_effectively_disabled,
        elf_tail_sha256: elf_hash,
        expected_elf_tail_sha256: expected_elf_hash.map(str::to_owned),
        elf_tail_matches_expected: hash_match,
    })
}

fn execute_dclutch_lifecycle(
    rpc: &mut Rpc,
    pin: &DclutchPin,
    payer: &Keypair,
    system: Pubkey,
    rent: Pubkey,
    transactions: &mut Vec<TransactionEvidence>,
) -> AnyResult<DclutchLifecycleEvidence> {
    let authority = RefundAuthority::new(payer.pubkey().to_bytes())
        .map_err(|error| BootstrapError(format!("invalid ephemeral authority: {error:?}")))?;
    let (credit, bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, payer.pubkey().as_ref()],
        &pin.program_id,
    );
    if rpc.account(credit)?.is_some() {
        return fail(format!(
            "dClutch rent-credit {credit} already exists; integrated bootstrap requires fresh state"
        ));
    }
    let create = CreateRentCreditV1::new(authority, bump);
    transactions.push(rpc.send_transaction(
        "dclutch_create_rent_credit",
        &[Instruction {
            program_id: pin.program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(credit, false),
                AccountMeta::new_readonly(system, false),
                AccountMeta::new_readonly(rent, false),
            ],
            data: create.to_bytes().to_vec(),
        }],
        payer,
        &[],
    )?);
    let rent_floor = rpc.minimum_balance(RENT_CREDIT_BYTES_V1)?;
    let created = rpc.required_account(credit, "created dClutch rent credit")?;
    let expected_state = create.credit().to_bytes();
    require_account(
        &created,
        pin.program_id,
        false,
        Some(&expected_state),
        "created dClutch rent credit",
    )?;
    if created.lamports != rent_floor {
        return fail(format!(
            "created dClutch rent credit has {} lamports, expected current rent floor {rent_floor}",
            created.lamports
        ));
    }
    let after_create = account_evidence(credit, &created);

    transactions.push(rpc.send_transaction(
        "fund_dclutch_rent_credit_surplus",
        &[system_transfer(
            payer.pubkey(),
            credit,
            RENT_CREDIT_DEPOSIT_LAMPORTS,
            system,
        )],
        payer,
        &[],
    )?);
    let funded = rpc.required_account(credit, "funded dClutch rent credit")?;
    let funded_balance = rent_floor
        .checked_add(RENT_CREDIT_DEPOSIT_LAMPORTS)
        .ok_or_else(|| BootstrapError("funded rent-credit balance overflow".into()))?;
    if funded.data != expected_state
        || funded.owner != pin.program_id
        || funded.lamports != funded_balance
    {
        return fail("funding changed rent-credit state or produced the wrong exact balance");
    }
    let after_fund = account_evidence(credit, &funded);

    let withdraw = WithdrawRentCreditV1::new(RENT_CREDIT_WITHDRAW_LAMPORTS)
        .map_err(|error| BootstrapError(format!("invalid bounded withdrawal: {error:?}")))?;
    transactions.push(rpc.send_transaction(
        "dclutch_withdraw_rent_credit_surplus",
        &[Instruction {
            program_id: pin.program_id,
            accounts: vec![
                AccountMeta::new(credit, false),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(rent, false),
            ],
            data: withdraw.to_bytes().to_vec(),
        }],
        payer,
        &[],
    )?);
    let withdrawn = rpc.required_account(credit, "withdrawn dClutch rent credit")?;
    let final_surplus = RENT_CREDIT_DEPOSIT_LAMPORTS
        .checked_sub(RENT_CREDIT_WITHDRAW_LAMPORTS)
        .ok_or_else(|| BootstrapError("rent-credit fixture underflow".into()))?;
    let withdrawn_balance = rent_floor
        .checked_add(final_surplus)
        .ok_or_else(|| BootstrapError("withdrawn rent-credit balance overflow".into()))?;
    if withdrawn.data != expected_state
        || withdrawn.owner != pin.program_id
        || withdrawn.lamports != withdrawn_balance
    {
        return fail("withdrawal changed rent-credit state or produced the wrong exact balance");
    }
    let after_withdraw = account_evidence(credit, &withdrawn);

    Ok(DclutchLifecycleEvidence {
        source_commit: pin.source_commit.clone(),
        source_archive_sha256: pin.source_archive_sha256.clone(),
        lifecycle: "RentCredit Create -> System fund -> authority Withdraw",
        authority: payer.pubkey().to_string(),
        rent_credit: credit.to_string(),
        pda_bump: bump,
        rent_floor_lamports: rent_floor,
        deposited_lamports: RENT_CREDIT_DEPOSIT_LAMPORTS,
        withdrawn_lamports: RENT_CREDIT_WITHDRAW_LAMPORTS,
        final_surplus_lamports: final_surplus,
        exact_state_matches_contract: true,
        after_create,
        after_fund,
        after_withdraw,
        resolution_executed: false,
        resolution_boundary: "This nested RentCredit probe does not itself perform resolution; integrated evidence records the separately checked real-provider Source composition under dclutch_source.",
    })
}

fn verify_encoded_vaa(data: &[u8], authority: Pubkey) -> AnyResult<()> {
    if data.len() != ENCODED_VAA_HEADER_BYTES + SIGNED_VAA.len() {
        return fail(format!(
            "verified EncodedVAA has invalid length {}",
            data.len()
        ));
    }
    if data.get(..8) != Some(&anchor_discriminator(b"account:EncodedVaa")) {
        return fail("verified EncodedVAA discriminator mismatch");
    }
    if byte(data, 8, "EncodedVAA status")? != 2 {
        return fail("EncodedVAA is not ProcessingStatus::Verified");
    }
    if array_32(data, 9, "EncodedVAA write authority")? != authority.to_bytes() {
        return fail("EncodedVAA write authority mismatch");
    }
    if byte(data, 41, "EncodedVAA version")? != 1 {
        return fail("EncodedVAA is not VAA version 1");
    }
    let length = usize::try_from(u32_le(data, 42, "EncodedVAA length")?)?;
    if length != SIGNED_VAA.len() || data.get(46..) != Some(SIGNED_VAA) {
        return fail("EncodedVAA does not contain the exact pinned signed VAA");
    }
    Ok(())
}

fn verify_semantics(
    config: &RpcAccount,
    encoded: &RpcAccount,
    payer: Pubkey,
    update: &RpcAccount,
) -> AnyResult<SemanticEvidence> {
    if RECEIVER_POST_UPDATE.len() != 102 {
        return fail("captured receiver PostUpdate instruction length changed");
    }
    let post = PostUpdateParamsView::parse(&RECEIVER_POST_UPDATE[8..])
        .map_err(|error| BootstrapError(format!("invalid PostUpdate fixture: {error:?}")))?;
    if post.message().len() != 85 || post.proof_count() != 0 || post.treasury_id() != 0 {
        return fail("captured PostUpdate body semantics changed");
    }
    if SIGNED_VAA.len() < 6 {
        return fail("signed VAA is truncated");
    }
    let signature_count = SIGNED_VAA[5];
    let body_offset = 6_usize
        .checked_add(
            usize::from(signature_count)
                .checked_mul(66)
                .ok_or_else(|| BootstrapError("VAA signatures overflow".into()))?,
        )
        .ok_or_else(|| BootstrapError("VAA body offset overflow".into()))?;
    let vaa_body = SIGNED_VAA
        .get(body_offset..)
        .ok_or_else(|| BootstrapError("signed VAA body is truncated".into()))?;
    if SIGNED_VAA[0] != 1
        || u32_be(SIGNED_VAA, 1, "guardian set index")? != 0
        || signature_count != 13
    {
        return fail("signed VAA header differs from pinned 13-of-19 V1 proof");
    }
    if update.data.len() != FULL_PRICE_UPDATE_BYTES {
        return fail(format!("PriceUpdateV2 length was {}", update.data.len()));
    }
    if update.data.get(..8) != PRICE_UPDATE.get(..8)
        || update.data.get(40..125) != PRICE_UPDATE.get(40..125)
        || update.data.get(133) != Some(&0)
    {
        return fail("PriceUpdateV2 stable semantic body differs from pinned fixture");
    }
    let parsed = FullPriceUpdateV2::parse(&update.data)
        .map_err(|error| BootstrapError(format!("invalid posted PriceUpdateV2: {error:?}")))?;
    if parsed.write_authority() != payer.to_bytes()
        || parsed.feed_id() != [0x2a; 32]
        || parsed.price() != 100_000_000
        || parsed.confidence() != 6_357
        || parsed.exponent() != -8
        || parsed.publish_time() != FIXTURE_PUBLISH_TIME
        || parsed.prev_publish_time() != FIXTURE_PUBLISH_TIME - 1
        || parsed.ema_price() != 99_999_000
        || parsed.ema_confidence() != 6_400
    {
        return fail("posted PriceUpdateV2 decoded semantics differ from fixture provenance");
    }
    Ok(SemanticEvidence {
        receiver_config_exact_fixture: config.data == RECEIVER_CONFIG,
        encoded_vaa_exact_fixture: encoded.data.get(46..) == Some(SIGNED_VAA),
        encoded_vaa_status: encoded.data[8],
        encoded_vaa_version: encoded.data[41],
        signed_vaa_version: SIGNED_VAA[0],
        signed_vaa_guardian_set_index: u32_be(SIGNED_VAA, 1, "guardian set index")?,
        signed_vaa_signature_count: signature_count,
        signed_vaa_body_sha256: sha256(vaa_body),
        post_update_message_len: post.message().len(),
        post_update_message_sha256: sha256(post.message()),
        post_update_proof_count: post.proof_count(),
        post_update_treasury_id: post.treasury_id(),
        price_update_stable_body_matches_fixture: true,
        price_update_write_authority: payer.to_string(),
        price_update_feed_id_hex: hex(&parsed.feed_id()),
        price: parsed.price(),
        confidence: parsed.confidence(),
        exponent: parsed.exponent(),
        publish_time: parsed.publish_time(),
        prev_publish_time: parsed.prev_publish_time(),
        ema_price: parsed.ema_price(),
        ema_confidence: parsed.ema_confidence(),
        posted_slot: parsed.posted_slot(),
    })
}

fn system_create_account(
    payer: Pubkey,
    created: Pubkey,
    lamports: u64,
    space: usize,
    owner: Pubkey,
    system_program: Pubkey,
) -> AnyResult<Instruction> {
    let space = u64::try_from(space)?;
    let mut data = Vec::with_capacity(52);
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    data.extend_from_slice(&space.to_le_bytes());
    data.extend_from_slice(owner.as_ref());
    Ok(Instruction {
        program_id: system_program,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(created, true),
        ],
        data,
    })
}

fn system_transfer(
    source: Pubkey,
    destination: Pubkey,
    lamports: u64,
    system_program: Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2_u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: system_program,
        accounts: vec![
            AccountMeta::new(source, true),
            AccountMeta::new(destination, false),
        ],
        data,
    }
}

fn write_encoded_vaa(
    router: Pubkey,
    authority: Pubkey,
    encoded: Pubkey,
    index: usize,
    bytes: &[u8],
) -> AnyResult<Instruction> {
    let mut data = anchor_discriminator(b"global:write_encoded_vaa").to_vec();
    data.extend_from_slice(&u32::try_from(index)?.to_le_bytes());
    data.extend_from_slice(&u32::try_from(bytes.len())?.to_le_bytes());
    data.extend_from_slice(bytes);
    Ok(Instruction {
        program_id: router,
        accounts: vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(encoded, false),
        ],
        data,
    })
}

fn set_compute_unit_limit(program_id: Pubkey, units: u32) -> Instruction {
    let mut data = Vec::with_capacity(5);
    data.push(2);
    data.extend_from_slice(&units.to_le_bytes());
    Instruction {
        program_id,
        accounts: Vec::new(),
        data,
    }
}

fn require_account(
    account: &RpcAccount,
    owner: Pubkey,
    executable: bool,
    exact_data: Option<&[u8]>,
    label: &str,
) -> AnyResult<()> {
    if account.owner != owner || account.executable != executable {
        return fail(format!("{label} owner/executable mismatch"));
    }
    if exact_data.is_some_and(|expected| account.data != expected) {
        return fail(format!("{label} body differs from exact fixture"));
    }
    Ok(())
}

fn account_evidence(address: Pubkey, account: &RpcAccount) -> AccountEvidence {
    AccountEvidence {
        address: address.to_string(),
        owner: account.owner.to_string(),
        executable: account.executable,
        lamports: account.lamports,
        rent_epoch: account.rent_epoch,
        data_len: account.data.len(),
        data_sha256: sha256(&account.data),
    }
}

fn anchor_discriminator(name: &[u8]) -> [u8; 8] {
    let digest = Sha256::digest(name);
    let mut output = [0_u8; 8];
    output.copy_from_slice(&digest[..8]);
    output
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(DIGITS[usize::from(byte >> 4)]));
        output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    output
}

fn json_u64(value: &Value, field: &str) -> AnyResult<u64> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| Box::new(BootstrapError(format!("JSON field {field} was not u64"))) as _)
}

fn json_str<'a>(value: &'a Value, field: &str) -> AnyResult<&'a str> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| Box::new(BootstrapError(format!("JSON field {field} was not string"))) as _)
}

fn decode_rpc_account(value: &Value) -> AnyResult<RpcAccount> {
    let encoded = value
        .get("data")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .ok_or_else(|| BootstrapError("RPC account returned non-base64 data".into()))?;
    Ok(RpcAccount {
        lamports: json_u64(value, "lamports")?,
        owner: Pubkey::from_str(json_str(value, "owner")?)?,
        executable: value
            .get("executable")
            .and_then(Value::as_bool)
            .ok_or_else(|| BootstrapError("account executable was not boolean".into()))?,
        rent_epoch: json_u64(value, "rentEpoch")?,
        data: BASE64.decode(encoded)?,
    })
}

fn pubkey(text: &str) -> AnyResult<Pubkey> {
    Ok(Pubkey::from_str(text)?)
}

fn byte(bytes: &[u8], offset: usize, label: &str) -> AnyResult<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or_else(|| Box::new(BootstrapError(format!("{label} is truncated"))) as _)
}

fn array_32(bytes: &[u8], offset: usize, label: &str) -> AnyResult<[u8; 32]> {
    let end = offset
        .checked_add(32)
        .ok_or_else(|| BootstrapError(format!("{label} offset overflow")))?;
    bytes
        .get(offset..end)
        .ok_or_else(|| BootstrapError(format!("{label} is truncated")))?
        .try_into()
        .map_err(|_| Box::new(BootstrapError(format!("{label} width mismatch"))) as _)
}

fn u32_le(bytes: &[u8], offset: usize, label: &str) -> AnyResult<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| BootstrapError(format!("{label} offset overflow")))?;
    Ok(u32::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or_else(|| BootstrapError(format!("{label} is truncated")))?
            .try_into()?,
    ))
}

fn u32_be(bytes: &[u8], offset: usize, label: &str) -> AnyResult<u32> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| BootstrapError(format!("{label} offset overflow")))?;
    Ok(u32::from_be_bytes(
        bytes
            .get(offset..end)
            .ok_or_else(|| BootstrapError(format!("{label} is truncated")))?
            .try_into()?,
    ))
}

fn u64_le(bytes: &[u8], offset: usize, label: &str) -> AnyResult<u64> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| BootstrapError(format!("{label} offset overflow")))?;
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or_else(|| BootstrapError(format!("{label} is truncated")))?
            .try_into()?,
    ))
}

fn i64_le(bytes: &[u8], offset: usize, label: &str) -> AnyResult<i64> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| BootstrapError(format!("{label} offset overflow")))?;
    Ok(i64::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or_else(|| BootstrapError(format!("{label} is truncated")))?
            .try_into()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_url_policy_refuses_external_and_ambient_shapes() {
        assert!(validate_rpc_url("http://127.0.0.1:18890").is_ok());
        assert!(validate_rpc_url("http://[::1]:18890").is_ok());
        assert!(validate_rpc_url("http://localhost:18890").is_ok());
        for hostile in [
            "https://127.0.0.1:18890",
            "http://192.168.1.2:18890",
            "http://example.com:18890",
            "http://user@127.0.0.1:18890",
            "http://127.0.0.1:18890/path",
            "http://127.0.0.1",
        ] {
            assert!(validate_rpc_url(hostile).is_err(), "accepted {hostile}");
        }
    }

    #[test]
    fn all_fixture_hashes_and_semantic_shapes_are_pinned() {
        assert_eq!(verify_fixtures().expect("pinned fixtures").len(), 10);
        let post =
            PostUpdateParamsView::parse(&RECEIVER_POST_UPDATE[8..]).expect("exact post body");
        assert_eq!(post.message().len(), 85);
        assert_eq!(post.proof_count(), 0);
        assert_eq!(post.treasury_id(), 0);
        let parsed = FullPriceUpdateV2::parse(PRICE_UPDATE).expect("exact captured update");
        assert_eq!(parsed.feed_id(), [0x2a; 32]);
        assert_eq!(parsed.price(), 100_000_000);
        assert_eq!(parsed.confidence(), 6_357);
        assert_eq!(parsed.exponent(), -8);
        assert_eq!(parsed.publish_time(), FIXTURE_PUBLISH_TIME);
    }

    #[test]
    fn system_create_account_wire_is_exact() {
        let payer = Pubkey::new_from_array([1; 32]);
        let created = Pubkey::new_from_array([2; 32]);
        let owner = Pubkey::new_from_array([3; 32]);
        let system = Pubkey::default();
        let instruction = system_create_account(payer, created, 7, 1_044, owner, system)
            .expect("bounded create account");
        assert_eq!(instruction.program_id, system);
        assert_eq!(instruction.data.len(), 52);
        assert_eq!(&instruction.data[0..4], &0_u32.to_le_bytes());
        assert_eq!(&instruction.data[4..12], &7_u64.to_le_bytes());
        assert_eq!(&instruction.data[12..20], &1_044_u64.to_le_bytes());
        assert_eq!(&instruction.data[20..52], owner.as_ref());
    }

    #[test]
    fn integrated_arguments_are_all_or_nothing_and_bind_canonical_id() {
        let valid = [
            "--rpc-url",
            "http://127.0.0.1:19890",
            "--dclutch-program-id",
            DCLUTCH_PROGRAM,
            "--dclutch-elf-sha256",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--dclutch-source-commit",
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "--dclutch-source-archive-sha256",
            "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        ]
        .into_iter()
        .map(str::to_owned);
        let parsed = parse_args(valid).expect("complete integrated provenance");
        let pin = parsed.dclutch.expect("integrated mode");
        assert_eq!(pin.program_id, Pubkey::new_from_array([71; 32]));

        let partial = [
            "--rpc-url",
            "http://127.0.0.1:19890",
            "--dclutch-program-id",
            DCLUTCH_PROGRAM,
        ]
        .into_iter()
        .map(str::to_owned);
        assert!(parse_args(partial).is_err());
    }

    #[test]
    fn system_transfer_wire_is_exact() {
        let source = Pubkey::new_from_array([1; 32]);
        let destination = Pubkey::new_from_array([2; 32]);
        let system = Pubkey::default();
        let instruction = system_transfer(source, destination, 400_000, system);
        assert_eq!(instruction.program_id, system);
        assert_eq!(instruction.data.len(), 12);
        assert_eq!(&instruction.data[..4], &2_u32.to_le_bytes());
        assert_eq!(&instruction.data[4..], &400_000_u64.to_le_bytes());
        assert_eq!(instruction.accounts.len(), 2);
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[0].is_writable);
        assert!(!instruction.accounts[1].is_signer);
        assert!(instruction.accounts[1].is_writable);
    }
}
