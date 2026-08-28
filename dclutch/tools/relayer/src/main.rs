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

use dclutch_relay_contract::record::{RelayedObservationRecordViewV1, RelayedRecordPhaseV1};
use dclutch_relayer::artifacts::ArtifactWriter;
use dclutch_relayer::config::{Config, endpoint_host_for_display};
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
    build_relay_transaction_plan, derive_record_address, message_bytes, require_packet_fit,
    seal_record_instruction, serialize_transaction, sign_transaction,
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
    /// Push the publication log to a local public-serve directory.
    ///
    /// This is the file-target half of §4.11's publication requirement: it
    /// refuses to overwrite a divergent public copy (append-only or nothing),
    /// and writes a `LATEST.json` a verifier can poll.  Serving the directory
    /// is the operator's act; no external service is contacted here.
    PublishLog(PublishLogArgs),
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
struct PublishLogArgs {
    /// Path to the TOML configuration.
    #[arg(long)]
    config: PathBuf,
    /// The directory whose contents will be served publicly.
    #[arg(long)]
    to: PathBuf,
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
        Command::PublishLog(args) => publish_log(&args),
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
            "submit host:           {} (allow_public_submission = {}, genesis {})",
            endpoint_host_for_display(&submit.endpoint),
            submit.allow_public_submission,
            base58(&submit.expected_genesis_hash)
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
}

async fn prepare_submission(config: &Config) -> Result<Submitter> {
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
    })
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

        for (set_index, message_bytes, signature) in &set.attestations {
            let plan = append_observation_instruction(
                self.relay_program_id,
                &addresses,
                self.generation,
                set.observed_slot,
                message_bytes,
            )?;
            let context = format!("append position {set_index}");
            let signature = self.send(&context, &plan, &set.signer, signature).await?;
            println!("  appended position {set_index} in {signature}");
            // Appends fill strictly increasing positions and the NEXT append's
            // preflight simulates against the FINALIZED bank, so an append
            // submitted before its predecessor finalizes is refused as
            // out-of-order -- correctly, by the record's own replay rule. The
            // record's state is the precondition, so the record's state is
            // what this waits on, not a signature status.
            let expected = set_index
                .checked_add(1)
                .ok_or_else(|| RelayerError::config("append position overflowed"))?;
            self.await_filled(record, expected).await?;
        }

        let plan = seal_record_instruction(
            self.relay_program_id,
            &addresses,
            self.generation,
            set.observed_slot,
            &set.seal_bytes,
        )?;
        let signature = self
            .send("seal", &plan, &set.signer, &set.seal_signature)
            .await?;
        println!("  sealed in {signature}");
        // Exit only once the seal is FINAL: the process boundary is the
        // contract, and a caller that reads the record after this command
        // returns success must find it Sealed rather than race the bank.
        self.await_sealed(record).await?;
        Ok(())
    }

    /// Poll the record at finalized commitment until `expected` positions are
    /// filled.
    async fn await_filled(&self, record: [u8; ID_BYTES], expected: u16) -> Result<()> {
        self.await_record(record, &format!("filled_count {expected}"), |view| {
            view.filled_count() == Ok(expected)
        })
        .await
    }

    /// Poll the record at finalized commitment until it is Sealed.
    async fn await_sealed(&self, record: [u8; ID_BYTES]) -> Result<()> {
        self.await_record(record, "phase Sealed", |view| {
            view.phase() == Ok(RelayedRecordPhaseV1::Sealed)
        })
        .await
    }

    async fn await_record(
        &self,
        record: [u8; ID_BYTES],
        condition: &str,
        reached: impl Fn(RelayedObservationRecordViewV1<'_>) -> bool,
    ) -> Result<()> {
        // 300 seconds at one read per second, matching the campaign's own
        // finalization patience: a co-tenant laptop can stall finalization
        // past a minute while the validator is healthy.
        for _ in 0..300 {
            let read = self
                .rpc
                .get_multiple_accounts(&[record], u16::MAX, None)
                .await?;
            if let Some(Some(account)) = read.accounts.first()
                && let Ok(view) = RelayedObservationRecordViewV1::decode(&account.data)
                && reached(view)
            {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Err(RelayerError::config(format!(
            "the observation record did not reach {condition} within 300 seconds of the \
             transaction landing; the validator may have stopped finalizing"
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
fn publish_log(args: &PublishLogArgs) -> Result<()> {
    let config = Config::load(&args.config, home().as_deref())?;
    let source = config.output_dir.join("publication_log.jsonl");
    let bytes =
        std::fs::read(&source).map_err(|source_error| RelayerError::io(&source, source_error))?;

    std::fs::create_dir_all(&args.to)
        .map_err(|source_error| RelayerError::io(&args.to, source_error))?;
    let destination = args.to.join("publication_log.jsonl");
    if destination.exists() {
        let published = std::fs::read(&destination)
            .map_err(|source_error| RelayerError::io(&destination, source_error))?;
        if bytes.len() < published.len() || bytes.get(..published.len()) != Some(&published[..]) {
            return Err(RelayerError::config(format!(
                "{} is not a prefix of the local log; a published history is append-only, and a \
                 divergent copy means one of the two was rewritten. Refusing to overwrite the \
                 public copy — resolve which history is real first",
                destination.display()
            )));
        }
    }
    let staging = args.to.join("publication_log.jsonl.tmp");
    std::fs::write(&staging, &bytes)
        .map_err(|source_error| RelayerError::io(&staging, source_error))?;
    std::fs::rename(&staging, &destination)
        .map_err(|source_error| RelayerError::io(&destination, source_error))?;

    let lines = bytes.iter().filter(|byte| **byte == b'\n').count();
    let digest = dclutch_relayer::derive::sha256(&bytes);
    let latest = serde_json::json!({
        "schema": "dclutch.relayer.publication-push.v1",
        "log_file": "publication_log.jsonl",
        "lines": lines,
        "byte_len": bytes.len(),
        "sha256_hex": to_hex(&digest),
        "updated_wall_unix_seconds": dclutch_relayer::publog::wall_unix_seconds(),
    });
    let latest_path = args.to.join("LATEST.json");
    let text = serde_json::to_string_pretty(&latest)
        .map_err(|source_error| RelayerError::Serialization(source_error.to_string()))?;
    std::fs::write(&latest_path, text)
        .map_err(|source_error| RelayerError::io(&latest_path, source_error))?;

    println!(
        "pushed {} lines ({} bytes, sha256 {}) to {}",
        lines,
        bytes.len(),
        to_hex(&digest),
        destination.display()
    );
    println!(
        "serve {} statically (any static host works); a verifier fetches LATEST.json, then the \
         log, and checks each line's signature against the pinned relayer key set and each \
         attested account against the observed cluster",
        args.to.display()
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
