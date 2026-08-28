#![forbid(unsafe_code)]

use std::{env, error::Error as StdError, fmt, io::Write, path::PathBuf};

use solana_sdk::pubkey::Pubkey;

mod campaign;
mod cluster;
mod direct_market;
mod flagship_resolution;
// The journey campaign's conservation engine, shared textually the same way
// the journey shares this tree's modules back. Its unused-in-this-binary
// helpers stay allowed the way every #[path] include here is.
#[path = "../../../../gauntlet/journey/src/ledger.rs"]
#[allow(dead_code)]
mod ledger;
mod market;
mod model;
mod plan;
mod relayed;
mod rpc;
mod runtime;
mod seed;
mod terminal_lifecycle;
mod upgrade;
mod user_position_admission;
mod wallet_terminal;

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
struct Error(String);

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl StdError for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<std::time::SystemTimeError> for Error {
    fn from(error: std::time::SystemTimeError) -> Self {
        Self::new(error.to_string())
    }
}

fn main() -> core::result::Result<(), Box<dyn StdError>> {
    match run() {
        Ok(()) => Ok(()),
        Err(error) => Err(Box::new(error)),
    }
}

fn run() -> Result<()> {
    let mut arguments = env::args();
    let _program = arguments.next();
    match arguments.next().as_deref() {
        Some("prepare") => run_prepare(arguments.collect()),
        Some("demo-market") => run_demo_market(arguments.collect()),
        Some("devnet-market") => run_devnet_market(arguments.collect()),
        Some("graduation-market") => run_graduation_market(arguments.collect()),
        Some("ledger-census") => run_ledger_census(arguments.collect()),
        Some("wallet-terminal-payout-input") => {
            terminal_lifecycle::run_wallet_terminal_input(arguments.collect())
        }
        Some("terminal-lifecycle-plan") => {
            terminal_lifecycle::run_terminal_lifecycle_plan(arguments.collect())
        }
        Some("wallet-terminal-payout-alt-plan") => wallet_terminal::run_alt(arguments.collect()),
        Some("wallet-terminal-payout-plan") => wallet_terminal::run(arguments.collect()),
        Some("run") => run_runtime(arguments.collect()),
        Some("campaign") => run_campaign(arguments.collect()),
        Some("devnet-upgrade-baseline-v1") => upgrade::run_baseline(arguments.collect()),
        Some("devnet-upgrade-extend-v1") => upgrade::run_extension(arguments.collect()),
        Some("devnet-upgrade-v1") => upgrade::run(arguments.collect()),
        Some("devnet-user-position-admission-v1") => {
            user_position_admission::run(arguments.collect())
        }
        Some("flagship-resolution-v1") => flagship_resolution::run(arguments.collect()),
        Some("help" | "-h" | "--help") | None => {
            usage();
            Ok(())
        }
        Some(command) => Err(Error::new(format!("unknown command: {command}"))),
    }
}

fn run_runtime(arguments: Vec<String>) -> Result<()> {
    let mut spec = None;
    let mut keypair_seed = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--spec" => &mut spec,
            // Optional and TEST-ONLY. Absent is a fresh unreproducible key per
            // request, which is what this command did before the flag existed.
            "--keypair-seed" => &mut keypair_seed,
            _ => return Err(Error::new(format!("unknown run argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    runtime::execute(&absolute(spec, "--spec")?, keypair_seed.as_deref())
}

/// The devnet driver's command line.
///
/// Two things are deliberately NOT flags here. There is no `--keypair-seed`:
/// the driver's keys come from files the operator holds, and a reproducible
/// key on a public cluster is the footgun `seed.rs` documents at length. And
/// there is no `--force`: every refusal this driver can raise is a statement
/// about the chain, and the fix is to change the chain or the plan, never to
/// tell the tool to stop noticing.
fn run_campaign(arguments: Vec<String>) -> Result<()> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut market = None;
    let mut evidence = None;
    let mut through = None;
    let mut execute = false;
    let mut keypairs = std::collections::BTreeMap::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        // The one valueless flag, matched before anything demands a value.
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
        if let Some(role) = argument.strip_prefix("--keypair-") {
            let role = *campaign::KEYPAIR_ROLES
                .iter()
                .find(|known| **known == role)
                .ok_or_else(|| {
                    Error::new(format!(
                        "--keypair-{role} names no campaign role; the roles are {}",
                        campaign::KEYPAIR_ROLES.join(", ")
                    ))
                })?;
            let secret = campaign::read_keypair_file(&PathBuf::from(&value), role)?;
            if keypairs.insert(role.to_owned(), secret).is_some() {
                return Err(Error::new(format!(
                    "--keypair-{role} may be supplied only once"
                )));
            }
            continue;
        }
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME => &mut acknowledgment,
            "--plan" => &mut plan,
            // The run spec's `market` block as its own JSON document — the
            // founding stage's input. Optional: every earlier stage runs
            // without one, and the founding refuses by name when it is absent.
            "--market" => &mut market,
            "--evidence" => &mut evidence,
            "--through" => &mut through,
            _ => {
                return Err(Error::new(format!("unknown campaign argument: {argument}")));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let origin = cluster::ClusterOriginV1::parse(
        &required(rpc_url, "--rpc-url")?,
        acknowledgment.as_deref(),
    )?;
    let args = campaign::CampaignArgsV1 {
        origin,
        plan_path: absolute(plan, "--plan")?,
        market_path: match market {
            None => None,
            Some(path) => Some(absolute(Some(path), "--market")?),
        },
        evidence_path: match evidence {
            None => None,
            Some(path) => Some(absolute(Some(path), "--evidence")?),
        },
        keypairs,
        execute,
        through: match through.as_deref() {
            None => campaign::StageV1::Founding,
            Some(value) => campaign::StageV1::parse(value)?,
        },
    };
    campaign::execute(args)
}

#[derive(Default)]
struct DirectCompilerArgumentsV1 {
    plan: Option<String>,
    execution_config: Option<String>,
    activation_deadline_slot: Option<String>,
    root_rent_minimum_lamports: Option<String>,
}

impl DirectCompilerArgumentsV1 {
    fn slot(&mut self, argument: &str) -> Option<&mut Option<String>> {
        match argument {
            "--plan" => Some(&mut self.plan),
            "--direct-execution-config" => Some(&mut self.execution_config),
            "--direct-activation-deadline-slot" => Some(&mut self.activation_deadline_slot),
            "--direct-root-rent-minimum-lamports" => Some(&mut self.root_rent_minimum_lamports),
            _ => None,
        }
    }

    fn load(self, registry: Pubkey) -> Result<direct_market::DirectMarketCompilerOwnedV1> {
        let decimal = |value: Option<String>, label: &str| -> Result<u64> {
            required(value, label)?
                .parse::<u64>()
                .map_err(|_| Error::new(format!("{label} must be a decimal u64")))
        };
        direct_market::DirectMarketCompilerOwnedV1::load(
            &absolute(self.plan, "--plan")?,
            &absolute(self.execution_config, "--direct-execution-config")?,
            registry,
            decimal(
                self.activation_deadline_slot,
                "--direct-activation-deadline-slot",
            )?,
            decimal(
                self.root_rent_minimum_lamports,
                "--direct-root-rent-minimum-lamports",
            )?,
        )
    }
}

fn run_demo_market(arguments: Vec<String>) -> Result<()> {
    let mut registry = None;
    let mut direct = DirectCompilerArgumentsV1::default();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--registry-program-id" => Some(&mut registry),
            _ => direct.slot(&argument),
        }
        .ok_or_else(|| Error::new(format!("unknown demo-market argument: {argument}")))?;
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let registry = parse_pubkey(registry, "--registry-program-id")?;
    let direct = direct.load(registry)?;
    let input = market::demo_market_input(registry, direct.compiler())?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&input)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// The devnet flagship's market input: a Pyth range-protection market bound
/// to the committed devnet release row and a LIVE terminal window.
///
/// Every fact is explicit — there is no hidden clock. `--window-start` is unix
/// seconds the operator states (typically now, `date +%s`), and the width is
/// refused below the measured cadence floor rather than founded into a market
/// that fails for provider reasons.
fn run_devnet_market(arguments: Vec<String>) -> Result<()> {
    let mut registry = None;
    let mut price_update = None;
    let mut window_start = None;
    let mut window_width = None;
    let mut max_age = None;
    let mut cut_denominator = None;
    let mut cuts = None;
    let mut coefficients = None;
    let mut product_name = None;
    let mut coordinate_domain_name = None;
    let mut feed_label = None;
    let mut generation = None;
    let mut direct = DirectCompilerArgumentsV1::default();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--registry-program-id" => Some(&mut registry),
            "--price-update" => Some(&mut price_update),
            "--window-start" => Some(&mut window_start),
            "--window-width-seconds" => Some(&mut window_width),
            "--max-age-seconds" => Some(&mut max_age),
            "--cut-denominator" => Some(&mut cut_denominator),
            "--cuts" => Some(&mut cuts),
            "--coefficients" => Some(&mut coefficients),
            "--product" => Some(&mut product_name),
            "--coordinate-domain" => Some(&mut coordinate_domain_name),
            "--feed" => Some(&mut feed_label),
            "--generation" => Some(&mut generation),
            _ => direct.slot(&argument),
        }
        .ok_or_else(|| Error::new(format!("unknown devnet-market argument: {argument}")))?;
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    fn decimal<T: std::str::FromStr>(value: Option<String>, label: &str) -> Result<T> {
        required(value, label)?
            .parse::<T>()
            .map_err(|_| Error::new(format!("{label} must be a decimal number")))
    }
    fn comma_list<T: std::str::FromStr>(value: &str, label: &str) -> Result<Vec<T>> {
        value
            .split(',')
            .map(|item| {
                item.trim()
                    .parse::<T>()
                    .map_err(|_| Error::new(format!("{label} item {item:?} is not a number")))
            })
            .collect()
    }
    let registry = parse_pubkey(registry, "--registry-program-id")?;
    let price_update = std::fs::read(absolute(price_update, "--price-update")?)?;
    let spec = market::DevnetPythMarketSpecV1 {
        registry,
        price_update: &price_update,
        product_name: product_name
            .as_deref()
            .unwrap_or("product/sol-usd-range-protection"),
        coordinate_domain_name: coordinate_domain_name
            .as_deref()
            .unwrap_or("coordinate-domain/usd-cents-per-sol"),
        feed_label: feed_label.as_deref().unwrap_or("sol-usd").as_bytes(),
        window_start: decimal::<i64>(window_start, "--window-start")?,
        // 1,800 s: ~5.75 measured cadences, ~99.7% coverage — the runbook's
        // "a market that should not fail for provider reasons" width.
        window_width_seconds: match window_width {
            None => 1_800,
            Some(value) => decimal::<u32>(Some(value), "--window-width-seconds")?,
        },
        // Submission-latency budget against the known 4,784 s devnet outage.
        max_age_seconds: match max_age {
            None => 7_200,
            Some(value) => decimal::<u32>(Some(value), "--max-age-seconds")?,
        },
        cut_denominator: match cut_denominator {
            None => 100,
            Some(value) => decimal::<u64>(Some(value), "--cut-denominator")?,
        },
        cuts: comma_list::<i128>(cuts.as_deref().unwrap_or("12000,18000"), "--cuts")?,
        coefficients: comma_list::<u64>(
            coefficients.as_deref().unwrap_or("1,0,1,0"),
            "--coefficients",
        )?,
        generation: match generation {
            None => 1,
            Some(value) => decimal::<u64>(Some(value), "--generation")?,
        },
    };
    let direct = direct.load(registry)?;
    let input = market::devnet_market_input(spec, direct.compiler())?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&input)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// One conservation-ledger census against a live cluster.
///
/// The engine is the journey campaign's own `ConservationLedgerV1` — the
/// same seven laws, the same arithmetic — pointed at an EXTERNAL chain
/// through the driver's origin rails instead of at a validator the journey
/// launched. Each invocation takes one census and evaluates every law;
/// `--prior` reloads a previous invocation's observations so the delta laws
/// (Hoard movement declared, tracked-set movement declared) evaluate across
/// process boundaries. The lamport law records itself inapplicable with the
/// stated reason: this census does not drive the transactions between
/// boundaries and refuses to guess their fees.
///
/// Exit is nonzero if ANY law is violated at the new boundary.
fn run_ledger_census(arguments: Vec<String>) -> Result<()> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut mint = None;
    let mut payer = None;
    let mut hoard = None;
    let mut aggregate = None;
    let mut claim_unit = None;
    let mut stage = None;
    let mut declared_collateral = None;
    let mut declared_hoard = None;
    let mut prior = None;
    let mut output = None;
    let mut tokens: Vec<(String, Pubkey)> = Vec::new();
    let mut positions: Vec<(String, Pubkey)> = Vec::new();
    let mut watches: Vec<(String, Pubkey)> = Vec::new();
    fn labeled(value: &str, flag: &str) -> Result<(String, Pubkey)> {
        let (label, address) = value
            .split_once('=')
            .ok_or_else(|| Error::new(format!("{flag} takes LABEL=PUBKEY, got {value:?}")))?;
        Ok((label.to_owned(), plan::pubkey(address)?))
    }
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        match argument.as_str() {
            "--token" => {
                tokens.push(labeled(&value, "--token")?);
                continue;
            }
            "--position" => {
                positions.push(labeled(&value, "--position")?);
                continue;
            }
            "--watch" => {
                watches.push(labeled(&value, "--watch")?);
                continue;
            }
            _ => {}
        }
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME => &mut acknowledgment,
            "--mint" => &mut mint,
            "--payer" => &mut payer,
            "--hoard" => &mut hoard,
            "--aggregate" => &mut aggregate,
            "--claim-unit-atoms" => &mut claim_unit,
            "--stage" => &mut stage,
            "--declared-collateral-delta" => &mut declared_collateral,
            "--declared-hoard-delta" => &mut declared_hoard,
            "--prior" => &mut prior,
            "--output" => &mut output,
            _ => {
                return Err(Error::new(format!(
                    "unknown ledger-census argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let origin = cluster::ClusterOriginV1::parse(
        &required(rpc_url, "--rpc-url")?,
        acknowledgment.as_deref(),
    )?;
    let mut rpc = rpc::Rpc::connect_cluster(&origin, rpc::WritePolicyV1::ReadsOnly)?;
    let mut census = ledger::ConservationLedgerV1::new(
        parse_pubkey(mint, "--mint")?,
        parse_pubkey(payer, "--payer")?,
    );
    if let Some(path) = prior {
        let observations: Vec<ledger::ObservationV1> =
            serde_json::from_slice(&std::fs::read(absolute(Some(path), "--prior")?)?)?;
        census.restore_observations(observations);
    }
    for (label, address) in &tokens {
        census.track_token_account(label, *address);
    }
    for (label, address) in &positions {
        census.track_position(label, *address);
    }
    for (label, address) in &watches {
        census.watch(label, *address);
    }
    census.admit_founding(
        parse_pubkey(hoard, "--hoard")?,
        parse_pubkey(aggregate, "--aggregate")?,
        required(claim_unit, "--claim-unit-atoms")?
            .parse::<u64>()
            .map_err(|_| Error::new("--claim-unit-atoms must be a decimal u64"))?,
    );
    let parse_delta = |value: Option<String>, label: &str| -> Result<i128> {
        match value {
            None => Ok(0),
            Some(text) => text
                .parse::<i128>()
                .map_err(|_| Error::new(format!("{label} must be a decimal i128"))),
        }
    };
    census.observe(
        &mut rpc,
        &required(stage, "--stage")?,
        parse_delta(declared_collateral, "--declared-collateral-delta")?,
        parse_delta(declared_hoard, "--declared-hoard-delta")?,
        ledger::LamportClaimV1::inapplicable(
            "external census: the transactions between boundaries were not driven by this \
             ledger, and it refuses to guess their fees",
        ),
    )?;
    let observations = census.observations();
    std::fs::write(
        absolute(output, "--output")?,
        serde_json::to_vec_pretty(&observations)?,
    )?;
    let newest = observations
        .last()
        .ok_or_else(|| Error::new("census produced no observation"))?;
    let mut violated = 0_usize;
    for verdict in &newest.verdicts {
        if verdict.status == "violated" {
            violated += 1;
        }
        println!(
            "{} {}: {}",
            verdict.status.to_uppercase(),
            verdict.law,
            verdict.detail
        );
    }
    if violated > 0 {
        return Err(Error::new(format!(
            "{violated} conservation law(s) VIOLATED at stage {}",
            newest.stage
        )));
    }
    Ok(())
}

/// The relayed graduation market's input, over venue facts READ OFF REAL
/// MAINNET and the operated relayer's disclosed attestation key.
///
/// Everything is explicit: the watched pool, the venue's observed deployment
/// slot / authority / ELF digest, the window, the relayer key. The compiler is
/// the SAME one the relayed-vertical rehearsal exercises (`relayed.rs`), so
/// the shapes cannot drift — only whose facts they pin.
fn run_graduation_market(arguments: Vec<String>) -> Result<()> {
    let mut registry = None;
    let mut relayer = None;
    let mut pool = None;
    let mut venue_program = None;
    let mut venue_programdata = None;
    let mut venue_slot = None;
    let mut venue_authority = None;
    let mut venue_elf_sha256 = None;
    let mut window_start = None;
    let mut window_end = None;
    let mut max_age = None;
    let mut direct = DirectCompilerArgumentsV1::default();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--registry-program-id" => Some(&mut registry),
            "--relayer-attestation" => Some(&mut relayer),
            "--pool" => Some(&mut pool),
            "--venue-program" => Some(&mut venue_program),
            "--venue-programdata" => Some(&mut venue_programdata),
            "--venue-deployment-slot" => Some(&mut venue_slot),
            "--venue-upgrade-authority" => Some(&mut venue_authority),
            "--venue-elf-sha256" => Some(&mut venue_elf_sha256),
            "--window-start" => Some(&mut window_start),
            "--window-end" => Some(&mut window_end),
            "--max-age-seconds" => Some(&mut max_age),
            _ => direct.slot(&argument),
        }
        .ok_or_else(|| Error::new(format!("unknown graduation-market argument: {argument}")))?;
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    fn decimal<T: std::str::FromStr>(value: Option<String>, label: &str) -> Result<T> {
        required(value, label)?
            .parse::<T>()
            .map_err(|_| Error::new(format!("{label} must be a decimal number")))
    }
    fn hex32(value: Option<String>, label: &str) -> Result<[u8; 32]> {
        let text = required(value, label)?;
        let bytes = (0..64)
            .step_by(2)
            .map(|index| u8::from_str_radix(text.get(index..index + 2).unwrap_or("zz"), 16))
            .collect::<core::result::Result<Vec<_>, _>>()
            .map_err(|_| Error::new(format!("{label} must be 64 hex digits")))?;
        if text.len() != 64 {
            return Err(Error::new(format!("{label} must be 64 hex digits")));
        }
        let mut output = [0_u8; 32];
        output.copy_from_slice(&bytes);
        Ok(output)
    }
    let registry = parse_pubkey(registry, "--registry-program-id")?;
    // Meteora DBC's real mainnet addresses, as `twin.rs` and the relay dossier
    // pin them; overridable for a different venue.
    let venue = relayed::RelayedVenueFactsV1 {
        program: parse_pubkey(
            venue_program.or_else(|| Some("dbcij3LWUppWqq96dh6gJWwBifmcGfLSB5D4DuSMaqN".into())),
            "--venue-program",
        )?
        .to_bytes(),
        programdata: parse_pubkey(
            venue_programdata
                .or_else(|| Some("HUfnSSiJxgspQm6C1rkqv6L3XgVtn7AESApgCQpCXCYh".into())),
            "--venue-programdata",
        )?
        .to_bytes(),
        pool: parse_pubkey(pool, "--pool")?.to_bytes(),
        elf_digest: hex32(venue_elf_sha256, "--venue-elf-sha256")?,
        deployment_slot: decimal::<u64>(venue_slot, "--venue-deployment-slot")?,
        upgrade_authority: parse_pubkey(venue_authority, "--venue-upgrade-authority")?.to_bytes(),
    };
    let window = relayed::WindowChoiceV1 {
        start_unix_seconds: decimal::<i64>(window_start, "--window-start")?,
        end_unix_seconds: decimal::<i64>(window_end, "--window-end")?,
        max_age_seconds: decimal::<u32>(max_age, "--max-age-seconds")?,
    };
    let relayer = parse_pubkey(relayer, "--relayer-attestation")?;
    let direct = direct.load(registry)?;
    let facts = relayed::relayed_market_input(
        registry,
        relayer.to_bytes(),
        &window,
        &venue,
        direct.compiler(),
    )?;
    let hex = |bytes: &[u8]| plan::hex(bytes);
    let report = serde_json::json!({
        "schema": "dclutch-graduation-market-input-v1",
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
        "walk_bounty_lamports": relayed::WALK_BOUNTY_LAMPORTS,
        "admitted_principal_atoms": facts.admitted_principal_atoms.to_string(),
        "admitted_principal_cap_atoms": facts.admitted_principal_cap_atoms.to_string(),
        "disclosed_failure_conflation": relayed::DISCLOSED_FAILURE_CONFLATION,
    });
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&report)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn run_prepare(arguments: Vec<String>) -> Result<()> {
    let mut account_dir = None;
    let mut output = None;
    let mut registry_program = None;
    let mut registry_elf = None;
    let mut registry_sha256 = None;
    let mut registry_semantic_release = None;
    let mut core_program = None;
    let mut core_elf = None;
    let mut core_sha256 = None;
    let mut core_semantic_release = None;
    let mut core_bootstrap_upgrade_authority = None;
    let mut claims_program = None;
    let mut claims_elf = None;
    let mut claims_sha256 = None;
    let mut claims_semantic_release = None;
    let mut trading_program = None;
    let mut trading_elf = None;
    let mut trading_sha256 = None;
    let mut trading_semantic_release = None;
    let mut resolution_program = None;
    let mut resolution_elf = None;
    let mut resolution_sha256 = None;
    let mut resolution_semantic_release = None;
    let mut custody_program = None;
    let mut custody_elf = None;
    let mut custody_sha256 = None;
    let mut custody_semantic_release = None;
    let mut rent_credit_program = None;
    let mut rent_credit_elf = None;
    let mut rent_credit_sha256 = None;
    let mut rent_credit_semantic_release = None;
    let mut record_publication = None;
    // Seven optional observed Loader V3 ProgramData accounts and seven optional
    // genesis-install slots. The DEPLOYMENT SLOT is never one of these values:
    // it is hostile-decoded out of the resulting account image by exactly the
    // parse the on-chain authenticator runs. What these choose is which image.
    let mut deployments = plan::RoleDeploymentsV1::default();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        if let Some(rest) = argument.strip_prefix("--") {
            if let Some(role) = rest.strip_suffix("-live-elf-sha256")
                && let Some(target) = deployment_target(&mut deployments, role)
            {
                if target
                    .expected_live_elf_sha256
                    .replace(value.clone())
                    .is_some()
                {
                    return Err(Error::new(format!("{argument} may be supplied only once")));
                }
                continue;
            }
            if let Some(role) = rest.strip_suffix("-observed-programdata")
                && let Some(target) = deployment_target(&mut deployments, role)
            {
                if target
                    .observed_programdata
                    .replace(PathBuf::from(&value))
                    .is_some()
                {
                    return Err(Error::new(format!("{argument} may be supplied only once")));
                }
                continue;
            }
            if let Some(role) = rest.strip_suffix("-expected-upgrade-authority")
                && let Some(target) = deployment_target(&mut deployments, role)
            {
                if target
                    .expected_upgrade_authority
                    .replace(parse_pubkey(Some(value.clone()), &argument)?)
                    .is_some()
                {
                    return Err(Error::new(format!("{argument} may be supplied only once")));
                }
                continue;
            }
            if let Some(role) = rest.strip_suffix("-genesis-deployment-slot")
                && let Some(target) = deployment_target(&mut deployments, role)
            {
                if target.genesis_deployment_slot != 0 {
                    return Err(Error::new(format!("{argument} may be supplied only once")));
                }
                target.genesis_deployment_slot = value.parse::<u64>().map_err(|_| {
                    Error::new(format!("{argument} must be a decimal u64 slot number"))
                })?;
                if target.genesis_deployment_slot == 0 {
                    return Err(Error::new(format!(
                        "{argument} is zero, which is what absence already means"
                    )));
                }
                continue;
            }
        }
        let slot = match argument.as_str() {
            "--account-dir" => &mut account_dir,
            "--output" => &mut output,
            "--registry-program-id" => &mut registry_program,
            "--registry-elf" => &mut registry_elf,
            "--registry-sha256" => &mut registry_sha256,
            "--registry-semantic-release-id" => &mut registry_semantic_release,
            "--core-program-id" => &mut core_program,
            "--core-elf" => &mut core_elf,
            "--core-sha256" => &mut core_sha256,
            "--core-semantic-release-id" => &mut core_semantic_release,
            "--core-bootstrap-upgrade-authority" => &mut core_bootstrap_upgrade_authority,
            "--claims-program-id" => &mut claims_program,
            "--claims-elf" => &mut claims_elf,
            "--claims-sha256" => &mut claims_sha256,
            "--claims-semantic-release-id" => &mut claims_semantic_release,
            "--trading-program-id" => &mut trading_program,
            "--trading-elf" => &mut trading_elf,
            "--trading-sha256" => &mut trading_sha256,
            "--trading-semantic-release-id" => &mut trading_semantic_release,
            "--resolution-program-id" => &mut resolution_program,
            "--resolution-elf" => &mut resolution_elf,
            "--resolution-sha256" => &mut resolution_sha256,
            "--resolution-semantic-release-id" => &mut resolution_semantic_release,
            "--custody-program-id" => &mut custody_program,
            "--custody-elf" => &mut custody_elf,
            "--custody-sha256" => &mut custody_sha256,
            "--custody-semantic-release-id" => &mut custody_semantic_release,
            "--rent-credit-program-id" => &mut rent_credit_program,
            "--rent-credit-elf" => &mut rent_credit_elf,
            "--rent-credit-sha256" => &mut rent_credit_sha256,
            "--rent-credit-semantic-release-id" => &mut rent_credit_semantic_release,
            // Optional. Absent is `genesis`, which is byte-for-byte what this
            // subcommand did before the flag existed.
            "--record-publication" => &mut record_publication,
            _ => return Err(Error::new(format!("unknown prepare argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let args = plan::PrepareArgs {
        account_dir: absolute(account_dir, "--account-dir")?,
        plan_path: absolute(output, "--output")?,
        registry_program: parse_pubkey(registry_program, "--registry-program-id")?,
        registry_elf: absolute(registry_elf, "--registry-elf")?,
        registry_sha256: required(registry_sha256, "--registry-sha256")?,
        registry_semantic_release_id: required(
            registry_semantic_release,
            "--registry-semantic-release-id",
        )?,
        core_program: parse_pubkey(core_program, "--core-program-id")?,
        core_elf: absolute(core_elf, "--core-elf")?,
        core_sha256: required(core_sha256, "--core-sha256")?,
        core_semantic_release_id: required(core_semantic_release, "--core-semantic-release-id")?,
        core_bootstrap_upgrade_authority: parse_pubkey(
            core_bootstrap_upgrade_authority,
            "--core-bootstrap-upgrade-authority",
        )?,
        claims_program: parse_pubkey(claims_program, "--claims-program-id")?,
        claims_elf: absolute(claims_elf, "--claims-elf")?,
        claims_sha256: required(claims_sha256, "--claims-sha256")?,
        claims_semantic_release_id: required(
            claims_semantic_release,
            "--claims-semantic-release-id",
        )?,
        trading_program: parse_pubkey(trading_program, "--trading-program-id")?,
        trading_elf: absolute(trading_elf, "--trading-elf")?,
        trading_sha256: required(trading_sha256, "--trading-sha256")?,
        trading_semantic_release_id: required(
            trading_semantic_release,
            "--trading-semantic-release-id",
        )?,
        resolution_program: parse_pubkey(resolution_program, "--resolution-program-id")?,
        resolution_elf: absolute(resolution_elf, "--resolution-elf")?,
        resolution_sha256: required(resolution_sha256, "--resolution-sha256")?,
        resolution_semantic_release_id: required(
            resolution_semantic_release,
            "--resolution-semantic-release-id",
        )?,
        custody_program: parse_pubkey(custody_program, "--custody-program-id")?,
        custody_elf: absolute(custody_elf, "--custody-elf")?,
        custody_sha256: required(custody_sha256, "--custody-sha256")?,
        custody_semantic_release_id: required(
            custody_semantic_release,
            "--custody-semantic-release-id",
        )?,
        rent_credit_program: parse_pubkey(rent_credit_program, "--rent-credit-program-id")?,
        rent_credit_elf: absolute(rent_credit_elf, "--rent-credit-elf")?,
        rent_credit_sha256: required(rent_credit_sha256, "--rent-credit-sha256")?,
        rent_credit_semantic_release_id: required(
            rent_credit_semantic_release,
            "--rent-credit-semantic-release-id",
        )?,
        record_publication: match record_publication.as_deref() {
            None => plan::RecordPublicationV1::Genesis,
            Some(value) => plan::RecordPublicationV1::parse(value)?,
        },
        deployments,
    };
    let path = args.plan_path.clone();
    let prepared = plan::prepare(args)?;
    let summary = serde_json::json!({
        "schema": "dclutch-local-successor-prepare-result-v1",
        "plan": path,
        "account_dir": prepared.account_dir,
        "registry_program_id": prepared.registry.program_id,
        "core_program_id": prepared.core.program_id,
        "claims_program_id": prepared.claims.program_id,
        "trading_program_id": prepared.trading.program_id,
        "resolution_program_id": prepared.resolution.program_id,
        "custody_program_id": prepared.custody.program_id,
        "rent_credit_program_id": prepared.rent_credit.program_id,
        "genesis_account_count": prepared.genesis_accounts.len(),
    });
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&summary)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Resolve one launcher role name to its deployment-source slot.
///
/// `None` means the name is not a role, which lets the caller fall through to
/// the ordinary flag table instead of swallowing a typo.
fn deployment_target<'a>(
    deployments: &'a mut plan::RoleDeploymentsV1,
    role: &str,
) -> Option<&'a mut plan::RoleDeploymentInputV1> {
    match role {
        "registry" => Some(&mut deployments.registry),
        "core" => Some(&mut deployments.core),
        "claims" => Some(&mut deployments.claims),
        "trading" => Some(&mut deployments.trading),
        "resolution" => Some(&mut deployments.resolution),
        "custody" => Some(&mut deployments.custody),
        "rent-credit" => Some(&mut deployments.rent_credit),
        _ => None,
    }
}

fn required(value: Option<String>, label: &str) -> Result<String> {
    value.ok_or_else(|| Error::new(format!("{label} is required")))
}

fn absolute(value: Option<String>, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(required(value, label)?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

fn parse_pubkey(value: Option<String>, label: &str) -> Result<Pubkey> {
    plan::pubkey(&required(value, label)?)
}

fn usage() {
    usage_supervisor();
    println!("{}", upgrade::usage());
    println!("{}", terminal_lifecycle::usage());
    println!("{}", terminal_lifecycle::lifecycle_usage());
    println!("{}", user_position_admission::usage());
    println!("{}", flagship_resolution::usage());
    println!("{}", wallet_terminal::usage());
    println!(
        "\n  dclutch-local-successor-bootstrap campaign --rpc-url URL [{ack} GENESIS_HASH] --plan \
         ABSOLUTE_JSON [--market ABSOLUTE_JSON] --keypair-ROLE ABSOLUTE_KEYPAIR_JSON... \
         [--evidence ABSOLUTE_JSON] [--through STAGE] [--execute]\n\nThe campaign command is the \
         EXTERNAL-CLUSTER driver. It \
         launches no validator, holds no ephemeral authority, and signs only with keypair files \
         you hold. Default is PREFLIGHT: the connection is opened read-only and a method \
         allowlist refuses anything that is not a read, so a preflight cannot write rather than \
         intending not to. --execute opts into writing.\n\nORIGIN. A loopback origin needs no \
         ceremony. Any other origin is refused unless {ack} names devnet's genesis hash in full, \
         and the cluster's own getGenesisHash is checked against it at connect. {help}\n\nSTAGES \
         (--through, default founding): {stages}. Every stage detects its own completion by \
         READING THE CHAIN, never from a state file, so re-running after any failure is always \
         safe -- which is the shape devnet requires, because devnet dies mid-ladder. `substrate` \
         never writes: this driver does not deploy programs and has no code path that could. It \
         reports each role's observed deployment slot, exact upgrade authority, Loader owner, \
         ProgramData privilege, and complete live ELF digest against the release pins. Under \
         decision 0012 any mismatch is fail-closed, not a deploy error.\n\nROLES for \
         --keypair-ROLE: {roles}. Index 0 of each role is that \
         file's own key, so the address `solana address -k FILE` prints is the address you fund. \
         There is no --keypair-seed here: a reproducible private key on a public cluster is the \
         footgun seed.rs documents.",
        ack = campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME,
        help = campaign::acknowledgment_help(),
        stages = campaign::StageV1::ORDER
            .iter()
            .map(|stage| stage.name())
            .collect::<Vec<_>>()
            .join(", "),
        roles = campaign::KEYPAIR_ROLES.join(", "),
    );
    println!(
        "\n  dclutch-local-successor-bootstrap devnet-market --registry-program-id PUBKEY \
         --plan ABSOLUTE_JSON --direct-execution-config ABSOLUTE_FILE \
         --direct-activation-deadline-slot U64 --direct-root-rent-minimum-lamports U64 \
         --price-update ABSOLUTE_FILE --window-start UNIX_SECONDS [--window-width-seconds U32] \
         [--max-age-seconds U32] [--cut-denominator U64] [--cuts I128,..] [--coefficients U64,..] \
         [--product NAME] [--coordinate-domain NAME] [--feed LABEL] [--generation U64]\n  \
         dclutch-local-successor-bootstrap graduation-market --registry-program-id PUBKEY \
         --plan ABSOLUTE_JSON --direct-execution-config ABSOLUTE_FILE \
         --direct-activation-deadline-slot U64 --direct-root-rent-minimum-lamports U64 \
         --relayer-attestation PUBKEY --pool PUBKEY --venue-deployment-slot U64 \
         --venue-upgrade-authority PUBKEY --venue-elf-sha256 HEX64 --window-start I64 \
         --window-end I64 --max-age-seconds U32 [--venue-program PUBKEY] \
         [--venue-programdata PUBKEY]\n  dclutch-local-successor-bootstrap ledger-census \
         --rpc-url URL [{ack} GENESIS_HASH] --mint PUBKEY --payer PUBKEY --hoard PUBKEY \
         --aggregate PUBKEY --claim-unit-atoms U64 --stage NAME --output ABSOLUTE_JSON \
         [--token LABEL=PUBKEY]... [--position LABEL=PUBKEY]... [--watch LABEL=PUBKEY]... \
         [--prior ABSOLUTE_JSON] [--declared-collateral-delta I128] [--declared-hoard-delta I128]\n\
         \nThe market producers print a MarketRunInput document for the campaign's --market flag: \
         devnet-market the Pyth range-protection flagship (live PriceUpdateV2 body, window width \
         refused below the measured 1,252 s cadence floor), graduation-market the relayed \
         graduation market over venue facts read off real mainnet. ledger-census takes one \
         conservation-ledger census against a live cluster (reads only, enforced) and exits \
         nonzero on any violated law; --prior reloads a previous census so the delta laws \
         evaluate across invocations.",
        ack = campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME,
    );
}

fn usage_supervisor() {
    println!(
        "Usage:\n  dclutch-local-successor-bootstrap prepare --account-dir ABSOLUTE_NEW_DIR --output ABSOLUTE_NEW_JSON --registry-program-id PUBKEY --registry-elf ABSOLUTE_RAW_ELF --registry-sha256 RAW_SHA256 --registry-semantic-release-id SHA256 --core-program-id PUBKEY --core-elf ABSOLUTE_RAW_ELF --core-sha256 RAW_SHA256 --core-semantic-release-id SHA256 --core-bootstrap-upgrade-authority PUBKEY --claims-program-id PUBKEY --claims-elf ABSOLUTE_RAW_ELF --claims-sha256 RAW_SHA256 --claims-semantic-release-id SHA256 --trading-program-id PUBKEY --trading-elf ABSOLUTE_RAW_ELF --trading-sha256 RAW_SHA256 --trading-semantic-release-id SHA256 --resolution-program-id PUBKEY --resolution-elf ABSOLUTE_RAW_ELF --resolution-sha256 RAW_SHA256 --resolution-semantic-release-id SHA256 --custody-program-id PUBKEY --custody-elf ABSOLUTE_RAW_ELF --custody-sha256 RAW_SHA256 --custody-semantic-release-id SHA256 --rent-credit-program-id PUBKEY --rent-credit-elf ABSOLUTE_RAW_ELF --rent-credit-sha256 RAW_SHA256 --rent-credit-semantic-release-id SHA256 [--record-publication genesis|transaction] [--ROLE-observed-programdata ABSOLUTE_ACCOUNT_BODY --ROLE-live-elf-sha256 LIVE_SHA256] [--ROLE-genesis-deployment-slot U64] [--ROLE-expected-upgrade-authority PUBKEY]\n  dclutch-local-successor-bootstrap run --spec ABSOLUTE_JSON [--keypair-seed 64_LOWERCASE_HEX]\n  dclutch-local-successor-bootstrap demo-market --registry-program-id PUBKEY\n\nThe run command is the canonical same-process supervisor. It creates one ephemeral Core authority only in memory, prepares its public key into fresh genesis inputs, starts a guarded foreground localhost validator, initializes Core infrastructure, proves pre-revocation release refusal, revokes Loader-v3 authority to None, and activates the immutable release set. It never reads a wallet or CLI configuration.\n\nA ROLE is one of registry, core, claims, trading, resolution, custody, rent-credit. --ROLE-elf and --ROLE-sha256 always name the exact checked RAW build candidate. --ROLE-observed-programdata takes a complete Loader V3 ProgramData account body read off a cluster; it requires --ROLE-live-elf-sha256 naming the complete live ELF tail, including allocation padding. Prepare proves the live tail is exactly the raw candidate followed only by zero bytes, retains both digests and the padding width in the plan, and binds ArtifactReleaseV1 to the LIVE digest. The live digest without an observation, or an observation with only the old raw digest, refuses. --ROLE-genesis-deployment-slot writes a slot into the genesis install this plan materializes so a LOCAL rehearsal exercises a nonzero deployment slot. It is mutually exclusive with an observation, and no flag supplies the slot the release binds: that is always hostile-decoded out of the resulting ProgramData image by the same parse the on-chain authenticator runs.\n\n--ROLE-expected-upgrade-authority DECLARES the upgrade authority an observed ProgramData carries, for the mutable substrate decision 0012 chose. Like the slot, it does NOT supply what the release binds -- the authority is decoded out of the observation itself -- it is the declaration the observation is CHECKED AGAINST, so a role that quietly became mutable, or mutable under a key nobody named, still refuses at plan time instead of minting a release that hands upgrade rights to a stranger. Absent means the caller declares none, which is what every invocation before 0012 meant. It describes an observation and is refused against a genesis install. A role observed mutable mints ArtifactUpgradePolicyV1::ExactAuthority naming exactly that key; a revoked one mints Immutable as before. Core is the one role whose answer depends on the campaign: a genesis-installed Core binds None because the supervisor revokes it, while an observed Core binds what it carries because the external driver has no revoke stage -- and a plan of the second kind is one the run supervisor refuses.\n\n--keypair-seed is a TEST-ONLY affordance and is REFUSED unless the spec's RPC endpoint is on localhost or 127.0.0.1. With it, every signing key the campaign generates is derived deterministically as SHA-256(domain || 0 || seed || 0 || role-name || 0 || per-role index) read as an ed25519 secret seed, which collapses the find_program_address bump-search noise the gauntlet's compute budgets have to tolerate. It also makes every one of those private keys reproducible by anyone who can read the seed off a command line, a shell history or a checked-in script, so on any public cluster it would hand a stranger the campaign's funded accounts, mint authorities and upgrade authorities. Default is a fresh unreproducible key per request."
    );
}
