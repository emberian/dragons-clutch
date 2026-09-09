#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real-validator Fractional Open-to-terminal continuation.
//!
//! Genesis contains an Open Market, its live Primary Source, and active funded
//! Resolution escrow. It contains no terminal certificate, Core terminal
//! receipt, or Claims-role Custody replay. The runner produces those facts by
//! executing the shipped Resolution, Core, Claims, Custody and Token-2022 ELFs,
//! then either burns losing shards or redeems winning shards.

#[path = "../journal.rs"]
#[allow(dead_code)]
mod journal;
#[path = "../narrow_fixture.rs"]
#[allow(dead_code, unused_imports)]
mod narrow_fixture;
#[path = "../stage.rs"]
#[allow(dead_code)]
mod stage;
#[path = "../validator.rs"]
#[allow(dead_code)]
mod validator;

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::ExitCode,
    thread::sleep,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use solana_address_lookup_table_interface::instruction::{
    create_lookup_table, extend_lookup_table,
};
use solana_client::{rpc_client::RpcClient, rpc_request::RpcRequest};
use solana_commitment_config::CommitmentConfig;
use solana_compute_budget_interface::ComputeBudgetInstruction;
use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk::{
    signature::{Keypair, Signer, keypair_from_seed},
    transaction::{Transaction, VersionedTransaction},
};

use dclutch_claims::{
    custody_replay_v1::ClaimsCustodyReplayRequestV1,
    fractional::{
        FRACTIONAL_TERMINAL_ROOT_V3, FractionalCapabilityRootV4, FractionalExposureActionV2,
        FractionalExposureRequestInputV2, FractionalExposureRequestV2,
        decode_fractional_capability_root_v4,
    },
    fractional_kernel::{
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
        FractionalExposureTermsV2,
    },
    liability_basis_state_v2::LiabilityBasisMarketViewV2,
};
use dclutch_claims_sbf::custody_replay_v1::expected_request_v1;
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CustodyReplaySeedsV1, CustodyReplayV1,
};
use dclutch_market::{CoreState, Phase};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    deadline_failure_v1::{
        DeadlineFailureCoordinatesV1, FinalizedPairV1, build_commit_deadline_failure_v1,
    },
    fractional::build_fractional_terminal_atomic_claims_instruction_v3,
    resolution_core_v3::{
        ResolutionAdmitTerminalSnapshotV3, build_resolution_admit_terminal_v3,
        validate_resolution_admit_terminal_report_v3,
    },
};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_registry::{
    ACTIVATION_PDA_DOMAIN_V1,
    record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1},
    release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1},
};
use dclutch_source::{
    SourceResolutionPhaseV1, SourceResolutionStateV2, WINDOW_SPEC_SCHEMA_ID_V1,
    resolution::{ResolutionCertificateKindV2, ResolutionCertificateV2},
};

const RPC_PORT: u16 = 21061;
const ACTOR_SEED: [u8; 32] = [0x2c; 32];
const SLEEPER_SEED: [u8; 32] = [0x5c; 32];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalAction {
    ZeroBurn,
    Redeem,
}

impl TerminalAction {
    fn parse(value: &str) -> Result<Self> {
        match value {
            "zero-burn" => Ok(Self::ZeroBurn),
            "redeem" => Ok(Self::Redeem),
            _ => Err(Error::new("--terminal-action must be zero-burn or redeem").into()),
        }
    }

    const fn protocol(self) -> FractionalExposureActionV2 {
        match self {
            Self::ZeroBurn => FractionalExposureActionV2::TerminalZeroBurn,
            Self::Redeem => FractionalExposureActionV2::TerminalRedeem,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::ZeroBurn => "claims-terminal-zero-burn",
            Self::Redeem => "claims-terminal-redeem",
        }
    }
}

/// Stable runner error.
#[derive(Debug)]
pub struct Error(String);

impl Error {
    fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// Runner result.
pub type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>;

/// Read one exact ELF by its canonical filename.
pub fn read_elf(directory: &Path, name: &str) -> Result<Vec<u8>> {
    fs::read(directory.join(name)).map_err(|error| {
        Error::new(format!(
            "could not read {}: {error}",
            directory.join(name).display()
        ))
        .into()
    })
}

fn absolute(raw: Option<String>, flag: &str) -> Result<PathBuf> {
    let path = PathBuf::from(raw.ok_or_else(|| Error::new(format!("{flag} is required")))?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{flag} must be absolute")).into());
    }
    Ok(path)
}

fn parse() -> Result<(PathBuf, PathBuf, bool, TerminalAction)> {
    let mut arguments = env::args().skip(1);
    let (mut elf, mut out, mut keep, mut action) = (None, None, false, TerminalAction::ZeroBurn);
    while let Some(flag) = arguments.next() {
        match flag.as_str() {
            "--elf-dir" => elf = arguments.next(),
            "--out" => out = arguments.next(),
            "--keep" => keep = true,
            "--terminal-action" => {
                action = TerminalAction::parse(
                    &arguments
                        .next()
                        .ok_or_else(|| Error::new("--terminal-action value is required"))?,
                )?;
            }
            _ => return Err(Error::new(format!("unknown argument: {flag}")).into()),
        }
    }
    Ok((
        absolute(elf, "--elf-dir")?,
        absolute(out, "--out")?,
        keep,
        action,
    ))
}

fn system() -> Pubkey {
    Pubkey::new_from_array([0; 32])
}
fn rent_sysvar() -> Pubkey {
    solana_program::sysvar::rent::ID
}
fn clock_sysvar() -> Pubkey {
    solana_program::sysvar::clock::ID
}

fn transfer(from: Pubkey, to: Pubkey, lamports: u64) -> Instruction {
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&2_u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    Instruction {
        program_id: system(),
        accounts: vec![AccountMeta::new(from, true), AccountMeta::new(to, false)],
        data,
    }
}

fn pair(schema: [u8; 32], digest: [u8; 32]) -> (Pubkey, Pubkey) {
    (
        Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
            &stage::REGISTRY,
        )
        .0,
        Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &stage::REGISTRY,
        )
        .0,
    )
}

fn staged_programs(
    elves: &stage::Elves<'_>,
    resolution: &[u8],
    token: &[u8],
) -> Vec<(Pubkey, Vec<u8>)> {
    vec![
        (stage::CLAIMS, elves.claims.to_vec()),
        (stage::REGISTRY, elves.registry.to_vec()),
        (stage::CORE, elves.core.to_vec()),
        (stage::CUSTODY, elves.custody.to_vec()),
        (stage::CALLER, elves.caller.to_vec()),
        (stage::RESOLUTION, resolution.to_vec()),
        (stage::token_program(), token.to_vec()),
    ]
}

fn prepare(
    out: &Path,
    staged: &stage::TerminalStaged,
    programs: &[(Pubkey, Vec<u8>)],
) -> Result<()> {
    let accounts = out.join("accounts");
    if accounts.exists() {
        fs::remove_dir_all(&accounts)?;
    }
    fs::create_dir_all(&accounts)?;
    for account in &staged.base.accounts {
        // A finalized record's staging PDA and caller-authority seats are
        // canonically vacant. Naming them in the transaction materializes the
        // zero-lamport System view; preloading them would turn vacancy into a
        // rent-funded account and Resolution would correctly refuse it.
        if account.owner == system() && account.data.is_empty() {
            continue;
        }
        validator::write_account(
            &accounts,
            &validator::account_file(
                &validator::rpc(account.key),
                &validator::rpc(account.owner),
                &account.data,
                false,
            ),
        )?;
    }
    for (program, elf) in programs {
        let program = validator::rpc(*program);
        let programdata = validator::programdata_address(&program);
        validator::write_account(
            &accounts,
            &validator::account_file(
                &program,
                &validator::rpc(stage::loader()),
                &validator::program_account_bytes(&programdata),
                true,
            ),
        )?;
        validator::write_account(
            &accounts,
            &validator::account_file(
                &programdata,
                &validator::rpc(stage::loader()),
                &validator::programdata_bytes(elf),
                false,
            ),
        )?;
    }
    Ok(())
}

fn await_airdrop(client: &RpcClient, payer: &Keypair) -> Result<()> {
    let signature = client.request_airdrop(&payer.pubkey(), 10_000_000_000)?;
    let deadline = Instant::now() + Duration::from_secs(60);
    while !client.confirm_transaction(&signature)? {
        if Instant::now() > deadline {
            return Err(Error::new("airdrop never confirmed").into());
        }
        sleep(Duration::from_millis(250));
    }
    Ok(())
}

fn create_table(
    client: &RpcClient,
    payer: &Keypair,
    addresses: Vec<Pubkey>,
) -> Result<AddressLookupTableAccount> {
    let slot = client.get_slot()?;
    while client.get_slot()? <= slot {
        sleep(Duration::from_millis(200));
    }
    let (create, key) = create_lookup_table(payer.pubkey(), payer.pubkey(), slot);
    submit_legacy(client, payer, &[create])?;
    for chunk in addresses.chunks(20) {
        submit_legacy(
            client,
            payer,
            &[extend_lookup_table(
                key,
                payer.pubkey(),
                Some(payer.pubkey()),
                chunk.to_vec(),
            )],
        )?;
    }
    let warm = client.get_slot()?;
    while client.get_slot()? <= warm + 1 {
        sleep(Duration::from_millis(200));
    }
    Ok(AddressLookupTableAccount { key, addresses })
}

fn submit_legacy(client: &RpcClient, payer: &Keypair, instructions: &[Instruction]) -> Result<()> {
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&payer.pubkey()),
        &[payer],
        client.get_latest_blockhash()?,
    );
    client.send_and_confirm_transaction(&transaction)?;
    Ok(())
}

fn submit_v0(
    client: &RpcClient,
    payer: &Keypair,
    extra_signer: Option<&Keypair>,
    table: &AddressLookupTableAccount,
    instructions: &[Instruction],
) -> Result<(
    solana_sdk::signature::Signature,
    VersionedTransaction,
    usize,
)> {
    let mut all = Vec::with_capacity(instructions.len() + 1);
    all.push(ComputeBudgetInstruction::set_compute_unit_limit(1_400_000));
    all.extend_from_slice(instructions);
    let message = v0::Message::try_compile(
        &payer.pubkey(),
        &all,
        core::slice::from_ref(table),
        client.get_latest_blockhash()?,
    )?;
    let transaction = match extra_signer {
        Some(signer) => {
            VersionedTransaction::try_new(VersionedMessage::V0(message), &[payer, signer])?
        }
        None => VersionedTransaction::try_new(VersionedMessage::V0(message), &[payer])?,
    };
    let wire = bincode::serialize(&transaction)?.len();
    let signature = client.send_and_confirm_transaction(&transaction)?;
    Ok((signature, transaction, wire))
}

fn observation(client: &RpcClient) -> Result<Observation> {
    let slot = client.get_slot()?;
    let clock: solana_sdk::clock::Clock =
        bincode::deserialize(&client.get_account_data(&clock_sysvar())?)?;
    Ok(Observation {
        slot,
        unix_timestamp: clock.unix_timestamp,
        finality: Finality::Finalized,
    })
}

fn observed(client: &RpcClient, at: Observation, key: Pubkey) -> Result<ObservedAccount> {
    match client.get_account(&key) {
        Ok(account) => Ok(ObservedAccount {
            observation: at,
            key,
            owner: account.owner,
            lamports: account.lamports,
            executable: account.executable,
            data: account.data,
        }),
        Err(_) => Ok(ObservedAccount {
            observation: at,
            key,
            owner: system(),
            lamports: 0,
            executable: false,
            data: Vec::new(),
        }),
    }
}

fn balance(data: &[u8], offset: usize) -> u64 {
    data.get(offset..offset + 8)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_le_bytes)
        .unwrap_or(0)
}

fn fractional_poststate(client: &RpcClient, staged: &stage::Staged) -> Result<Value> {
    let offset = 128 + staged.representation_coordinate * 8;
    Ok(json!({
        "shard_mint_supply": balance(&client.get_account_data(&staged.shard_mint)?, 36),
        "holder_token_amount": balance(&client.get_account_data(&staged.holder_token)?, 64),
        "sleeper_token_amount": balance(&client.get_account_data(&staged.sleeper_token)?, 64),
        "actor_native_claims": balance(&client.get_account_data(&staged.actor_position)?, offset),
        "reserve_native_claims": balance(&client.get_account_data(&staged.reserve_position)?, offset),
    }))
}

fn expected_fractional_poststate(expected: stage::ExpectedPoststate) -> Value {
    json!({
        "shard_mint_supply": expected.shard_mint_supply,
        "holder_token_amount": expected.holder_token_amount,
        "sleeper_token_amount": expected.sleeper_token_amount,
        "actor_native_claims": expected.actor_native_claims,
        "reserve_native_claims": expected.reserve_native_claims,
    })
}

fn finalized(
    client: &RpcClient,
    label: &str,
    submitted: &(
        solana_sdk::signature::Signature,
        VersionedTransaction,
        usize,
    ),
    program: Pubkey,
    data: &[u8],
    inner: bool,
    poststate: &Value,
) -> Result<Value> {
    let response: Value = client.send(
        RpcRequest::GetTransaction,
        json!([submitted.0.to_string(), {"encoding": "base64", "commitment": "finalized",
            "maxSupportedTransactionVersion": 0}]),
    )?;
    let slot = response.get("slot").and_then(Value::as_u64).unwrap_or(0);
    let encoded = response
        .get("transaction")
        .and_then(Value::as_array)
        .and_then(|pair| pair.first())
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new(format!("{label} finalized packet missing")))?;
    let packet_bytes = STANDARD.decode(encoded)?;
    let packet: VersionedTransaction = bincode::deserialize(&packet_bytes)?;
    if bincode::serialize(&packet)? != bincode::serialize(&submitted.1)?
        || packet.signatures.first() != Some(&submitted.0)
    {
        return Err(Error::new(format!(
            "{label} finalized packet differs from submitted packet"
        ))
        .into());
    }
    let meta = response
        .get("meta")
        .and_then(Value::as_object)
        .ok_or_else(|| Error::new(format!("{label} finalized metadata missing")))?;
    if meta.get("err").is_none_or(|error| !error.is_null()) {
        return Err(Error::new(format!("{label} finalized metadata reports refusal")).into());
    }
    let mut keys = packet
        .message
        .static_account_keys()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    if let Some(loaded) = meta.get("loadedAddresses").and_then(Value::as_object) {
        for class in ["writable", "readonly"] {
            for address in loaded
                .get(class)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                keys.push(
                    address
                        .as_str()
                        .ok_or_else(|| Error::new("loaded address malformed"))?
                        .to_owned(),
                );
            }
        }
    }
    let resolve = |index: u8| -> Result<String> {
        keys.get(usize::from(index))
            .cloned()
            .ok_or_else(|| Error::new("program index out of range").into())
    };
    let mut matched = 0_usize;
    let mut instructions = Vec::new();
    for instruction in packet.message.instructions() {
        let id = resolve(instruction.program_id_index)?;
        if !inner && id == program.to_string() && instruction.data == data {
            matched += 1;
        }
        instructions.push(json!({"program_id": id, "data_hex": hex_lower(&instruction.data), "location": "top-level"}));
    }
    if let Some(groups) = meta.get("innerInstructions").and_then(Value::as_array) {
        for group in groups {
            for instruction in group
                .get("instructions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let index = instruction
                    .get("programIdIndex")
                    .and_then(Value::as_u64)
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| Error::new("inner program index missing"))?;
                let bytes = decode_base58(
                    instruction
                        .get("data")
                        .and_then(Value::as_str)
                        .ok_or_else(|| Error::new("inner instruction data missing"))?,
                )?;
                let id = resolve(index)?;
                if inner && id == program.to_string() && bytes == data {
                    matched += 1;
                }
                instructions.push(
                    json!({"program_id": id, "data_hex": hex_lower(&bytes), "location": "inner"}),
                );
            }
        }
    }
    if matched != 1 {
        return Err(Error::new(format!(
            "{label} finalized metadata matched {matched} native instructions"
        ))
        .into());
    }
    Ok(json!({
        "label": label, "signature": submitted.0.to_string(), "slot": slot,
        "wire_bytes": submitted.2, "native_program": program.to_string(),
        "native_data_hex": hex_lower(data), "native_location": if inner {"inner"} else {"top-level"},
        "fee_lamports": meta.get("fee"), "compute_units_consumed": meta.get("computeUnitsConsumed"),
        "logs": meta.get("logMessages"), "instructions": instructions, "poststate": poststate,
    }))
}

fn decode_base58(value: &str) -> Result<Vec<u8>> {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let zeros = value.bytes().take_while(|byte| *byte == b'1').count();
    let mut decoded = Vec::<u8>::new();
    for byte in value.bytes() {
        let digit = ALPHABET
            .iter()
            .position(|candidate| *candidate == byte)
            .ok_or_else(|| Error::new("instruction data was not base58"))?;
        let mut carry = digit;
        for held in decoded.iter_mut().rev() {
            let expanded = usize::from(*held) * 58 + carry;
            *held = u8::try_from(expanded & 0xff)?;
            carry = expanded >> 8;
        }
        while carry > 0 {
            decoded.insert(0, u8::try_from(carry & 0xff)?);
            carry >>= 8;
        }
    }
    let mut answer = vec![0; zeros];
    answer.extend(decoded);
    Ok(answer)
}

fn protocol<T, E: core::fmt::Debug>(value: core::result::Result<T, E>, label: &str) -> Result<T> {
    value.map_err(|error| Error::new(format!("{label}: {error:?}")).into())
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &stage::loader()).0
}

fn run(elf_dir: &Path, out: &Path, keep: bool, terminal_action: TerminalAction) -> Result<()> {
    fs::create_dir_all(out)?;
    let claims = read_elf(elf_dir, "dclutch_claims_sbf.so")?;
    let registry = read_elf(elf_dir, "dclutch_registry_sbf.so")?;
    let core = read_elf(elf_dir, "dclutch_core_sbf.so")?;
    let custody = read_elf(elf_dir, "dclutch_custody_sbf.so")?;
    let caller = read_elf(elf_dir, "dclutch_fractional_compaction_test_caller_sbf.so")?;
    let resolution = read_elf(elf_dir, "dclutch_resolution_proof_sbf.so")?;
    let token = read_elf(elf_dir, "spl_token_2022.so")?;
    let elves = stage::Elves {
        claims: &claims,
        registry: &registry,
        core: &core,
        custody: &custody,
        caller: &caller,
    };
    let actor = keypair_from_seed(&ACTOR_SEED).map_err(|error| Error::new(error.to_string()))?;
    let sleeper =
        keypair_from_seed(&SLEEPER_SEED).map_err(|error| Error::new(error.to_string()))?;
    let staged = match terminal_action {
        TerminalAction::ZeroBurn => {
            stage::stage_terminal(&elves, &resolution, actor.pubkey(), sleeper.pubkey())
        }
        TerminalAction::Redeem => {
            stage::stage_terminal_redeem(&elves, &resolution, actor.pubkey(), sleeper.pubkey())
        }
    };
    prepare(out, &staged, &staged_programs(&elves, &resolution, &token))?;

    let validator = validator::Validator::start_on(out, RPC_PORT, "")?;
    let client = RpcClient::new_with_commitment(
        format!("http://127.0.0.1:{RPC_PORT}"),
        CommitmentConfig::finalized(),
    );
    let outcome = execute(&client, out, &staged, &actor, &sleeper, terminal_action);
    if keep {
        println!("validator left running on port {RPC_PORT}");
    } else {
        validator.stop();
    }
    outcome
}

fn execute(
    client: &RpcClient,
    out: &Path,
    staged: &stage::TerminalStaged,
    actor: &Keypair,
    sleeper: &Keypair,
    terminal_action: TerminalAction,
) -> Result<()> {
    validator::await_health(client)?;
    let genesis_market = protocol(
        CoreState::decode(&client.get_account_data(&staged.base.market)?),
        "genesis Core Market",
    )?;
    let genesis_source = protocol(
        SourceResolutionStateV2::decode(&client.get_account_data(&staged.source_state)?),
        "genesis Source",
    )?;
    if genesis_market.phase != Phase::Open
        || genesis_market.terminal_receipt.is_some()
        || genesis_market.terminal_winner != 0
        || genesis_market.outstanding_capabilities != 2
        || genesis_source.phase() != SourceResolutionPhaseV1::Primary
        || client.get_account(&staged.certificate).is_ok()
        || client.get_account(&staged.custody_replay).is_ok()
    {
        return Err(Error::new("genesis boundary was not exactly preterminal").into());
    }
    let payer = Keypair::new();
    await_airdrop(client, &payer)?;
    let mut addresses = staged
        .base
        .accounts
        .iter()
        .map(|account| account.key)
        .collect::<Vec<_>>();
    addresses.extend([
        stage::CLAIMS,
        stage::REGISTRY,
        stage::CORE,
        stage::CUSTODY,
        stage::CALLER,
        stage::RESOLUTION,
        stage::token_program(),
        programdata(stage::CLAIMS),
        programdata(stage::CORE),
        programdata(stage::CALLER),
        programdata(stage::RESOLUTION),
        system(),
        rent_sysvar(),
        clock_sysvar(),
        staged.certificate,
        staged.custody_replay,
    ]);
    addresses.sort();
    addresses.dedup();
    addresses
        .retain(|key| *key != payer.pubkey() && *key != actor.pubkey() && *key != sleeper.pubkey());
    let table = create_table(client, &payer, addresses)?;
    let mut transactions = Vec::new();

    for action in &staged.base.actions {
        let instruction = Instruction {
            program_id: action.program,
            accounts: action
                .metas
                .iter()
                .map(|meta| AccountMeta {
                    pubkey: meta.key,
                    is_signer: meta.signer,
                    is_writable: meta.writable,
                })
                .collect(),
            data: action.data.clone(),
        };
        let signer = if action
            .metas
            .iter()
            .any(|meta| meta.key == actor.pubkey() && meta.signer)
        {
            Some(actor)
        } else {
            None
        };
        let submitted = submit_v0(
            client,
            &payer,
            signer,
            &table,
            core::slice::from_ref(&instruction),
        )?;
        let state = fractional_poststate(client, &staged.base)?;
        if state != expected_fractional_poststate(action.expected) {
            return Err(Error::new(format!(
                "{} exact Fractional poststate mismatch",
                action.name
            ))
            .into());
        }
        let (program, data, inner) = if action.program == stage::CALLER {
            (
                stage::CLAIMS,
                action
                    .data
                    .get(1..)
                    .ok_or_else(|| Error::new("caller request omitted"))?,
                true,
            )
        } else {
            (action.program, action.data.as_slice(), false)
        };
        transactions.push(finalized(
            client,
            action.name,
            &submitted,
            program,
            data,
            inner,
            &state,
        )?);
    }

    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &staged.base.release_set],
        &stage::REGISTRY,
    )
    .0;
    let product_pair = pair(PRODUCT_RECORD_SCHEMA_ID_V2, staged.base.product);
    let product = protocol(
        ProductRecordV2::decode(&client.get_account_data(&product_pair.0)?),
        "Product record",
    )?;
    let domain_pair = pair(
        RESULT_DOMAIN_SCHEMA_ID_V2,
        product.result_domain_digest().to_bytes(),
    );
    let portfolio_pair = pair(
        PORTFOLIO_SCHEMA_ID_V2,
        product.portfolio_digest().to_bytes(),
    );
    let window_value = protocol(
        dclutch_source::SourceMaterialV3::decode(&staged.source_material.bytes),
        "Source material",
    )?;
    let window_pair = pair(
        WINDOW_SPEC_SCHEMA_ID_V1,
        window_value.window_spec().to_bytes(),
    );
    let deadline = build_commit_deadline_failure_v1(
        &DeadlineFailureCoordinatesV1 {
            worker: payer.pubkey(),
            market: staged.base.market,
            core_program: stage::CORE,
            activation,
            resolution_program: stage::RESOLUTION,
            source_state: staged.source_state,
            material: FinalizedPairV1 {
                raw: staged.source_material.raw,
                staging: staged.source_material.staging,
            },
            window: FinalizedPairV1 {
                raw: window_pair.0,
                staging: window_pair.1,
            },
            product: FinalizedPairV1 {
                raw: product_pair.0,
                staging: product_pair.1,
            },
            result_domain: FinalizedPairV1 {
                raw: domain_pair.0,
                staging: domain_pair.1,
            },
            portfolio: FinalizedPairV1 {
                raw: portfolio_pair.0,
                staging: portfolio_pair.1,
            },
            manifest: FinalizedPairV1 {
                raw: staged.capability_manifest.raw,
                staging: staged.capability_manifest.staging,
            },
            funding_ledger: staged.funding_ledger,
        },
        staged.base.market_state_generation()?,
        staged.terminal_sequence,
    )
    .map_err(|error| Error::new(format!("deadline failure builder: {error:?}")))?;
    let certificate_rent = solana_program::rent::Rent::default()
        .minimum_balance(dclutch_source::resolution::RESOLUTION_CERTIFICATE_BYTES_V2);
    let failure_instructions = [
        transfer(
            payer.pubkey(),
            staged.funding_ledger,
            staged.funding_principal,
        ),
        transfer(payer.pubkey(), staged.certificate, certificate_rent),
        deadline.instruction.clone(),
    ];
    let submitted = submit_v0(client, &payer, None, &table, &failure_instructions)?;
    let source_after = protocol(
        SourceResolutionStateV2::decode(&client.get_account_data(&staged.source_state)?),
        "Source poststate",
    )?;
    let certificate = protocol(
        ResolutionCertificateV2::decode(&client.get_account_data(&staged.certificate)?),
        "Resolution certificate",
    )?;
    if source_after.phase() != SourceResolutionPhaseV1::FailureCommitted
        || certificate.kind != ResolutionCertificateKindV2::ResolutionFailure
        || certificate.receipt_account != staged.certificate.to_bytes()
        || certificate.work_paid == 0
    {
        return Err(Error::new("deadline walk did not produce the exact failure terminal").into());
    }
    let failure_state = json!({
        "source_phase": format!("{:?}", source_after.phase()),
        "certificate": staged.certificate.to_string(),
        "certificate_kind": format!("{:?}", certificate.kind),
        "selector": certificate.selector,
        "work_paid": certificate.work_paid,
    });
    transactions.push(finalized(
        client,
        "resolution-deadline-failure",
        &submitted,
        stage::RESOLUTION,
        &deadline.instruction.data,
        false,
        &failure_state,
    )?);

    let at = observation(client)?;
    let snapshot = ResolutionAdmitTerminalSnapshotV3 {
        market: observed(client, at, staged.base.market)?,
        activation_cache: observed(client, at, activation)?,
        registry_program: observed(client, at, stage::REGISTRY)?,
        core_program: observed(client, at, stage::CORE)?,
        core_programdata: observed(client, at, programdata(stage::CORE))?,
        resolution_program: observed(client, at, stage::RESOLUTION)?,
        resolution_programdata: observed(client, at, programdata(stage::RESOLUTION))?,
        source_material: observed(client, at, staged.source_material.raw)?,
        source_material_staging: observed(client, at, staged.source_material.staging)?,
        capability_manifest: observed(client, at, staged.capability_manifest.raw)?,
        capability_manifest_staging: observed(client, at, staged.capability_manifest.staging)?,
        source_state: observed(client, at, staged.source_state)?,
        funding_ledger: observed(client, at, staged.funding_ledger)?,
        certificate: observed(client, at, staged.certificate)?,
        rent_sysvar: observed(client, at, rent_sysvar())?,
        product_raw: observed(client, at, product_pair.0)?,
        product_staging: observed(client, at, product_pair.1)?,
        result_domain_raw: observed(client, at, domain_pair.0)?,
        result_domain_staging: observed(client, at, domain_pair.1)?,
        portfolio_raw: observed(client, at, portfolio_pair.0)?,
        portfolio_staging: observed(client, at, portfolio_pair.1)?,
    };
    let admit = build_resolution_admit_terminal_v3(&snapshot)
        .map_err(|error| Error::new(format!("admit terminal builder: {error:?}")))?;
    validate_resolution_admit_terminal_report_v3(&admit)
        .map_err(|error| Error::new(format!("admit terminal report: {error:?}")))?;
    let submitted = submit_v0(
        client,
        &payer,
        None,
        &table,
        core::slice::from_ref(&admit.instruction),
    )?;
    let core_after = protocol(
        CoreState::decode(&client.get_account_data(&staged.base.market)?),
        "Core terminal poststate",
    )?;
    if core_after.phase != Phase::Terminal
        || core_after.terminal_receipt.map(|value| value.to_bytes())
            != Some(staged.certificate.to_bytes())
        || core_after.terminal_winner != certificate.selector
    {
        return Err(Error::new("Core did not admit the produced certificate exactly").into());
    }
    let terminal_state = json!({
        "core_phase": format!("{:?}", core_after.phase),
        "terminal_receipt": staged.certificate.to_string(),
        "terminal_winner": core_after.terminal_winner,
    });
    transactions.push(finalized(
        client,
        "core-admit-terminal",
        &submitted,
        stage::CORE,
        &admit.instruction.data,
        false,
        &terminal_state,
    )?);

    let aggregate_bytes = client.get_account_data(&staged.base.aggregate)?;
    let aggregate = protocol(
        LiabilityBasisMarketViewV2::decode(&aggregate_bytes),
        "Claims aggregate",
    )?;
    let rent: solana_sdk::rent::Rent =
        bincode::deserialize(&client.get_account_data(&rent_sysvar())?)?;
    let replay_request = expected_request_v1(
        aggregate,
        stage::CLAIMS.to_bytes(),
        payer.pubkey().to_bytes(),
        core_after.rent_beneficiary.to_bytes(),
        rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1),
    )
    .map_err(|error| Error::new(format!("Claims replay request: {error:?}")))?;
    let request_bytes = protocol(replay_request.to_bytes(), "Custody replay request")?;
    let authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            protocol(
                ContentId::new(replay_request.release_set),
                "release-set identity",
            )?,
            replay_request.market,
            ExecutionRoleV1::Claims,
            replay_request.context,
            hash(&request_bytes).to_bytes(),
        )
        .map_err(|error| Error::new(format!("Claims caller seeds: {error:?}")))?
        .as_slices(),
        &stage::CLAIMS,
    )
    .0;
    let expected_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(replay_request).as_slices(),
        &stage::CUSTODY,
    )
    .0;
    if expected_replay != staged.custody_replay {
        return Err(Error::new("replay derivation drift").into());
    }
    let realm_pair = pair(
        dclutch_market::realm::REALM_SCHEMA_RELEASE_ID_V1,
        staged.base.realm,
    );
    let replay_instruction = Instruction {
        program_id: stage::CLAIMS,
        accounts: vec![
            AccountMeta::new_readonly(authority, false),
            AccountMeta::new_readonly(staged.base.market, false),
            AccountMeta::new_readonly(activation, false),
            AccountMeta::new_readonly(stage::REGISTRY, false),
            AccountMeta::new_readonly(stage::CLAIMS, false),
            AccountMeta::new_readonly(programdata(stage::CLAIMS), false),
            AccountMeta::new_readonly(realm_pair.0, false),
            AccountMeta::new_readonly(realm_pair.1, false),
            AccountMeta::new(staged.custody_replay, false),
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(system(), false),
            AccountMeta::new_readonly(rent_sysvar(), false),
            AccountMeta::new(core_after.rent_beneficiary.to_bytes().into(), false),
            AccountMeta::new_readonly(stage::CUSTODY, false),
            AccountMeta::new_readonly(staged.base.aggregate, false),
        ],
        data: protocol(
            ClaimsCustodyReplayRequestV1::new(staged.base.market.to_bytes()),
            "Claims replay wire",
        )?
        .to_bytes()
        .to_vec(),
    };
    let submitted = submit_v0(
        client,
        &payer,
        None,
        &table,
        core::slice::from_ref(&replay_instruction),
    )?;
    let replay_before_terminal = client.get_account_data(&staged.custody_replay)?;
    let replay = protocol(
        CustodyReplayV1::decode(&replay_before_terminal),
        "Claims replay poststate",
    )?;
    if replay.caller_role != CallerRoleV1::Claims || replay.next_revision != 1 {
        return Err(Error::new("real replay creation did not produce Claims revision 1").into());
    }
    let replay_state = json!({"caller_role": format!("{:?}", replay.caller_role), "next_revision": replay.next_revision});
    transactions.push(finalized(
        client,
        "claims-custody-replay",
        &submitted,
        stage::CLAIMS,
        &replay_instruction.data,
        false,
        &replay_state,
    )?);

    let terms_pair = pair(FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, staged.base.terms);
    let terms_bytes = client.get_account_data(&terms_pair.0)?;
    let terms = protocol(
        FractionalExposureTermsV2::decode(
            &terms_bytes,
            FractionalExposureTermsAdmissionV2 {
                selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                selected_terms_id: staged.base.terms,
                finalized_terms_id: staged.base.terms,
                recomputed_terms_digest: hash(&terms_bytes).to_bytes(),
                finalized_terms_digest: staged.base.terms,
                record_authenticated: true,
            },
        ),
        "Fractional terms",
    )?;
    let root_bytes = client.get_account_data(&staged.base.root)?;
    let root: FractionalCapabilityRootV4 = decode_fractional_capability_root_v4(&root_bytes)
        .ok_or_else(|| Error::new("Fractional root did not decode"))?;
    let request = protocol(
        FractionalExposureRequestV2::new(
            terminal_action.protocol(),
            FractionalExposureRequestInputV2 {
                release_set: staged.base.release_set,
                market: staged.base.market.to_bytes(),
                product_record: staged.base.product,
                result_domain: product.result_domain_digest().to_bytes(),
                terms: staged.base.terms,
                token_behavior: hash(
                    &protocol(
                        dclutch_custody::token_svm::TokenBehaviorSelectionV2::new(
                            staged.base.realm,
                            staged.base.release_set,
                        ),
                        "Token behavior",
                    )?
                    .to_bytes(),
                )
                .to_bytes(),
                exposure: terms.exposure_id(),
                owner: sleeper.pubkey().to_bytes(),
                source_token_account: staged.base.sleeper_token.to_bytes(),
                destination_token_account: [0; 32],
                terminal_digest: staged.certificate.to_bytes(),
                expected_revision: 1,
                quantity: staged.base.sleeper_shards,
                representation_coordinate: u32::try_from(staged.base.representation_coordinate)?,
            },
        ),
        terminal_action.label(),
    )?;
    let request_data = protocol(request.to_bytes(), terminal_action.label())?;
    let caller_authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            staged.base.release_set,
            staged.base.market.to_bytes(),
            ExecutionRoleV1::Trading,
            staged.base.terms,
            hash(&request_data).to_bytes(),
        )
        .map_err(|error| Error::new(format!("Trading caller seeds: {error:?}")))?
        .as_slices(),
        &stage::CALLER,
    )
    .0;
    let basis_pair = pair(
        dclutch_product::payoff::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        staged.base.product_basis,
    );
    let behavior_digest = hash(
        &protocol(
            dclutch_custody::token_svm::TokenBehaviorSelectionV2::new(
                staged.base.realm,
                staged.base.release_set,
            ),
            "Token behavior",
        )?
        .to_bytes(),
    )
    .to_bytes();
    let behavior_pair = pair(
        dclutch_custody::token_svm::TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        behavior_digest,
    );
    let exposure_pair = pair(
        dclutch_claims::composition::COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
        staged.base.exposure,
    );
    let exposure_bytes = client.get_account_data(&exposure_pair.0)?;
    let child = vec![
        AccountMeta::new_readonly(caller_authority, true),
        AccountMeta::new(staged.base.aggregate, false),
        AccountMeta::new_readonly(basis_pair.0, false),
        AccountMeta::new_readonly(basis_pair.1, false),
        AccountMeta::new_readonly(product_pair.0, false),
        AccountMeta::new_readonly(product_pair.1, false),
        AccountMeta::new_readonly(domain_pair.0, false),
        AccountMeta::new_readonly(domain_pair.1, false),
        AccountMeta::new_readonly(portfolio_pair.0, false),
        AccountMeta::new_readonly(portfolio_pair.1, false),
        AccountMeta::new_readonly(rent_sysvar(), false),
        AccountMeta::new_readonly(staged.base.market, false),
        AccountMeta::new_readonly(activation, false),
        AccountMeta::new_readonly(stage::REGISTRY, false),
        AccountMeta::new_readonly(stage::CALLER, false),
        AccountMeta::new_readonly(programdata(stage::CALLER), false),
        AccountMeta::new_readonly(stage::CLAIMS, false),
        AccountMeta::new_readonly(programdata(stage::CLAIMS), false),
        AccountMeta::new_readonly(stage::CORE, false),
        AccountMeta::new_readonly(programdata(stage::CORE), false),
        AccountMeta::new(staged.base.reserve_position, false),
        AccountMeta::new_readonly(exposure_pair.0, false),
        AccountMeta::new_readonly(exposure_pair.1, false),
        AccountMeta::new_readonly(stage::CLAIMS, false),
        AccountMeta::new_readonly(stage::CUSTODY, false),
        AccountMeta::new_readonly(staged.certificate, false),
        AccountMeta::new_readonly(stage::RESOLUTION, false),
        AccountMeta::new_readonly(programdata(stage::RESOLUTION), false),
        AccountMeta::new_readonly(realm_pair.0, false),
        AccountMeta::new_readonly(realm_pair.1, false),
        AccountMeta::new(staged.custody_replay, false),
        AccountMeta::new_readonly(stage::COLLATERAL_MINT, false),
        AccountMeta::new(staged.hoard, false),
        AccountMeta::new(staged.recipient_token, false),
        AccountMeta::new_readonly(staged.custody_authority, false),
        AccountMeta::new_readonly(stage::token_program(), false),
        AccountMeta::new_readonly(terms_pair.0, false),
        AccountMeta::new_readonly(terms_pair.1, false),
        AccountMeta::new_readonly(behavior_pair.0, false),
        AccountMeta::new_readonly(behavior_pair.1, false),
        AccountMeta::new(staged.base.root, true),
        AccountMeta::new_readonly(sleeper.pubkey(), true),
        AccountMeta::new(staged.base.shard_mint, false),
        AccountMeta::new(staged.base.sleeper_token, false),
    ];
    let native = build_fractional_terminal_atomic_claims_instruction_v3(
        request,
        terms,
        root,
        &exposure_bytes,
        &child,
    )
    .map_err(|error| Error::new(format!("terminal Claims builder: {error:?}")))?;
    let mut outer_accounts = vec![AccountMeta::new_readonly(stage::CLAIMS, false)];
    outer_accounts.extend(child.iter().enumerate().map(|(index, account)| {
        let pda_signer = index == 0 || index == FRACTIONAL_TERMINAL_ROOT_V3;
        AccountMeta {
            pubkey: account.pubkey,
            is_signer: account.is_signer && !pda_signer,
            is_writable: account.is_writable,
        }
    }));
    let mut wrapper = vec![0];
    wrapper.extend_from_slice(&request_data);
    let outer = Instruction {
        program_id: stage::CALLER,
        accounts: outer_accounts,
        data: wrapper,
    };
    let submitted = submit_v0(
        client,
        &payer,
        Some(sleeper),
        &table,
        core::slice::from_ref(&outer),
    )?;
    let after = fractional_poststate(client, &staged.base)?;
    let replay_after = client.get_account_data(&staged.custody_replay)?;
    let terminal_expected = json!({
        "shard_mint_supply": 0,
        "holder_token_amount": 0,
        "sleeper_token_amount": 0,
        "actor_native_claims": stage::WHOLE_UNWRAP_EXPECTED.actor_native_claims,
        "reserve_native_claims": 0,
    });
    let collateral_supply = balance(&client.get_account_data(&stage::COLLATERAL_MINT)?, 36);
    let hoard_amount = balance(&client.get_account_data(&staged.hoard)?, 64);
    let recipient_amount = balance(&client.get_account_data(&staged.recipient_token)?, 64);
    let replay_after_state = protocol(
        CustodyReplayV1::decode(&replay_after),
        "terminal Claims replay poststate",
    )?;
    let replay_immutable_matches = replay_after_state.caller_role == replay.caller_role
        && replay_after_state.release_set == replay.release_set
        && replay_after_state.market == replay.market
        && replay_after_state.realm == replay.realm
        && replay_after_state.context == replay.context
        && replay_after_state.caller_program == replay.caller_program
        && replay_after_state.rent_refund == replay.rent_refund
        && replay_after_state.open_vault_count == replay.open_vault_count
        && replay_after_state.generation == replay.generation;
    let replay_effect_matches = match terminal_action {
        TerminalAction::ZeroBurn => replay_after == replay_before_terminal,
        TerminalAction::Redeem => replay_immutable_matches && replay_after_state.next_revision == 2,
    };
    let expected_collateral_supply = staged.initial_collateral_supply;
    let expected_hoard = staged
        .initial_collateral_supply
        .checked_sub(staged.terminal_payout)
        .ok_or_else(|| Error::new("terminal payout exceeded staged collateral"))?;
    let expected_recipient = match terminal_action {
        TerminalAction::ZeroBurn => 0,
        TerminalAction::Redeem => staged.terminal_payout,
    };
    if after != terminal_expected
        || collateral_supply != expected_collateral_supply
        || hoard_amount != expected_hoard
        || recipient_amount != expected_recipient
        || !replay_effect_matches
    {
        return Err(Error::new(format!(
            "{} exact poststate mismatch",
            terminal_action.label()
        ))
        .into());
    }
    let terminal_poststate = json!({
        "scope": match terminal_action {
            TerminalAction::ZeroBurn => "losing-holder-zero-burn",
            TerminalAction::Redeem => "winner-reserve-slice-only",
        },
        "fractional": after,
        "initial_collateral_supply": staged.initial_collateral_supply,
        "reserve_slice_payout": staged.terminal_payout,
        "collateral_mint_supply": collateral_supply,
        "hoard_token_amount": hoard_amount,
        "recipient_token_amount": recipient_amount,
        "custody_replay_hex": hex_lower(&replay_after),
        "custody_replay_next_revision": replay_after_state.next_revision,
    });
    transactions.push(finalized(
        client,
        terminal_action.label(),
        &submitted,
        stage::CLAIMS,
        &native.data,
        true,
        &terminal_poststate,
    )?);

    let evidence = json!({
        "schema": "dclutch/fractional-terminal-validator-evidence/v1",
        "genesis_boundary": {"market_phase": "Open", "source_phase": "Primary",
            "outstanding_capabilities": genesis_market.outstanding_capabilities,
            "terminal_receipt": null, "terminal_winner": 0, "terminal_planted": false,
            "certificate_planted": false, "custody_replay_planted": false},
        "market": staged.base.market.to_string(),
        "source_state": staged.source_state.to_string(),
        "certificate": staged.certificate.to_string(),
        "release_set": hex_lower(&staged.base.release_set),
        "terminal": terminal_state,
        "terminal_action": terminal_action.label(),
        "terminal_poststate": terminal_poststate,
        "custody_replay_effect_matches": replay_effect_matches,
        "transactions": transactions,
    });
    fs::write(
        out.join("terminal-evidence.json"),
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )?;
    Ok(())
}

trait MarketGeneration {
    fn market_state_generation(&self) -> Result<u64>;
}

impl MarketGeneration for stage::Staged {
    fn market_state_generation(&self) -> Result<u64> {
        let account = self
            .accounts
            .iter()
            .find(|account| account.key == self.market)
            .ok_or_else(|| Error::new("staged Market missing"))?;
        Ok(
            protocol(CoreState::decode(&account.data), "staged Core Market")?
                .identity
                .generation,
        )
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() -> ExitCode {
    let outcome = parse().and_then(|(elf, out, keep, action)| run(&elf, &out, keep, action));
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dclutch-fractional-terminal: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(accounts: &Path, key: Pubkey) -> PathBuf {
        accounts.join(format!("{key}.json"))
    }

    #[test]
    fn prepare_preserves_finalized_staging_vacancy() -> Result<()> {
        let claims = [1_u8];
        let registry = [2_u8];
        let core = [3_u8];
        let custody = [4_u8];
        let caller = [5_u8];
        let resolution = [6_u8];
        let elves = stage::Elves {
            claims: &claims,
            registry: &registry,
            core: &core,
            custody: &custody,
            caller: &caller,
        };
        let staged = stage::stage_terminal(
            &elves,
            &resolution,
            Pubkey::new_from_array([0x31; 32]),
            Pubkey::new_from_array([0x32; 32]),
        );
        let out = env::temp_dir().join(format!(
            "dclutch-fractional-terminal-prepare-{}",
            std::process::id()
        ));
        if out.exists() {
            fs::remove_dir_all(&out)?;
        }
        prepare(&out, &staged, &[])?;
        let accounts = out.join("accounts");
        assert!(file(&accounts, staged.source_material.raw).is_file());
        assert!(!file(&accounts, staged.source_material.staging).exists());
        assert!(file(&accounts, staged.capability_manifest.raw).is_file());
        assert!(!file(&accounts, staged.capability_manifest.staging).exists());
        assert!(file(&accounts, staged.source_state).is_file());
        fs::remove_dir_all(out)?;
        Ok(())
    }
}
