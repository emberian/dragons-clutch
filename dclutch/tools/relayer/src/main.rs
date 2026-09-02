//! `dclutch-relayer` — the `RelayedMainnetStateV1` observation daemon.
//!
//! Run modes are deliberately narrow.  `run --dry-run` observes, signs and
//! writes artifacts and the publication log, and touches no cluster except to
//! read.  `run --submit` additionally sends transactions, and refuses any
//! endpoint that is not loopback unless the operator has set
//! `allow_public_submission = true` under an authorization that names devnet or
//! mainnet submission.

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Args, Parser, Subcommand};

use dclutch_relayer::artifacts::ArtifactWriter;
use dclutch_relayer::config::{Config, endpoint_host_for_display};
use dclutch_relayer::delivery::{
    DeliveryAction, DeliveryExpectation, DeliveryJournal, LaunchExpectation,
    reconcile_finalized_record, require_live_launch_accounts, require_live_lookup_table,
};
use dclutch_relayer::error::{RelayerError, Result};
use dclutch_relayer::id32::{ID_BYTES, base58, parse_id32, to_hex};
use dclutch_relayer::keeper::{CreateRecordKeeperRequest, run_create_record_keeper};
use dclutch_relayer::keys::{AttestationSigner, generate_keypair_file};
use dclutch_relayer::observe::{ObservationCycle, SetWatcher};
use dclutch_relayer::publog::{MessageKind, PublicationLog, RpcReadLog};
use dclutch_relayer::rpc::{RpcClient, base64_encode};
use dclutch_relayer::segments;
use dclutch_relayer::skew::measure_skew;
use dclutch_relayer::submit::require_submission_admitted;
use dclutch_relayer::txn::{
    ComputeBudget, RelayFrameAddresses, RelayInstructionPlan, append_observation_instruction,
    build_relay_transaction_plan, derive_record_address, message_bytes, require_packet_fit,
    seal_record_instruction, serialize_transaction, sign_transaction,
};
use solana_message::AddressLookupTableAccount;

// PROVISIONAL operational bounds: one action occupies at most 5 * 30 seconds
// before it yields a named refusal. The lifting plan is configuration only
// after the loopback/runtime evidence supplies an observed finalization tail;
// neither value is an economic or protocol bound.
const MAX_DELIVERY_SEND_ATTEMPTS: u8 = 5;
const ACK_POLLS_PER_SEND_ATTEMPT: u8 = 30;

#[derive(Parser)]
#[command(
    name = "dclutch-relayer",
    about = "Signs observations of another cluster's account bytes. Never interpretations.",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate a fresh test keypair at a path you name.
    ///
    /// This is the only way this service obtains a key.  It never scans for
    /// wallets, never defaults to `id.json`, and refuses paths inside
    /// `~/.config`, `~/.ssh`, or any `.config/solana` directory.
    Keygen(KeygenArgs),
    /// Validate a config file and print every derived identity.
    ShowConfig(ConfigArgs),
    /// Observe, sign, and write artifacts.
    Run(RunArgs),
    /// Submit the append and seal routes from a dry-run artifact directory.
    ///
    /// The observation record for the artifact's `(market, generation,
    /// account_set_id, observed_slot)` must already exist: record creation is
    /// the keeper's act, and the record's address is seeded by the observed
    /// slot, so in a fresh deployment the honest order is observe (dry run),
    /// create the record for that slot, then submit these exact recorded
    /// bytes.  Re-submitting the recorded observation is the §4.11 rule
    /// applied across processes: re-sign, never re-observe.
    SubmitArtifacts(SubmitArtifactsArgs),
    /// Plan or execute creation of the slot-seeded observation record on devnet.
    ///
    /// The default is read-only: the command authenticates one finalized
    /// 21-account frame and persists an unsigned plan without opening a key.
    /// `--execute` loads only the configured fee payer after that plan exists,
    /// then reauthenticates the unchanged prestate before signing.
    CreateRecord(CreateRecordArgs),
    /// Push the publication log to a local public-serve directory.
    ///
    /// This is the file-target half of §4.11's publication requirement: it
    /// refuses to overwrite a divergent public copy (append-only or nothing),
    /// and writes a `LATEST.json` a verifier can poll.  Serving the directory
    /// is the operator's act; no external service is contacted here.
    ///
    /// The published log is **segmented**: the active segment seals at a size
    /// threshold and is renamed to a number it keeps forever, and each segment
    /// after the first opens with a header carrying its predecessor's SHA-256,
    /// so the segments form a chain a reader can verify a prefix of.  See
    /// `segments.rs`.
    PublishLog(PublishLogArgs),
    /// Verify a served publication-log directory, offline.
    ///
    /// No config, no network, no key: this reads a directory (the one on the
    /// box, or one a stranger downloaded) and checks every property the served
    /// `README.txt` claims — segment digests, the chain fold, slot order, and
    /// that `LATEST.json` describes exactly what is on disk.  With `--against`
    /// it additionally proves every published byte is the local log's byte at
    /// the same offset.
    VerifyLog(VerifyLogArgs),
    /// Measure the maximum |a_now - b_now| between two clusters' Clock sysvars.
    MeasureSkew(SkewArgs),
}

#[derive(Args)]
struct KeygenArgs {
    /// Where to write the keypair.
    #[arg(long)]
    out: PathBuf,
}

#[derive(Args)]
struct ConfigArgs {
    /// Path to the TOML configuration.
    #[arg(long)]
    config: PathBuf,
}

#[derive(Args)]
struct RunArgs {
    /// Path to the TOML configuration.
    #[arg(long)]
    config: PathBuf,
    /// Observe and sign, writing artifacts; submit nothing.
    #[arg(long, conflicts_with = "submit")]
    dry_run: bool,
    /// Additionally submit, subject to the endpoint gate.
    #[arg(long)]
    submit: bool,
    /// How many cycles to run; 0 runs until interrupted.
    #[arg(long, default_value_t = 1)]
    cycles: u32,
}

#[derive(Args)]
struct SubmitArtifactsArgs {
    /// Path to the TOML configuration.
    #[arg(long)]
    config: PathBuf,
    /// One dry-run slot directory: `<output_dir>/artifacts/<set>/slot-<N>/`.
    #[arg(long)]
    slot_dir: PathBuf,
}

#[derive(Args)]
struct CreateRecordArgs {
    /// Path to the TOML configuration naming exact Solana devnet genesis.
    #[arg(long)]
    config: PathBuf,
    /// One dry-run slot directory: `<output_dir>/artifacts/<set>/slot-<N>/`.
    #[arg(long)]
    slot_dir: PathBuf,
    /// Explicit worker/fee-payer public key in base58 or 64 lowercase hex.
    #[arg(long)]
    worker: String,
    /// Load the configured fee payer and submit after persisting the plan.
    #[arg(long, default_value_t = false)]
    execute: bool,
}

#[derive(Args)]
struct PublishLogArgs {
    /// Path to the TOML configuration.
    #[arg(long)]
    config: PathBuf,
    /// The directory whose contents will be served publicly.
    #[arg(long)]
    to: PathBuf,
    /// Seal the active segment when the next record would take it past this
    /// many bytes.
    ///
    /// The default is argued in `segments::DEFAULT_SEGMENT_BYTES` against the
    /// measured line size.  Lower it in a *rehearsal* directory to exercise the
    /// seal path; changing it never invalidates anything already sealed.
    #[arg(long, default_value_t = dclutch_relayer::segments::DEFAULT_SEGMENT_BYTES)]
    segment_bytes: u64,
}

#[derive(Args)]
struct VerifyLogArgs {
    /// A served publication-log directory.
    #[arg(long)]
    dir: PathBuf,
    /// Additionally prove every published byte is this local log's byte at the
    /// same offset.  This is the whole-history comparison; the per-cycle push
    /// makes a bounded version of it.
    #[arg(long)]
    against: Option<PathBuf>,
}

#[derive(Args)]
struct SkewArgs {
    /// First cluster's RPC endpoint.
    #[arg(long)]
    endpoint_a: String,
    /// Second cluster's RPC endpoint.
    #[arg(long)]
    endpoint_b: String,
    /// How many paired samples to take.
    #[arg(long)]
    samples: u32,
    /// Seconds between samples.
    #[arg(long, default_value_t = 5)]
    interval_seconds: u64,
    /// Where to write the JSON report and the RPC read log.
    #[arg(long)]
    out_dir: PathBuf,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match dispatch(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dclutch-relayer: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Keygen(args) => keygen(&args),
        Command::ShowConfig(args) => show_config(&args),
        Command::Run(args) => run(&args).await,
        Command::SubmitArtifacts(args) => submit_artifacts(&args).await,
        Command::CreateRecord(args) => create_record(&args).await,
        Command::PublishLog(args) => publish_log(&args),
        Command::VerifyLog(args) => verify_log(&args),
        Command::MeasureSkew(args) => skew(&args).await,
    }
}

async fn create_record(args: &CreateRecordArgs) -> Result<()> {
    let home = home();
    let config = Config::load(&args.config, home.as_deref())?;
    let worker = parse_id32("--worker", &args.worker)?;
    let report = run_create_record_keeper(CreateRecordKeeperRequest {
        config: &config,
        slot_dir: &args.slot_dir,
        worker,
        execute: args.execute,
        home: home.as_deref(),
    })
    .await?;
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| RelayerError::Serialization(format!("keeper report: {error}")))?
    );
    Ok(())
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn keygen(args: &KeygenArgs) -> Result<()> {
    let home = home();
    let public = generate_keypair_file(&args.out, home.as_deref())?;
    println!("wrote a fresh keypair to {}", args.out.display());
    println!("public key (base58): {}", base58(&public));
    println!("public key (hex):    {}", to_hex(&public));
    println!(
        "this key is a TEST key until it is pinned into a RelayerKeySetV1 record; pinning it is \
         what makes it a provider release identity"
    );
    Ok(())
}

fn show_config(args: &ConfigArgs) -> Result<()> {
    let config = Config::load(&args.config, home().as_deref())?;
    print_config(&config);
    Ok(())
}

fn print_config(config: &Config) {
    println!("config:                {}", config.source_path.display());
    println!("output dir:            {}", config.output_dir.display());
    println!("poll interval:         {}s", config.poll_interval.as_secs());
    println!("body page bytes:       {}", config.body_page_bytes);
    // HOSTS, NOT URLS, and the label was already telling the truth this line
    // was not: a provider URL carries its API key in the query string, so the
    // old `config.primary_endpoint()` printed a credential to the terminal and
    // into whatever file an operator redirected it to. Found by reading a
    // deployment's own `show-config` output back off the box.
    println!(
        "primary endpoint host: {}",
        endpoint_host_for_display(config.primary_endpoint())
    );
    println!(
        "cross-check endpoints: {}",
        config.cross_check_endpoints().len()
    );
    println!(
        "expected genesis hash: {}",
        base58(&config.expected_genesis_hash)
    );
    println!(
        "attestation keypair:   {}",
        config.attestation_keypair_path.display()
    );
    match &config.fee_payer_keypair_path {
        Some(path) => println!("fee-payer keypair:     {}", path.display()),
        None => println!("fee-payer keypair:     (none; dry-run only)"),
    }
    match &config.submit {
        Some(submit) => println!(
            "submit host:           {} (allow_public_submission = {}, genesis {}, mode = {})",
            endpoint_host_for_display(&submit.endpoint),
            submit.allow_public_submission,
            base58(&submit.expected_genesis_hash),
            if submit
                .launch_capability
                .as_ref()
                .is_some_and(|capability| capability.submission_enabled)
            {
                "send-capable after live checks"
            } else {
                "armed-no-send"
            }
        ),
        None => println!("submit:                (not configured)"),
    }
    for set in &config.account_sets {
        println!();
        println!("account set {:?}", set.name);
        println!("  relay_family_id    {}", to_hex(&set.relay_family_id));
        println!("  decoding_rules_id  {}", to_hex(&set.decoding_rules_id));
        println!(
            "  account_set_id     {}   <- derived; pin this at founding",
            to_hex(&set.account_set_id)
        );
        println!("  account_set_id b58 {}", base58(&set.account_set_id));
        for (index, position) in set.positions.iter().enumerate() {
            println!(
                "  [{index}] {} owner {} inline_len {} admitted {:?}",
                base58(&position.key),
                base58(&position.expected_owner),
                position.inline_len,
                position.admitted_data_lens
            );
        }
    }
}

async fn run(args: &RunArgs) -> Result<()> {
    if !args.dry_run && !args.submit {
        return Err(RelayerError::config(
            "pass --dry-run or --submit; there is no default mode, because the difference between \
             them is whether this process touches a cluster",
        ));
    }
    let config = Config::load(&args.config, home().as_deref())?;
    print_config(&config);
    std::fs::create_dir_all(&config.output_dir)
        .map_err(|source| RelayerError::io(&config.output_dir, source))?;

    let read_log = RpcReadLog::open(&config.output_dir)?;
    let publication = PublicationLog::open(&config.output_dir)?;
    let artifacts = ArtifactWriter::new(&config.output_dir);

    let mut endpoints = Vec::with_capacity(config.rpc_endpoints.len());
    for url in &config.rpc_endpoints {
        endpoints.push(
            RpcClient::new(url, config.request_timeout, None)?.with_read_log(read_log.clone()),
        );
    }
    let (primary, cross_check) = endpoints
        .split_first()
        .ok_or_else(|| RelayerError::config("no rpc endpoints"))?;

    // The submission gate runs before the first network call.  A refused
    // endpoint must be refused on its own terms, not incidentally because some
    // earlier read happened to fail first.
    let submitter = if args.submit {
        Some(prepare_submission(&config).await?)
    } else {
        println!("dry run: nothing will be submitted");
        None
    };

    // The observed cluster is checked once, before anything is signed. Nothing
    // else distinguishes a mainnet account from a byte-identical twin on
    // another cluster, so a mismatch is fatal rather than degrading.
    primary
        .require_expected_genesis(config.expected_genesis_hash)
        .await?;
    for endpoint in cross_check {
        endpoint
            .require_expected_genesis(config.expected_genesis_hash)
            .await?;
    }
    println!(
        "verified genesis hash {} on {} endpoint(s)",
        base58(&config.expected_genesis_hash),
        endpoints.len()
    );

    let signer = AttestationSigner::load(&config.attestation_keypair_path, home().as_deref())?;
    println!("attestation signer: {}", signer.public_key_base58());

    let rehearsal_observed_genesis = config
        .rehearsal_attested_cluster_id
        .map(|_| config.expected_genesis_hash);
    if let Some(attested) = config.rehearsal_attested_cluster_id {
        eprintln!(
            "REHEARSAL TWIN: reading loopback cluster {} while attesting AS IF it were {}. Every \
             artifact and publication line is labelled; nothing produced here is an observation \
             of the claimed cluster.",
            base58(&config.expected_genesis_hash),
            base58(&attested)
        );
    }
    let mut watchers: Vec<SetWatcher> = config
        .account_sets
        .iter()
        .cloned()
        .map(|set| {
            SetWatcher::new(
                set,
                config.attested_cluster_id(),
                rehearsal_observed_genesis,
                config.body_page_bytes,
            )
        })
        .collect();

    // §4.11's refusal list includes a `deployment_slot` change, and that
    // refusal is only worth anything if it outlives the process that made it.
    // A restarted daemon with an empty map reads the upgraded slot as the first
    // one it has ever seen and attests a program it had already refused, so
    // every watcher is seeded from the newest artifact it published for its
    // set. A refused cycle writes no artifact, so that artifact carries the
    // last slot this daemon ACCEPTED, which is what the refusal compares
    // against.
    for watcher in &mut watchers {
        let set_name = watcher.config().name.clone();
        match artifacts.last_deployment_slots(&set_name)? {
            Some(seeded) => {
                for (key, slot) in &seeded.slots {
                    watcher.seed_deployment_slot(*key, *slot);
                }
                println!(
                    "set {set_name:?}: carried {} deployment_slot(s) forward from the artifact at \
                     slot {}; an upgrade refused before this restart is still refused",
                    seeded.slots.len(),
                    seeded.observed_slot
                );
            }
            None => {
                // Named limit, said out loud rather than left to be inferred
                // from silence: with no published artifact there is nothing to
                // remember, so this set's first observation defines its
                // baseline and a refusal cannot outlive this process.
                println!(
                    "set {set_name:?}: no published artifact to carry a deployment_slot forward \
                     from; this set's first observation sets its baseline, and a redeploy \
                     refused after it survives only as long as this process"
                );
            }
        }
    }

    let mut cycle_index = 0u32;
    loop {
        if args.cycles != 0 && cycle_index >= args.cycles {
            break;
        }
        if cycle_index > 0 {
            tokio::time::sleep(config.poll_interval).await;
        }
        cycle_index = cycle_index.saturating_add(1);

        let mut live = 0usize;
        for watcher in &mut watchers {
            if let Some(reason) = watcher.stopped_reason() {
                eprintln!(
                    "set {:?} is STOPPED and will not be attested again: {reason}",
                    watcher.config().name
                );
                continue;
            }
            live = live.saturating_add(1);
            match watcher.observe(primary, cross_check, &signer).await {
                Ok(cycle) => {
                    publish(&publication, &cycle)?;
                    let dir = artifacts.write_cycle(&cycle)?;
                    println!(
                        "set {:?} slot {} set_digest {} -> {}",
                        cycle.set_name,
                        cycle.observed_slot,
                        to_hex(&cycle.set_digest),
                        dir.display()
                    );
                    if let Some(submitter) = &submitter {
                        submitter.submit_cycle(&cycle).await?;
                    }
                }
                Err(error) => {
                    // A refusal has already stopped the set inside the watcher;
                    // a transport failure has not, and is retried next cycle.
                    eprintln!("set {:?}: {error}", watcher.config().name);
                }
            }
        }
        if live == 0 {
            eprintln!("every watched set has stopped; exiting");
            return Err(RelayerError::ObservationRefused {
                set: "*".to_owned(),
                reason: "every watched set stopped attesting".to_owned(),
            });
        }
    }
    println!("publication log: {}", publication.path().display());
    println!("rpc read log:    {}", read_log.path().display());
    Ok(())
}

fn publish(log: &PublicationLog, cycle: &ObservationCycle) -> Result<()> {
    for position in &cycle.positions {
        log.record(
            MessageKind::Attestation,
            &cycle.set_name,
            &cycle.account_set_id,
            cycle.observed_slot,
            Some(position.set_index),
            &position.message_bytes,
            &cycle.signer,
            &position.signature,
            cycle.rehearsal_observed_genesis.as_ref(),
        )?;
    }
    log.record(
        MessageKind::Seal,
        &cycle.set_name,
        &cycle.account_set_id,
        cycle.observed_slot,
        None,
        &cycle.seal_bytes,
        &cycle.signer,
        &cycle.seal_signature,
        cycle.rehearsal_observed_genesis.as_ref(),
    )
}

struct Submitter {
    rpc: RpcClient,
    fee_payer: AttestationSigner,
    relay_program_id: [u8; ID_BYTES],
    market: [u8; ID_BYTES],
    generation: u64,
    relayer_key_set: [u8; ID_BYTES],
    relayer_key_set_staging_vacancy: [u8; ID_BYTES],
    compute_budget: ComputeBudget,
    lookup_tables: Vec<AddressLookupTableAccount>,
    output_dir: PathBuf,
    submit_cluster_id: [u8; ID_BYTES],
    observed_cluster_id: [u8; ID_BYTES],
    source_material_id: [u8; ID_BYTES],
    provider_release_id: [u8; ID_BYTES],
    relayer_key_set_id: [u8; ID_BYTES],
}

async fn prepare_submission(config: &Config) -> Result<Submitter> {
    let submit = config.submit.as_ref().ok_or_else(|| {
        RelayerError::MissingCapability(
            "--submit needs a [submit] table in the config file".to_owned(),
        )
    })?;
    require_submission_admitted(submit)?;

    let capability = submit.launch_capability.as_ref().ok_or_else(|| {
        RelayerError::MissingCapability(
            "submission is armed-no-send: [submit.launch_capability] is absent; add it only after \
             the accepted public caller and live Market exist"
                .to_owned(),
        )
    })?;
    if !capability.submission_enabled {
        return Err(RelayerError::MissingCapability(
            "submission is armed-no-send: submit.launch_capability.submission_enabled is false"
                .to_owned(),
        ));
    }
    let receipt = std::fs::read(&capability.accepted_caller_receipt_path)
        .map_err(|source| RelayerError::io(&capability.accepted_caller_receipt_path, source))?;
    let receipt_digest = dclutch_relayer::derive::sha256(&receipt);
    if receipt_digest != capability.accepted_caller_receipt_sha256 {
        return Err(RelayerError::MissingCapability(format!(
            "accepted caller receipt {} has SHA-256 {}, not the capability-pinned {}; submission \
             remains armed-no-send",
            capability.accepted_caller_receipt_path.display(),
            to_hex(&receipt_digest),
            to_hex(&capability.accepted_caller_receipt_sha256)
        )));
    }

    let fee_payer_path = config.fee_payer_keypair_path.as_ref().ok_or_else(|| {
        RelayerError::MissingCapability(
            "--submit needs keys.fee_payer_keypair_path; the fee payer is a distinct key from the \
             attestation key"
                .to_owned(),
        )
    })?;
    let fee_payer = AttestationSigner::load(fee_payer_path, home().as_deref())?;
    println!("fee payer: {}", fee_payer.public_key_base58());
    eprintln!(
        "NOTE: this daemon builds only the append and seal routes. The observation record must \
         already exist for the (market, generation, account_set_id, observed_slot) being \
         submitted; record creation and retirement are not constructed here."
    );

    let lookup_tables = submit
        .address_lookup_table
        .as_ref()
        .map(|table| AddressLookupTableAccount {
            key: solana_address::Address::from(table.key),
            addresses: table
                .addresses
                .iter()
                .copied()
                .map(solana_address::Address::from)
                .collect(),
        })
        .into_iter()
        .collect();

    let rpc = RpcClient::new(&submit.endpoint, config.request_timeout, None)?
        .logging_to(&config.output_dir)?;

    // THE SUBMIT CLUSTER IS NAMED AS A VALUE, AND CHECKED, exactly as the
    // observed cluster is at the top of `run`.  A URL is a routing detail an
    // operator can mistype into pointing anywhere; a genesis hash is the
    // cluster's identity, and checking it makes the daemon UNABLE to sign a
    // transaction toward a cluster the config did not name.  This runs before
    // the fee payer is even reported and long before a transaction is built.
    rpc.require_expected_genesis(submit.expected_genesis_hash)
        .await?;
    println!(
        "verified submit-cluster genesis hash {}",
        base58(&submit.expected_genesis_hash)
    );
    require_live_launch_contract(&rpc, submit, capability).await?;
    if let Some(table) = submit.address_lookup_table.as_ref() {
        require_live_lookup_table_contract(&rpc, table).await?;
    }

    Ok(Submitter {
        rpc,
        fee_payer,
        relay_program_id: submit.relay_program_id,
        market: submit.market,
        generation: submit.generation,
        relayer_key_set: submit.relayer_key_set,
        relayer_key_set_staging_vacancy: submit.relayer_key_set_staging_vacancy,
        compute_budget: ComputeBudget {
            unit_limit: submit.compute_unit_limit,
            unit_price_micro_lamports: submit.compute_unit_price_micro_lamports,
        },
        lookup_tables,
        output_dir: config.output_dir.clone(),
        submit_cluster_id: submit.expected_genesis_hash,
        observed_cluster_id: config.attested_cluster_id(),
        source_material_id: capability.source_material_id,
        provider_release_id: capability.provider_release_id,
        relayer_key_set_id: capability.relayer_key_set_id,
    })
}

async fn require_live_launch_contract(
    rpc: &RpcClient,
    submit: &dclutch_relayer::config::SubmitConfig,
    capability: &dclutch_relayer::config::LaunchCapabilityConfig,
) -> Result<()> {
    let read = rpc
        .get_multiple_accounts(
            &[
                submit.relay_program_id,
                capability.relay_program_data,
                submit.market,
            ],
            45,
            None,
        )
        .await?;
    let program = read
        .accounts
        .first()
        .and_then(Option::as_ref)
        .ok_or_else(|| RelayerError::MissingCapability("relay program is absent".to_owned()))?;
    let programdata = read
        .accounts
        .get(1)
        .and_then(Option::as_ref)
        .ok_or_else(|| RelayerError::MissingCapability("relay ProgramData is absent".to_owned()))?;
    let market = read
        .accounts
        .get(2)
        .and_then(Option::as_ref)
        .ok_or_else(|| RelayerError::MissingCapability("live Market is absent".to_owned()))?;
    require_live_launch_accounts(
        LaunchExpectation {
            relay_program_id: submit.relay_program_id,
            relay_program_data: capability.relay_program_data,
            relay_program_deployment_slot: capability.relay_program_deployment_slot,
            market: submit.market,
            market_owner: capability.market_owner,
        },
        program,
        programdata,
        market,
    )?;
    println!(
        "launch capability: accepted caller receipt {}, relay slot {}, live Market finalized at \
         slot {}",
        to_hex(&capability.accepted_caller_receipt_sha256),
        capability.relay_program_deployment_slot,
        read.slot
    );
    Ok(())
}

/// Read the configured lookup table live and refuse a stale or permuted one.
///
/// Until this check the `[submit.address_lookup_table]` list was trusted
/// verbatim.  A v0 message compiles table *indexes*, so a configured order
/// that differs from the stored order delivers a permuted account frame the
/// program refuses while the table looks healthy from every other angle.  The
/// slice width is the exact expected table width; the full-width pin inside
/// the gate makes a longer table refuse rather than truncate into a match.
async fn require_live_lookup_table_contract(
    rpc: &RpcClient,
    table: &dclutch_relayer::config::AddressLookupTableConfig,
) -> Result<()> {
    let expected_len = dclutch_relayer::chain::LOOKUP_TABLE_META_BYTES
        + dclutch_relayer::id32::ID_BYTES * table.addresses.len();
    let slice_len = u16::try_from(expected_len).map_err(|_| {
        RelayerError::MissingCapability(format!(
            "configured lookup table {} would be {} bytes; no loadable table is that wide",
            base58(&table.key),
            expected_len
        ))
    })?;
    let read = rpc
        .get_multiple_accounts(&[table.key], slice_len, None)
        .await?;
    let live = read
        .accounts
        .first()
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            RelayerError::MissingCapability(format!(
                "configured lookup table {} does not exist on the submit cluster",
                base58(&table.key)
            ))
        })?;
    require_live_lookup_table(table, live, read.slot)?;
    println!(
        "lookup table {}: live, activated, {} addresses in the configured order",
        base58(&table.key),
        table.addresses.len()
    );
    Ok(())
}

/// One complete signed observation set, however it reached this process —
/// straight from an in-process observation cycle, or read back from a dry-run
/// artifact directory whose signatures were re-verified.
struct SignedObservationSet {
    account_set_id: [u8; ID_BYTES],
    observed_slot: u64,
    signer: [u8; ID_BYTES],
    /// `(set_index, exact message bytes, signature)`, in set order.
    attestations: Vec<(u16, Vec<u8>, [u8; 64])>,
    seal_bytes: [u8; dclutch_relay_contract::RELAYED_SEAL_BYTES],
    seal_signature: [u8; 64],
}

impl SignedObservationSet {
    fn from_cycle(cycle: &ObservationCycle) -> Self {
        Self {
            account_set_id: cycle.account_set_id,
            observed_slot: cycle.observed_slot,
            signer: cycle.signer,
            attestations: cycle
                .positions
                .iter()
                .map(|position| {
                    (
                        position.set_index,
                        position.message_bytes.clone(),
                        position.signature,
                    )
                })
                .collect(),
            seal_bytes: cycle.seal_bytes,
            seal_signature: cycle.seal_signature,
        }
    }

    fn expectation(&self, submitter: &Submitter) -> Result<DeliveryExpectation> {
        let expected_count = u16::try_from(self.attestations.len())
            .map_err(|_| RelayerError::config("attestation count does not fit in u16"))?;
        let mut bodies = Vec::with_capacity(self.attestations.len());
        for (expected_index, (set_index, message_bytes, _)) in self.attestations.iter().enumerate()
        {
            let expected_index = u16::try_from(expected_index)
                .map_err(|_| RelayerError::config("attestation index does not fit in u16"))?;
            if *set_index != expected_index {
                return Err(RelayerError::config(format!(
                    "attestations are reordered or discontinuous: expected position \
                     {expected_index}, found {set_index}"
                )));
            }
            let message = dclutch_relay_contract::wire::AttestationMessageV1::decode(message_bytes)
                .map_err(|error| RelayerError::wire("delivery attestation", error))?;
            if message.observed_cluster_id() != submitter.observed_cluster_id
                || message.account_set_id() != self.account_set_id
                || message.observed_slot() != self.observed_slot
                || message.set_index() != expected_index
                || message.set_count() != expected_count
            {
                return Err(RelayerError::config(format!(
                    "attestation {expected_index} does not carry the delivery's exact \
                     cluster/set/slot/order/count binding"
                )));
            }
            let body = message.body();
            let mut encoded = vec![0u8; body.encoded_len()];
            body.encode_into(&mut encoded)
                .map_err(|error| RelayerError::wire("delivery body", error))?;
            bodies.push(encoded);
        }
        let seal = dclutch_relay_contract::wire::ObservationSetSealV1::decode(&self.seal_bytes)
            .map_err(|error| RelayerError::wire("delivery seal", error))?;
        if seal.observed_cluster_id() != submitter.observed_cluster_id
            || seal.account_set_id() != self.account_set_id
            || seal.observed_slot() != self.observed_slot
            || seal.set_count() != expected_count
        {
            return Err(RelayerError::config(
                "seal does not carry the delivery's exact cluster/set/slot/count binding",
            ));
        }
        Ok(DeliveryExpectation {
            submit_cluster_id: submitter.submit_cluster_id,
            relay_program_id: submitter.relay_program_id,
            market: submitter.market,
            generation: submitter.generation,
            source_material_id: submitter.source_material_id,
            account_set_id: self.account_set_id,
            provider_release_id: submitter.provider_release_id,
            // The record carries the key set's IDENTITY; `relayer_key_set` is
            // the record account's ADDRESS the frame passes, and the address is
            // derived FROM the identity, so the two never compare equal.
            relayer_key_set_id: submitter.relayer_key_set_id,
            observed_cluster_id: submitter.observed_cluster_id,
            observed_slot: self.observed_slot,
            bodies,
            set_digest: seal.set_digest(),
        })
    }
}

impl Submitter {
    async fn submit_cycle(&self, cycle: &ObservationCycle) -> Result<()> {
        self.submit_signed_set(&SignedObservationSet::from_cycle(cycle))
            .await
    }

    async fn submit_signed_set(&self, set: &SignedObservationSet) -> Result<()> {
        let (record, _bump) = derive_record_address(
            self.relay_program_id,
            self.market,
            self.generation,
            set.account_set_id,
            set.observed_slot,
        );
        let addresses = RelayFrameAddresses {
            worker: self.fee_payer.public_key(),
            market: self.market,
            record,
            relayer_key_set: self.relayer_key_set,
            relayer_key_set_staging_vacancy: self.relayer_key_set_staging_vacancy,
        };
        let expectation = set.expectation(self)?;
        let mut journal = DeliveryJournal::open(&self.output_dir, record)?;
        journal.record(
            "delivery-open",
            serde_json::json!({
                "record": base58(&record),
                "account_set_id": to_hex(&set.account_set_id),
                "observed_slot": set.observed_slot,
                "body_count": set.attestations.len(),
            }),
        )?;

        loop {
            let action = self.reconcile(record, &expectation).await?;
            journal.record(
                "finalized-reconcile",
                serde_json::json!({ "action": format!("{action:?}") }),
            )?;
            match action {
                DeliveryAction::AwaitRecord => {
                    return Err(RelayerError::MissingCapability(format!(
                        "observation record {} does not exist at finalized commitment; create it \
                         for this exact set and slot, then rerun the same artifact",
                        base58(&record)
                    )));
                }
                DeliveryAction::Complete => {
                    journal.record(
                        "delivery-complete",
                        serde_json::json!({ "record": base58(&record) }),
                    )?;
                    return Ok(());
                }
                DeliveryAction::Append(index) => {
                    let (set_index, message, signature) = set
                        .attestations
                        .get(usize::from(index))
                        .ok_or_else(|| RelayerError::config("next append is outside artifact"))?;
                    if *set_index != index {
                        return Err(RelayerError::config("next append index is reordered"));
                    }
                    let plan = append_observation_instruction(
                        self.relay_program_id,
                        &addresses,
                        self.generation,
                        set.observed_slot,
                        message,
                    )?;
                    self.send_until_advanced(
                        record,
                        &expectation,
                        action,
                        &format!("append position {index}"),
                        &plan,
                        &set.signer,
                        signature,
                        &mut journal,
                    )
                    .await?;
                }
                DeliveryAction::Seal => {
                    let plan = seal_record_instruction(
                        self.relay_program_id,
                        &addresses,
                        self.generation,
                        set.observed_slot,
                        &set.seal_bytes,
                    )?;
                    self.send_until_advanced(
                        record,
                        &expectation,
                        action,
                        "seal",
                        &plan,
                        &set.signer,
                        &set.seal_signature,
                        &mut journal,
                    )
                    .await?;
                }
            }
        }
    }

    async fn reconcile(
        &self,
        record: [u8; ID_BYTES],
        expectation: &DeliveryExpectation,
    ) -> Result<DeliveryAction> {
        let read = self
            .rpc
            .get_multiple_accounts(&[record], u16::MAX, None)
            .await?;
        let account = read.accounts.first().and_then(Option::as_ref);
        if account.is_some_and(|account| account.executable) {
            return Err(RelayerError::config(
                "record acknowledgement account is unexpectedly executable",
            ));
        }
        reconcile_finalized_record(
            expectation,
            account.map(|account| account.owner),
            account.map(|account| account.data.as_slice()),
        )
    }

    #[allow(clippy::too_many_arguments)]
    async fn send_until_advanced(
        &self,
        record: [u8; ID_BYTES],
        expectation: &DeliveryExpectation,
        before: DeliveryAction,
        context: &str,
        plan: &RelayInstructionPlan,
        attestation_signer: &[u8; ID_BYTES],
        attestation_signature: &[u8; 64],
        journal: &mut DeliveryJournal,
    ) -> Result<()> {
        for attempt in 1..=MAX_DELIVERY_SEND_ATTEMPTS {
            journal.record(
                "send-attempt",
                serde_json::json!({ "context": context, "attempt": attempt }),
            )?;
            let send = self
                .send(context, plan, attestation_signer, attestation_signature)
                .await;
            match &send {
                Ok(signature) => {
                    println!("  {context} submitted as {signature}");
                    journal.record(
                        "send-response",
                        serde_json::json!({
                            "context": context,
                            "attempt": attempt,
                            "signature": signature,
                        }),
                    )?;
                }
                Err(error) => {
                    eprintln!(
                        "  {context} attempt {attempt} returned {error}; checking finalized state \
                         before any retry"
                    );
                    journal.record(
                        "send-response-lost-or-refused",
                        serde_json::json!({
                            "context": context,
                            "attempt": attempt,
                            "diagnostic": error.to_string(),
                        }),
                    )?;
                }
            }
            for _ in 0..ACK_POLLS_PER_SEND_ATTEMPT {
                match self.reconcile(record, expectation).await {
                    Ok(after) if after != before && after != DeliveryAction::AwaitRecord => {
                        journal.record(
                            "finalized-ack",
                            serde_json::json!({
                                "context": context,
                                "attempt": attempt,
                                "next_action": format!("{after:?}"),
                            }),
                        )?;
                        return Ok(());
                    }
                    Ok(_) => tokio::time::sleep(Duration::from_secs(1)).await,
                    Err(error) => {
                        journal.record(
                            "ack-refused",
                            serde_json::json!({
                                "context": context,
                                "attempt": attempt,
                                "diagnostic": error.to_string(),
                            }),
                        )?;
                        return Err(error);
                    }
                }
            }
            // A new iteration obtains a new blockhash and fee-payer signature,
            // but `plan` still contains the identical signed observation.
        }
        Err(RelayerError::config(format!(
            "{context} did not obtain an exact finalized record acknowledgement after \
             {MAX_DELIVERY_SEND_ATTEMPTS} bounded attempts"
        )))
    }

    async fn send(
        &self,
        context: &str,
        plan: &RelayInstructionPlan,
        attestation_signer: &[u8; ID_BYTES],
        attestation_signature: &[u8; 64],
    ) -> Result<String> {
        let (blockhash_text, _) = self.rpc.get_latest_blockhash().await?;
        let blockhash = dclutch_relayer::id32::parse_id32("blockhash", &blockhash_text)?;
        // On expiry the caller re-signs this same built message with a fresh
        // blockhash; it never re-observes. The attestation is bound to a slot,
        // and re-observing would silently change the fact being attested.
        let built = build_relay_transaction_plan(
            self.fee_payer.public_key(),
            plan,
            attestation_signer,
            attestation_signature,
            self.compute_budget,
            &self.lookup_tables,
            blockhash,
        )?;
        let bytes = message_bytes(&built.message);
        let fee_payer_signature = self.fee_payer.sign(&bytes);
        let transaction = sign_transaction(built.message, fee_payer_signature);
        let wire = serialize_transaction(&transaction)?;
        // Refuse an unsendable wire BEFORE it reaches an RPC node: the two
        // known oversized frames in this family (the full-body VirtualPool
        // append and the consumption) must ride the Market ALT, and this is
        // where a missing table is reported as itself rather than as an
        // opaque RPC refusal.
        let routed = self
            .lookup_tables
            .iter()
            .map(|table| table.addresses.len())
            .sum();
        require_packet_fit(context, &wire, routed)?;
        self.rpc.send_transaction(&base64_encode(&wire)).await
    }
}

/// Submit the append and seal routes from one dry-run artifact directory.
async fn submit_artifacts(args: &SubmitArtifactsArgs) -> Result<()> {
    let config = Config::load(&args.config, home().as_deref())?;
    let submitter = prepare_submission(&config).await?;

    let manifest_path = args.slot_dir.join("manifest.json");
    let manifest_text = std::fs::read_to_string(&manifest_path)
        .map_err(|source| RelayerError::io(&manifest_path, source))?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest_text)
        .map_err(|source| RelayerError::Serialization(source.to_string()))?;
    if manifest.get("artifact_schema").and_then(|v| v.as_str())
        != Some("dclutch.relayer.dry-run.v1")
    {
        return Err(RelayerError::config(format!(
            "{} does not carry artifact_schema dclutch.relayer.dry-run.v1",
            manifest_path.display()
        )));
    }
    let set_name = manifest
        .get("set_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RelayerError::config("manifest has no set_name"))?;
    let account_set_id = dclutch_relayer::id32::parse_id32(
        "manifest.account_set_id_hex",
        manifest
            .get("account_set_id_hex")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    )?;
    let observed_slot = manifest
        .get("observed_slot")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| RelayerError::config("manifest has no observed_slot"))?;
    let signer = dclutch_relayer::id32::parse_id32(
        "manifest.attestation_signer_pubkey_hex",
        manifest
            .get("attestation_signer_pubkey_hex")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
    )?;
    if manifest.get("rehearsal_twin").is_some_and(|v| !v.is_null()) {
        eprintln!(
            "REHEARSAL ARTIFACTS: these attestations claim a cluster a loopback twin stood in \
             for. The submission gate already refuses public endpoints for them."
        );
    }

    // The artifact's set must be one this config still derives identically:
    // the pinned ordered positions are the authority for what may be attested,
    // and a drifted config must refuse rather than submit under an old pin.
    let configured = config
        .account_sets
        .iter()
        .find(|set| set.name == set_name)
        .ok_or_else(|| {
            RelayerError::config(format!(
                "this config watches no account set named {set_name:?}"
            ))
        })?;
    if configured.account_set_id != account_set_id {
        return Err(RelayerError::config(format!(
            "the artifact's account_set_id {} does not match the one this config derives for set \
             {:?}; the pinned positions have drifted since this observation was taken",
            dclutch_relayer::id32::to_hex(&account_set_id),
            set_name
        )));
    }

    let positions = manifest
        .get("positions")
        .and_then(|v| v.as_array())
        .ok_or_else(|| RelayerError::config("manifest has no positions"))?;
    let mut attestations = Vec::with_capacity(positions.len());
    for position in positions {
        let set_index = u16::try_from(
            position
                .get("set_index")
                .and_then(serde_json::Value::as_u64)
                .ok_or_else(|| RelayerError::config("position has no set_index"))?,
        )
        .map_err(|_| RelayerError::config("set_index does not fit in a u16"))?;
        let message_file = position
            .get("message_file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RelayerError::config("position has no message_file"))?;
        let message_path = args.slot_dir.join(message_file);
        let message_bytes = std::fs::read(&message_path)
            .map_err(|source| RelayerError::io(&message_path, source))?;
        let signature_file = position
            .get("signature_file")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RelayerError::config("position has no signature_file"))?;
        let signature_path = args.slot_dir.join(signature_file);
        let signature_bytes = std::fs::read(&signature_path)
            .map_err(|source| RelayerError::io(&signature_path, source))?;
        let signature: [u8; 64] = signature_bytes
            .as_slice()
            .try_into()
            .map_err(|_| RelayerError::config("a signature file is not exactly 64 bytes"))?;

        // Everything below is re-derived from the exact bytes on disk, not
        // trusted from the manifest: the signature must verify against the
        // named signer, and the message must decode as an attestation of this
        // exact set at this exact slot at this exact position.
        if !dclutch_relayer::keys::verify_detached(&signer, &message_bytes, &signature) {
            return Err(RelayerError::config(format!(
                "attestation {set_index}: the recorded signature does not verify against the \
                 recorded message and signer; refusing to submit bytes that would be refused \
                 on chain"
            )));
        }
        let decoded = dclutch_relay_contract::wire::AttestationMessageV1::decode(&message_bytes)
            .map_err(|error| RelayerError::wire("recorded attestation", error))?;
        if decoded.account_set_id() != account_set_id
            || decoded.observed_slot() != observed_slot
            || decoded.set_index() != set_index
        {
            return Err(RelayerError::config(format!(
                "attestation {set_index}: the recorded message does not attest this artifact's \
                 own set and slot"
            )));
        }
        attestations.push((set_index, message_bytes, signature));
    }
    attestations.sort_by_key(|(set_index, _, _)| *set_index);

    let seal_path = args.slot_dir.join("seal.bin");
    let seal_bytes_vec =
        std::fs::read(&seal_path).map_err(|source| RelayerError::io(&seal_path, source))?;
    let seal_bytes: [u8; dclutch_relay_contract::RELAYED_SEAL_BYTES] = seal_bytes_vec
        .as_slice()
        .try_into()
        .map_err(|_| RelayerError::config("seal.bin is not exactly the seal width"))?;
    let seal_sig_path = args.slot_dir.join("seal.sig");
    let seal_sig_vec =
        std::fs::read(&seal_sig_path).map_err(|source| RelayerError::io(&seal_sig_path, source))?;
    let seal_signature: [u8; 64] = seal_sig_vec
        .as_slice()
        .try_into()
        .map_err(|_| RelayerError::config("seal.sig is not exactly 64 bytes"))?;
    if !dclutch_relayer::keys::verify_detached(&signer, &seal_bytes, &seal_signature) {
        return Err(RelayerError::config(
            "the recorded seal signature does not verify against the recorded seal and signer",
        ));
    }
    let decoded_seal = dclutch_relay_contract::wire::ObservationSetSealV1::decode(&seal_bytes)
        .map_err(|error| RelayerError::wire("recorded seal", error))?;
    if decoded_seal.account_set_id() != account_set_id
        || decoded_seal.observed_slot() != observed_slot
    {
        return Err(RelayerError::config(
            "the recorded seal does not seal this artifact's own set and slot",
        ));
    }

    println!(
        "submitting recorded observation of set {:?} at slot {observed_slot}: {} attestation(s) \
         and one seal",
        set_name,
        attestations.len()
    );
    submitter
        .submit_signed_set(&SignedObservationSet {
            account_set_id,
            observed_slot,
            signer,
            attestations,
            seal_bytes,
            seal_signature,
        })
        .await
}

/// Push the publication log to a local public-serve directory.
///
/// The push is incremental: only the local log's unpublished tail is read, and
/// only the active segment is written.  A flat push read and rewrote the whole
/// history every cycle, which is fine at eight lines and is not fine inside a
/// unit with `MemoryMax=256M` once the log is measured in hundreds of megabytes.
fn publish_log(args: &PublishLogArgs) -> Result<()> {
    let config = Config::load(&args.config, home().as_deref())?;
    let source = config.output_dir.join(segments::LOCAL_LOG_FILE);

    let mut published = segments::PublishedLog::open(&args.to, args.segment_bytes)?;
    let outcome = published.publish(&source, args.segment_bytes)?;

    for sealed in &outcome.sealed_this_run {
        println!(
            "sealed {} — immutable from here; the next segment opens with its digest",
            segments::segment_file_name(*sealed)
        );
    }
    if outcome.retired_flat_log {
        println!(
            "{} is complete: it holds exactly the bytes of {} plus one continuation record naming \
             where the log goes next. Every byte ever served at that path is still at the offset \
             it was served at",
            segments::LEGACY_FLAT_LOG_FILE,
            segments::segment_file_name(1)
        );
    }
    println!(
        "published {} new record(s); segment {} now {} bytes; {} record(s) and {} record bytes in \
         all; chain head {}",
        outcome.records_appended,
        outcome.current_segment,
        outcome.current_bytes,
        outcome.total_records,
        outcome.total_record_bytes,
        outcome.chain_head_sha256_hex
    );
    if outcome.deferred_bytes > 0 {
        println!(
            "{} local byte(s) remain unpublished this cycle (the per-run cap keeps a catch-up \
             bounded); the next cycle continues",
            outcome.deferred_bytes
        );
    }
    println!(
        "serve {} statically (any static host works). A reader fetches {} for liveness, {} for \
         the segment index, and checks each record's signature against the pinned relayer key set \
         and each attested account against the observed cluster; {} in that directory says how",
        args.to.display(),
        segments::LATEST_FILE,
        segments::INDEX_FILE,
        segments::README_FILE
    );
    Ok(())
}

/// Verify a served publication-log directory, offline.
fn verify_log(args: &VerifyLogArgs) -> Result<()> {
    let report = segments::verify_directory(&args.dir, args.against.as_deref())?;
    for check in &report.checks {
        println!("ok  {check}");
    }
    println!(
        "verified {}: {} sealed segment(s), {} record(s), {} record bytes, chain head {}",
        args.dir.display(),
        report.sealed_segments,
        report.records,
        report.record_bytes,
        report.chain_head_sha256_hex
    );
    Ok(())
}

async fn skew(args: &SkewArgs) -> Result<()> {
    std::fs::create_dir_all(&args.out_dir)
        .map_err(|source| RelayerError::io(&args.out_dir, source))?;
    let read_log = RpcReadLog::open(&args.out_dir)?;
    let timeout = Duration::from_secs(30);
    let a = RpcClient::new(&args.endpoint_a, timeout, None)?.with_read_log(read_log.clone());
    let b = RpcClient::new(&args.endpoint_b, timeout, None)?.with_read_log(read_log.clone());

    eprintln!(
        "measure-skew is read-only: {} paired Clock reads against {} and {}, every call logged to \
         {}",
        args.samples,
        a.host(),
        b.host(),
        read_log.path().display()
    );
    let report = measure_skew(
        &a,
        &b,
        args.samples,
        Duration::from_secs(args.interval_seconds),
    )
    .await?;
    let json = report.to_json();
    let path = args.out_dir.join("clock_skew_report.json");
    let text = serde_json::to_string_pretty(&json)
        .map_err(|source| RelayerError::Serialization(source.to_string()))?;
    std::fs::write(&path, text).map_err(|source| RelayerError::io(&path, source))?;
    println!(
        "max observed |{} - {}| = {} seconds over {} samples",
        report.a_host,
        report.b_host,
        report.max_abs_delta_seconds,
        report.samples.len()
    );
    println!("report: {}", path.display());
    println!(
        "label this measured-profile and state the sampling window; max_cluster_skew_seconds \
         stays provisional until a release names both"
    );
    Ok(())
}
