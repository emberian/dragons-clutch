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
use dclutch_relayer::config::Config;
use dclutch_relayer::error::{RelayerError, Result};
use dclutch_relayer::id32::{ID_BYTES, base58, to_hex};
use dclutch_relayer::keys::{AttestationSigner, generate_keypair_file};
use dclutch_relayer::observe::{ObservationCycle, SetWatcher};
use dclutch_relayer::publog::{MessageKind, PublicationLog, RpcReadLog};
use dclutch_relayer::rpc::{RpcClient, base64_encode};
use dclutch_relayer::skew::measure_skew;
use dclutch_relayer::submit::require_submission_admitted;
use dclutch_relayer::txn::{
    ComputeBudget, RelayFrameAddresses, RelayInstructionPlan, append_observation_instruction,
    build_relay_transaction_plan, derive_record_address, message_bytes, seal_record_instruction,
    serialize_transaction, sign_transaction,
};
use solana_message::AddressLookupTableAccount;

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
        Command::MeasureSkew(args) => skew(&args).await,
    }
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
    println!("primary endpoint host: {}", config.primary_endpoint());
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
            "submit:                {} (allow_public_submission = {})",
            submit.endpoint, submit.allow_public_submission
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
        Some(prepare_submission(&config)?)
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

    let mut watchers: Vec<SetWatcher> = config
        .account_sets
        .iter()
        .cloned()
        .map(|set| SetWatcher::new(set, config.expected_genesis_hash, config.body_page_bytes))
        .collect();

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
}

fn prepare_submission(config: &Config) -> Result<Submitter> {
    let submit = config.submit.as_ref().ok_or_else(|| {
        RelayerError::MissingCapability(
            "--submit needs a [submit] table in the config file".to_owned(),
        )
    })?;
    require_submission_admitted(submit)?;

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

    Ok(Submitter {
        rpc: RpcClient::new(&submit.endpoint, config.request_timeout, None)?
            .logging_to(&config.output_dir)?,
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
    })
}

impl Submitter {
    async fn submit_cycle(&self, cycle: &ObservationCycle) -> Result<()> {
        let (record, _bump) = derive_record_address(
            self.relay_program_id,
            self.market,
            self.generation,
            cycle.account_set_id,
            cycle.observed_slot,
        );
        let addresses = RelayFrameAddresses {
            worker: self.fee_payer.public_key(),
            market: self.market,
            record,
            relayer_key_set: self.relayer_key_set,
            relayer_key_set_staging_vacancy: self.relayer_key_set_staging_vacancy,
        };

        for position in &cycle.positions {
            let plan = append_observation_instruction(
                self.relay_program_id,
                &addresses,
                self.generation,
                cycle.observed_slot,
                &position.message_bytes,
            )?;
            let signature = self.send(&plan, &cycle.signer, &position.signature).await?;
            println!("  appended position {} in {signature}", position.set_index);
        }

        let plan = seal_record_instruction(
            self.relay_program_id,
            &addresses,
            self.generation,
            cycle.observed_slot,
            &cycle.seal_bytes,
        )?;
        let signature = self
            .send(&plan, &cycle.signer, &cycle.seal_signature)
            .await?;
        println!("  sealed in {signature}");
        Ok(())
    }

    async fn send(
        &self,
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
        self.rpc.send_transaction(&base64_encode(&wire)).await
    }
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
