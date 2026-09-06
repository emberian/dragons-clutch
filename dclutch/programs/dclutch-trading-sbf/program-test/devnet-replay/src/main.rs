//! Replay one captured chain transaction inside `ProgramTest`, byte for byte.
//!
//! # Why this exists
//!
//! `TradingSbfError::Content` is one wire code over thousands of sites, and the
//! tree's own instrument -- `hot_cu_checkpoint!` and the `hot-cu-profile`
//! `dclutch-hot-why:` lines -- cannot be pointed at a release-pinned chain: a
//! release pins the ELF digest AND the deployment slot, so an instrumented
//! build refuses before it can print a checkpoint. That is what made
//! cohort-16.1's `OpenBatch` refusal "could not be localized".
//!
//! It is only a wall on a chain. A hot route never hashes an ELF
//! (`crates/dclutch-trading/src/shadow_accelerator_auth/deployment.rs:97`
//! takes the activation-bound arm, and `slot_pinned_release_elf_digest_v1`
//! compares the SLOT and the AUTHORITY and returns the release's own recorded
//! digest). So the pins are satisfied by the ProgramData account's 45-byte
//! Loader V3 HEADER, and the ELF tail behind it is free. `--programdata-elf`
//! rewrites exactly that tail.
//!
//! # The recipe
//!
//! 1. capture: the transaction's own wire bytes, every account it names read
//!    at `finalized`, and every lookup table it resolves through;
//! 2. replay: `simulate_transaction`, which is what a chain preflight does --
//!    no signature, no blockhash, no fee;
//! 3. localize: re-run with `--programdata-elf TRADING=<profiled .so>` and read
//!    the checkpoint that precedes the refusal.
//!
//! Step 2 must reproduce the chain's refusal code AND its compute units before
//! step 3 means anything. The replay prints both.

#![forbid(unsafe_code)]
#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use std::{collections::BTreeMap, env, fs, path::PathBuf, process::ExitCode, str::FromStr};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::Value;
use solana_account::Account;
use solana_clock::Clock;
use solana_program_test::ProgramTest;
use solana_pubkey::Pubkey;
use solana_transaction::versioned::VersionedTransaction;

const USAGE: &str = "\
dclutch-devnet-frame-replay --capture CAPTURE_JSON [options]

  --capture PATH              a dclutch-devnet-frame-capture-v1 document
  --programdata-elf KEY=PATH  replace the ELF tail of the ProgramData account
                              KEY, keeping its 45-byte Loader V3 header (the
                              deployment slot and upgrade authority every
                              release pin compares) byte for byte
  --set-account KEY=PATH      replace the whole data of account KEY with a file
  --programdata-slot KEY=N    rewrite the Loader V3 deployment slot of the
                              ProgramData account KEY. THE HARNESS WALL: a
                              program's deployment slot must sit inside
                              `BankForks::root()..=highest_slot()` or the
                              program cache reloads it forever and the
                              transaction dies `ProgramCacheHitMaxLimit` with no
                              logs. `ProgramTestContext::warp_to_slot(w)` roots
                              the fork at `w - 1`, so a captured devnet slot is
                              always below the root. Slot ZERO is the escape:
                              `extract` admits `deployment_slot <=
                              latest_root_slot` before it consults the fork
                              graph at all. Rewriting the slot MOVES the release
                              pin, so a `ReleaseSuperseded` after this flag is
                              the instrument talking and not the frame.
  --expect-units N            refuse unless the replay consumes exactly N units
  --expect-error TEXT         refuse unless the debug form of the refusal
                              contains TEXT
  --log-filter TEXT           print only log lines containing TEXT
";

fn die(message: impl AsRef<str>) -> ! {
    eprintln!("devnet-frame-replay: {}", message.as_ref());
    std::process::exit(2)
}

fn address(value: &str) -> Pubkey {
    Pubkey::from_str(value).unwrap_or_else(|_| die(format!("not an address: {value}")))
}

fn bytes(document: &Value, key: &str) -> Vec<u8> {
    BASE64
        .decode(
            document
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_else(|| die(format!("capture has no base64 field {key}"))),
        )
        .unwrap_or_else(|_| die(format!("field {key} is not base64")))
}

struct ArgumentsV1 {
    capture: PathBuf,
    programdata_elf: Vec<(Pubkey, PathBuf)>,
    set_account: Vec<(Pubkey, PathBuf)>,
    programdata_slot: Vec<(Pubkey, u64)>,
    expect_units: Option<u64>,
    expect_error: Option<String>,
    log_filter: Option<String>,
}

fn pair(value: &str) -> (Pubkey, PathBuf) {
    let (key, path) = value
        .split_once('=')
        .unwrap_or_else(|| die(format!("expected ADDRESS=PATH, got {value}")));
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        die(format!("{} must be an absolute path", path.display()));
    }
    (address(key), path)
}

fn parse_arguments(arguments: Vec<String>) -> ArgumentsV1 {
    let mut parsed = ArgumentsV1 {
        capture: PathBuf::new(),
        programdata_elf: Vec::new(),
        set_account: Vec::new(),
        programdata_slot: Vec::new(),
        expect_units: None,
        expect_error: None,
        log_filter: None,
    };
    let mut rest = arguments.into_iter();
    while let Some(flag) = rest.next() {
        let mut value = || {
            rest.next()
                .unwrap_or_else(|| die(format!("{flag} needs a value")))
        };
        match flag.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0)
            }
            "--capture" => parsed.capture = PathBuf::from(value()),
            "--programdata-elf" => parsed.programdata_elf.push(pair(&value())),
            "--set-account" => parsed.set_account.push(pair(&value())),
            "--programdata-slot" => {
                let raw = value();
                let (key, slot) = raw
                    .split_once('=')
                    .unwrap_or_else(|| die(format!("expected ADDRESS=SLOT, got {raw}")));
                parsed.programdata_slot.push((
                    address(key),
                    slot.parse()
                        .unwrap_or_else(|_| die("--programdata-slot needs a number")),
                ));
            }
            "--expect-units" => {
                parsed.expect_units = Some(
                    value()
                        .parse()
                        .unwrap_or_else(|_| die("--expect-units needs a number")),
                );
            }
            "--expect-error" => parsed.expect_error = Some(value()),
            "--log-filter" => parsed.log_filter = Some(value()),
            other => die(format!("unknown flag: {other}\n\n{USAGE}")),
        }
    }
    if parsed.capture.as_os_str().is_empty() {
        die(format!("--capture is required\n\n{USAGE}"));
    }
    parsed
}

/// The Loader V3 ProgramData header: variant tag, deployment slot, and the
/// upgrade-authority option with its key slot.
///
/// `waist::programdata_v2` writes it and `ProgramDataV3View::parse` reads it;
/// the two numbers a release pin compares live entirely inside it.
const PROGRAMDATA_HEADER_BYTES_V3: usize = 45;

/// `SysvarC1ock11111111111111111111111111111111`, read for its first eight
/// bytes so the replay never couples to one `solana-clock` version.
/// `BPFLoaderUpgradeab1e11111111111111111111111`.
const LOADER_V3_V1: Pubkey = Pubkey::new_from_array([
    2, 168, 246, 145, 78, 136, 161, 176, 226, 16, 21, 62, 247, 99, 174, 43, 0, 194, 185, 61, 22,
    193, 36, 210, 192, 83, 122, 16, 4, 128, 0, 0,
]);

const CLOCK_SYSVAR_V1: Pubkey = Pubkey::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);

fn main() -> ExitCode {
    let arguments = parse_arguments(env::args().skip(1).collect());
    let capture: Value = serde_json::from_slice(
        &fs::read(&arguments.capture).unwrap_or_else(|error| die(format!("--capture: {error}"))),
    )
    .unwrap_or_else(|error| die(format!("--capture is not JSON: {error}")));
    let schema = capture.get("schema").and_then(Value::as_str).unwrap_or("");
    if schema != "dclutch-devnet-frame-capture-v1" {
        die(format!(
            "capture schema is {schema:?}, not dclutch-devnet-frame-capture-v1"
        ));
    }

    let mut accounts: BTreeMap<Pubkey, Account> = BTreeMap::new();
    let state = capture
        .get("state")
        .and_then(Value::as_object)
        .unwrap_or_else(|| die("capture has no state object"));
    for (key, value) in state {
        let data = BASE64
            .decode(
                value["dataBase64"]
                    .as_str()
                    .unwrap_or_else(|| die("dataBase64")),
            )
            .unwrap_or_else(|_| die("dataBase64 is not base64"));
        accounts.insert(
            address(key),
            Account {
                lamports: value["lamports"]
                    .as_u64()
                    .unwrap_or_else(|| die("lamports")),
                data,
                owner: address(value["owner"].as_str().unwrap_or_else(|| die("owner"))),
                executable: value["executable"].as_bool().unwrap_or(false),
                rent_epoch: value["rentEpoch"].as_u64().unwrap_or(0),
            },
        );
    }

    for (key, path) in &arguments.programdata_elf {
        let elf =
            fs::read(path).unwrap_or_else(|error| die(format!("{}: {error}", path.display())));
        let account = accounts
            .get_mut(key)
            .unwrap_or_else(|| die(format!("capture holds no account {key}")));
        if account.data.len() < PROGRAMDATA_HEADER_BYTES_V3 {
            die(format!("{key} is not a Loader V3 ProgramData account"));
        }
        let header: Vec<u8> = account.data[..PROGRAMDATA_HEADER_BYTES_V3].to_vec();
        let before = account.data.len();
        account.data = [header.as_slice(), elf.as_slice()].concat();
        eprintln!(
            "replay: programdata {key} ELF replaced from {} ({} -> {} bytes, header {} bytes kept)",
            path.display(),
            before,
            account.data.len(),
            PROGRAMDATA_HEADER_BYTES_V3,
        );
    }
    for (key, slot) in &arguments.programdata_slot {
        let account = accounts
            .get_mut(key)
            .unwrap_or_else(|| die(format!("capture holds no account {key}")));
        if account.data.len() < PROGRAMDATA_HEADER_BYTES_V3 {
            die(format!("{key} is not a Loader V3 ProgramData account"));
        }
        let before = u64::from_le_bytes(account.data[4..12].try_into().expect("deployment slot"));
        account.data[4..12].copy_from_slice(&slot.to_le_bytes());
        eprintln!("replay: programdata {key} deployment slot {before} -> {slot}");
    }
    for (key, path) in &arguments.set_account {
        let data =
            fs::read(path).unwrap_or_else(|error| die(format!("{}: {error}", path.display())));
        let account = accounts
            .get_mut(key)
            .unwrap_or_else(|| die(format!("capture holds no account {key}")));
        eprintln!(
            "replay: account {key} data replaced from {} ({} -> {} bytes)",
            path.display(),
            account.data.len(),
            data.len()
        );
        account.data = data;
    }

    let mut transaction: VersionedTransaction =
        bincode::deserialize(&bytes(&capture, "transactionBase64")).unwrap_or_else(|error| {
            die(format!("transactionBase64 is not a transaction: {error}"))
        });
    let warp = capture
        .get("warpSlot")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| die("capture has no warpSlot"));

    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    for (key, account) in &accounts {
        test.add_account(*key, account.clone());
    }

    // THE BANK SLOT IS NOT A FREE CHOICE, and this is the whole harness fact.
    //
    // `ProgramCache::extract` admits a program only when its deployment slot is
    // at or below the cache root OR the fork graph calls it an ancestor, and
    // `BankForks::relationship` answers `Unknown` for anything below the root.
    // `warp_to_slot(w)` roots the fork at `w - 1`. So every captured deployment
    // slot must be at or above `w - 1`, and `w` must be above every deployment
    // slot: the two together force ONE deployment slot D and a bank at D + 1.
    // Otherwise the transaction dies `ProgramCacheHitMaxLimit` with zero logs
    // and zero units, which reads exactly like a program that refused nothing.
    //
    // The chain slot the ROUTE saw is then restored on top, as the Clock
    // sysvar, so the program reads the slot it read on chain while the bank
    // sits where the loader needs it.
    let mut deployment_slots: Vec<u64> = Vec::new();
    for account in accounts.values() {
        if account.owner != LOADER_V3_V1 || account.data.len() < PROGRAMDATA_HEADER_BYTES_V3 {
            continue;
        }
        let variant = u32::from_le_bytes(account.data[..4].try_into().expect("variant tag"));
        if variant != 3 {
            continue;
        }
        let slot = u64::from_le_bytes(account.data[4..12].try_into().expect("deployment slot"));
        deployment_slots.push(slot);
    }
    deployment_slots.sort_unstable();
    deployment_slots.dedup();
    println!("replay: captured deployment slots {deployment_slots:?}");
    let bank_slot = match deployment_slots.as_slice() {
        [] => warp,
        [only] => only.saturating_add(1),
        many => die(format!(
            "the capture's loaded programs carry {} distinct deployment slots {many:?}; \
             equalize them with --programdata-slot (and move each release's own pin with \
             --set-account) before replaying",
            many.len()
        )),
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async move {
        let mut context = test.start_with_context().await;
        context.warp_to_slot(bank_slot).expect("warp the bank");
        let clock_account = context
            .banks_client
            .get_account(CLOCK_SYSVAR_V1)
            .await
            .expect("clock query")
            .expect("clock sysvar account");
        let mut clock: Clock =
            bincode::deserialize(&clock_account.data).expect("clock sysvar body");
        println!(
            "replay: bank slot {} (one past the captured deployment slot)",
            clock.slot
        );
        // `set_sysvar` and not `set_account`: `Clock::get()` is a syscall over
        // the bank's SYSVAR CACHE, which a bare account write does not touch.
        clock.slot = warp;
        context.set_sysvar(&clock);
        let readback = context
            .banks_client
            .get_account(CLOCK_SYSVAR_V1)
            .await
            .expect("clock query")
            .expect("clock sysvar account");
        let observed: Clock = bincode::deserialize(&readback.data).expect("clock sysvar body");
        println!(
            "replay: Clock slot set to {} (capture chain slot {warp})",
            observed.slot
        );
        assert_eq!(
            observed.slot, warp,
            "the Clock sysvar did not take the captured slot"
        );
        println!("replay: accounts installed {}", accounts.len());

        // THE ONE BYTE FIELD THIS REPLAY REWRITES. A capture carries the
        // blockhash the chain's own preflight used, and no bank but that one
        // holds it; `simulate_transaction_unchecked` still checks the hash is
        // in the queue even though it checks no signature. The instruction,
        // its accounts, its privileges and its data are untouched.
        let blockhash = context
            .banks_client
            .get_latest_blockhash()
            .await
            .expect("latest blockhash");
        transaction.message.set_recent_blockhash(blockhash);
        println!("replay: recent blockhash rewritten to the bank's own {blockhash}");
        let simulated = context
            .banks_client
            .simulate_transaction(transaction)
            .await
            .expect("simulate");
        let Some(details) = simulated.simulation_details else {
            println!(
                "replay: no simulation details; result {:?}",
                simulated.result
            );
            return ExitCode::from(1);
        };
        println!("replay: units consumed {}", details.units_consumed);
        println!("replay: result {:?}", simulated.result);
        println!("replay: logs {}", details.logs.len());
        for line in &details.logs {
            match &arguments.log_filter {
                Some(filter) if !line.contains(filter.as_str()) => {}
                _ => println!("  {line}"),
            }
        }
        let mut failed = false;
        if let Some(expected) = arguments.expect_units
            && details.units_consumed != expected
        {
            eprintln!(
                "replay: REFUSED -- expected {expected} units, consumed {}",
                details.units_consumed
            );
            failed = true;
        }
        if let Some(expected) = &arguments.expect_error {
            let observed = format!("{:?}", simulated.result);
            if !observed.contains(expected.as_str()) {
                eprintln!(
                    "replay: REFUSED -- expected error containing {expected:?}, got {observed}"
                );
                failed = true;
            }
        }
        if failed {
            ExitCode::from(1)
        } else {
            ExitCode::SUCCESS
        }
    })
}
