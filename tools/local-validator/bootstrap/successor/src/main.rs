#![forbid(unsafe_code)]

use std::{env, error::Error as StdError, fmt, io::Write, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use solana_sdk::pubkey::Pubkey;

mod campaign;
mod cluster;
mod direct_market;
mod direct_trade;
mod direct_trade_producer;
mod direct_trade_setup;
mod direct_trade_setup_journal;
mod direct_trade_token_setup;
mod flagship_resolution;
mod funding_readiness;
mod local_mutable;
// The journey campaign's conservation engine, shared textually the same way
// the journey shares this tree's modules back. Its unused-in-this-binary
// helpers stay allowed the way every #[path] include here is.
#[path = "../../../../gauntlet/journey/src/ledger.rs"]
#[allow(dead_code)]
mod ledger;
mod market;
mod model;
mod plan;
mod private_activity;
mod private_lifecycle;
mod pyth_vaa_provisioning;
mod relayed;
mod release_capture;
mod rpc;
mod runtime;
mod seed;
mod sponsored_push;
mod terminal_exterior_pyth;
mod terminal_lifecycle;
mod terminal_sequence;
mod upgrade;
mod user_position_admission;
mod wallet_terminal;
mod wallet_terminal_payout_exterior;

type Result<T> = core::result::Result<T, Error>;
const PUBLIC_TERMINAL_COMMANDS_V1: [&str; 1] = ["devnet-terminal-sequence-v1"];
const OWNED_LOOPBACK_TERMINAL_COMMANDS_V1: [&str; 2] = [
    "local-private-validator-flagship-resolution-v1",
    "local-private-validator-terminal-sequence-v1",
];

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
        Some(command)
            if command == terminal_lifecycle::OWNED_LOOPBACK_WALLET_TERMINAL_INPUT_COMMAND_V1 =>
        {
            terminal_lifecycle::run_wallet_terminal_input_owned_loopback_v1(arguments.collect())
        }
        Some(command) if command == PUBLIC_TERMINAL_COMMANDS_V1[0] => {
            terminal_sequence::run_terminal_sequence(arguments.collect())
        }
        Some("wallet-terminal-payout-alt-plan") => wallet_terminal::run_alt(arguments.collect()),
        Some("wallet-terminal-payout-plan") => wallet_terminal::run(arguments.collect()),
        Some("run") => run_runtime(arguments.collect()),
        Some("campaign") => run_campaign(arguments.collect()),
        Some("devnet-upgrade-baseline-v1") => upgrade::run_baseline(arguments.collect()),
        Some("devnet-upgrade-extend-v1") => upgrade::run_extension(arguments.collect()),
        Some("devnet-upgrade-v1") => upgrade::run(arguments.collect()),
        Some("devnet-deployment-set-journal-v2") => upgrade::run_set_journal(arguments.collect()),
        Some("devnet-carry-forward-capture-v1") => {
            release_capture::run_carry_forward(arguments.collect())
        }
        Some("devnet-prepare-programdata-capture-v1") => {
            release_capture::run_prepare_programdata(arguments.collect())
        }
        Some("devnet-permanent-substrate-capture-v1") => {
            release_capture::run_permanent_substrate(arguments.collect())
        }
        Some("devnet-user-position-admission-v1") => {
            user_position_admission::run(arguments.collect())
        }
        Some("local-private-validator-user-position-admission-v1") => {
            user_position_admission::run_owned_loopback(arguments.collect())
        }
        Some("local-private-validator-wallet-terminal-payout-v1") => {
            wallet_terminal_payout_exterior::run(arguments.collect())
        }
        Some("devnet-direct-trade-v1") => direct_trade::run_devnet(arguments.collect()),
        Some("local-private-validator-direct-trade-v1") => {
            direct_trade::run_owned_loopback(arguments.collect())
        }
        Some("local-private-validator-direct-trade-produce-v1") => {
            direct_trade_producer::run_owned_loopback(arguments.collect())
        }
        Some("flagship-resolution-v1") => flagship_resolution::run(arguments.collect()),
        Some("devnet-sponsored-push-v1") => sponsored_push::run_devnet(arguments.collect()),
        Some("local-private-validator-sponsored-push-v1") => {
            sponsored_push::run_owned_loopback(arguments.collect())
        }
        Some(command) if command == OWNED_LOOPBACK_TERMINAL_COMMANDS_V1[0] => {
            flagship_resolution::run_owned_loopback(arguments.collect())
        }
        Some("devnet-pyth-vaa-provision-v1") => pyth_vaa_provisioning::run(arguments.collect()),
        Some("local-private-validator-pyth-vaa-provision-v1") => {
            terminal_exterior_pyth::run(arguments.collect())
        }
        Some("local-private-validator-pyth-provider-closure-v1") => {
            terminal_exterior_pyth::run_provider_closure(arguments.collect())
        }
        Some(command) if command == private_activity::STAGE_COMMAND_V1 => {
            let parsed = private_activity::parse_stage_args(arguments.collect::<Vec<_>>())?;
            let value = private_activity::run_stage(parsed)?;
            stdout_json_value_v1(&value)
        }
        Some(command) if command == private_activity::MANIFEST_COMMAND_V1 => {
            let parsed = private_activity::parse_manifest_args(arguments.collect::<Vec<_>>())?;
            let value = private_activity::run_manifest(parsed)?;
            stdout_json_value_v1(&value)
        }
        Some(command) if command == private_activity::CAPTURE_COMMAND_V1 => {
            let parsed = private_activity::parse_capture_args(arguments.collect::<Vec<_>>())?;
            let value = private_activity::run_capture(parsed)?;
            stdout_json_value_v1(&value)
        }
        Some(command) if command == private_activity::LIFECYCLE_SESSION_COMMAND_V1 => {
            let parsed =
                private_activity::parse_lifecycle_session_args(arguments.collect::<Vec<_>>())?;
            let value = private_activity::run_lifecycle_session(parsed)?;
            stdout_json_value_v1(&value)
        }
        Some(command) if command == private_lifecycle::COMMAND_V1 => {
            let parsed = private_lifecycle::parse_args(arguments.collect::<Vec<_>>())?;
            let value = private_lifecycle::run(parsed)?;
            stdout_json_value_v1(&value)
        }
        Some(command) if command == private_lifecycle::DIRECT_PAYOUT_SCHEDULE_COMMAND_V1 => {
            let parsed = private_lifecycle::parse_direct_payout_schedule_args(
                arguments.collect::<Vec<_>>(),
            )?;
            let value = private_lifecycle::run_direct_payout_schedule(parsed)?;
            stdout_json_value_v1(&value)
        }
        Some("local-mutable-prepare-v1") => local_mutable::run_prepare(arguments.collect()),
        Some("local-mutable-plan-authenticate-v1") => {
            local_mutable::run_authenticate(arguments.collect())
        }
        Some("local-private-validator-market-v1") => local_mutable::run_market(arguments.collect()),
        Some(command) if command == OWNED_LOOPBACK_TERMINAL_COMMANDS_V1[1] => {
            terminal_sequence::run_terminal_sequence_owned_loopback_v1(arguments.collect())
        }
        Some("help" | "-h" | "--help") | None => {
            usage();
            Ok(())
        }
        Some(command) => Err(Error::new(format!("unknown command: {command}"))),
    }
}

fn stdout_json_value_v1(value: &serde_json::Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value)?;
    stdout.write_all(b"\n")?;
    Ok(())
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
    campaign::execute(parse_campaign_args_v1(arguments)?)
}

fn parse_campaign_args_v1(arguments: Vec<String>) -> Result<campaign::CampaignArgsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut market = None;
    let mut evidence = None;
    let mut through = None;
    let mut founding_founder = None;
    let mut substituted_founder = None;
    let mut execute = false;
    let mut founding_only = false;
    let mut keypairs = std::collections::BTreeMap::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        // Valueless flags are matched before anything demands a value.
        if matches!(argument.as_str(), "--execute" | "--founding-only") {
            let (seen, label) = if argument == "--execute" {
                (&mut execute, "--execute")
            } else {
                (&mut founding_only, "--founding-only")
            };
            if *seen {
                return Err(Error::new(format!("{label} may be supplied only once")));
            }
            *seen = true;
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
            let path = absolute(Some(value), &format!("--keypair-{role}"))?;
            if keypairs.insert(role.to_owned(), path).is_some() {
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
            "--founding-founder" => &mut founding_founder,
            "--substituted-founder" => &mut substituted_founder,
            _ => {
                return Err(Error::new(format!("unknown campaign argument: {argument}")));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let mode = if founding_only {
        campaign::CampaignModeV1::FoundingOnly
    } else {
        campaign::CampaignModeV1::Administration
    };
    let through = match (mode, through.as_deref()) {
        (campaign::CampaignModeV1::Administration, None) => campaign::StageV1::Activation,
        (campaign::CampaignModeV1::FoundingOnly, None) => campaign::StageV1::Founding,
        (_, Some(value)) => campaign::StageV1::parse(value)?,
    };
    let market_path = market
        .map(|path| absolute(Some(path), "--market"))
        .transpose()?;
    let founding_founder = founding_founder.as_deref().map(plan::pubkey).transpose()?;
    let substituted_founder = substituted_founder
        .as_deref()
        .map(plan::pubkey)
        .transpose()?;
    match mode {
        campaign::CampaignModeV1::Administration => {
            if through > campaign::StageV1::Activation {
                return Err(Error::new(
                    "administration mode is infrastructure-only and stops at activation; pass --founding-only for a Market founding",
                ));
            }
            if market_path.is_some() || founding_founder.is_some() || substituted_founder.is_some()
            {
                return Err(Error::new(
                    "administration mode refuses --market, --founding-founder, and --substituted-founder; pass --founding-only for a Market founding",
                ));
            }
            if let Some(role) = keypairs
                .keys()
                .find(|role| role.as_str() != seed::role::CORE_UPGRADE_AUTHORITY)
            {
                return Err(Error::new(format!(
                    "administration mode refuses --keypair-{role}; its only signer path is --keypair-core-upgrade-authority"
                )));
            }
        }
        campaign::CampaignModeV1::FoundingOnly => {
            if through != campaign::StageV1::Founding {
                return Err(Error::new(
                    "--founding-only requires --through founding; it never owns an infrastructure prefix",
                ));
            }
            if market_path.is_none() {
                return Err(Error::new(
                    "--founding-only requires --market ABSOLUTE_JSON",
                ));
            }
            let founder = founding_founder
                .ok_or_else(|| Error::new("--founding-only requires --founding-founder PUBKEY"))?;
            let substituted = substituted_founder.ok_or_else(|| {
                Error::new("--founding-only requires --substituted-founder PUBKEY")
            })?;
            if founder == Pubkey::default()
                || substituted == Pubkey::default()
                || founder == substituted
            {
                return Err(Error::new(
                    "--founding-founder and --substituted-founder must be nonzero, distinct public identities",
                ));
            }
            if keypairs.contains_key(seed::role::CORE_UPGRADE_AUTHORITY) {
                return Err(Error::new(
                    "--founding-only refuses --keypair-core-upgrade-authority; infrastructure must already be Complete",
                ));
            }
            let missing = campaign::FOUNDING_REQUIRED_ROLES
                .iter()
                .filter(|role| !keypairs.contains_key(**role))
                .copied()
                .collect::<Vec<_>>();
            if !missing.is_empty() {
                return Err(Error::new(format!(
                    "--founding-only omitted required keypair paths: {}",
                    missing.join(", ")
                )));
            }
            for role in keypairs.keys() {
                if !campaign::FOUNDING_REQUIRED_ROLES.contains(&role.as_str())
                    && role != crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1
                    && role != crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1
                {
                    return Err(Error::new(format!(
                        "--founding-only refuses --keypair-{role}; it is not one of the exact founding signer paths"
                    )));
                }
            }
            let fixture_owner =
                keypairs.contains_key(crate::market::LOCAL_PARTICIPANT_FIXTURE_OWNER_ROLE_V1);
            let fixture_source =
                keypairs.contains_key(crate::market::LOCAL_PARTICIPANT_FIXTURE_SOURCE_ROLE_V1);
            if fixture_owner != fixture_source {
                return Err(Error::new(
                    "local participant fixture owner and source keypair paths must be supplied together",
                ));
            }
        }
    }
    let origin = cluster::ClusterOriginV1::parse(
        &required(rpc_url, "--rpc-url")?,
        acknowledgment.as_deref(),
    )?;
    Ok(campaign::CampaignArgsV1 {
        origin,
        mode,
        plan_path: absolute(plan, "--plan")?,
        market_path,
        evidence_path: match evidence {
            None => None,
            Some(path) => Some(absolute(Some(path), "--evidence")?),
        },
        founding_founder,
        substituted_founder,
        keypairs,
        execute,
        through,
    })
}

#[derive(Default)]
struct DirectCompilerArgumentsV1 {
    plan: Option<String>,
    rpc_url: Option<String>,
    acknowledgment: Option<String>,
    fee_basis_points: Option<String>,
    fee_recipient: Option<String>,
}

impl DirectCompilerArgumentsV1 {
    fn slot(&mut self, argument: &str) -> Option<&mut Option<String>> {
        match argument {
            "--plan" => Some(&mut self.plan),
            "--rpc-url" => Some(&mut self.rpc_url),
            campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME => Some(&mut self.acknowledgment),
            "--direct-fee-basis-points" => Some(&mut self.fee_basis_points),
            "--direct-fee-recipient" => Some(&mut self.fee_recipient),
            _ => None,
        }
    }

    fn load(self, registry: Pubkey) -> Result<direct_market::DirectMarketCompilerOwnedV1> {
        let fee_basis_points = self
            .fee_basis_points
            .map(|value| {
                value
                    .parse::<u16>()
                    .map_err(|_| Error::new("--direct-fee-basis-points must be a decimal u16"))
            })
            .transpose()?;
        let fee_recipient = self
            .fee_recipient
            .as_deref()
            .map(plan::pubkey)
            .transpose()?;
        let plan = absolute(self.plan, "--plan")?;
        let rpc_url = required(self.rpc_url, "--rpc-url")?;
        direct_market::DirectMarketCompilerOwnedV1::load_devnet(
            &plan,
            &rpc_url,
            self.acknowledgment.as_deref(),
            registry,
            fee_basis_points,
            fee_recipient,
        )
    }
}

const DEMO_MARKET_REFUSAL_V1: &str = "demo-market is a retired local-only fixture: it cannot \
authenticate the permanent devnet Direct deployment and refuses to invent Direct authority; use \
devnet-market or graduation-market with the acknowledged devnet planner";

fn direct_market_usage_v1() -> String {
    format!(
        "  dclutch-local-successor-bootstrap devnet-market --registry-program-id PUBKEY \
         --plan ABSOLUTE_JSON --rpc-url URL {ack} GENESIS_HASH \
         --direct-fee-basis-points U16 --direct-fee-recipient PUBKEY \
         --price-update ABSOLUTE_FILE --window-start UNIX_SECONDS [--window-width-seconds U32] \
         [--max-age-seconds U32] [--cut-denominator U64] [--cuts I128,..] [--coefficients U64,..] \
         [--product NAME] [--coordinate-domain NAME] [--feed LABEL] [--generation U64]\n  \
         dclutch-local-successor-bootstrap graduation-market --registry-program-id PUBKEY \
         --plan ABSOLUTE_JSON --rpc-url URL {ack} GENESIS_HASH \
         --direct-fee-basis-points U16 --direct-fee-recipient PUBKEY \
         --relayer-attestation PUBKEY --pool PUBKEY --venue-deployment-slot U64 \
         --venue-upgrade-authority PUBKEY --venue-elf-sha256 HEX64 --window-start I64 \
         --window-end I64 --max-age-seconds U32 [--venue-program PUBKEY] \
         [--venue-programdata PUBKEY]",
        ack = campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME,
    )
}

fn run_demo_market(_arguments: Vec<String>) -> Result<()> {
    Err(Error::new(DEMO_MARKET_REFUSAL_V1))
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
    let mut upgrade_set_journal = None;
    let mut deployment_set_rpc_url = None;
    let mut deployment_set_devnet_acknowledgment = None;
    let mut deployment_set_solana_cli = None;
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
            "--deployment-set-journal" => &mut upgrade_set_journal,
            "--rpc-url" => &mut deployment_set_rpc_url,
            "--i-mean-devnet" => &mut deployment_set_devnet_acknowledgment,
            "--solana-cli" => &mut deployment_set_solana_cli,
            // Optional. Absent is `genesis`, which is byte-for-byte what this
            // subcommand did before the flag existed.
            "--record-publication" => &mut record_publication,
            _ => return Err(Error::new(format!("unknown prepare argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    if upgrade_set_journal.is_some() {
        for (label, supplied) in [
            ("--registry-program-id", registry_program.is_some()),
            ("--registry-elf", registry_elf.is_some()),
            ("--registry-sha256", registry_sha256.is_some()),
            (
                "--registry-semantic-release-id",
                registry_semantic_release.is_some(),
            ),
            ("--core-program-id", core_program.is_some()),
            ("--core-elf", core_elf.is_some()),
            ("--core-sha256", core_sha256.is_some()),
            (
                "--core-semantic-release-id",
                core_semantic_release.is_some(),
            ),
            (
                "--core-bootstrap-upgrade-authority",
                core_bootstrap_upgrade_authority.is_some(),
            ),
            ("--claims-program-id", claims_program.is_some()),
            ("--claims-elf", claims_elf.is_some()),
            ("--claims-sha256", claims_sha256.is_some()),
            (
                "--claims-semantic-release-id",
                claims_semantic_release.is_some(),
            ),
            ("--trading-program-id", trading_program.is_some()),
            ("--trading-elf", trading_elf.is_some()),
            ("--trading-sha256", trading_sha256.is_some()),
            (
                "--trading-semantic-release-id",
                trading_semantic_release.is_some(),
            ),
            ("--resolution-program-id", resolution_program.is_some()),
            ("--resolution-elf", resolution_elf.is_some()),
            ("--resolution-sha256", resolution_sha256.is_some()),
            (
                "--resolution-semantic-release-id",
                resolution_semantic_release.is_some(),
            ),
            ("--custody-program-id", custody_program.is_some()),
            ("--custody-elf", custody_elf.is_some()),
            ("--custody-sha256", custody_sha256.is_some()),
            (
                "--custody-semantic-release-id",
                custody_semantic_release.is_some(),
            ),
            ("--rent-credit-program-id", rent_credit_program.is_some()),
            ("--rent-credit-elf", rent_credit_elf.is_some()),
            ("--rent-credit-sha256", rent_credit_sha256.is_some()),
            (
                "--rent-credit-semantic-release-id",
                rent_credit_semantic_release.is_some(),
            ),
        ] {
            if supplied {
                return Err(Error::new(format!(
                    "{label} is forbidden with --deployment-set-journal; checked evidence is the sole owner"
                )));
            }
        }
        for (role, flag_role, input, carried) in [
            ("registry", "registry", &deployments.registry, true),
            ("rent", "rent-credit", &deployments.rent_credit, true),
            ("custody", "custody", &deployments.custody, false),
            ("resolution", "resolution", &deployments.resolution, false),
            ("claims", "claims", &deployments.claims, false),
            ("trading", "trading", &deployments.trading, false),
            ("core", "core", &deployments.core, false),
        ] {
            if input.expected_live_elf_sha256.is_some()
                || input.expected_upgrade_authority.is_some()
                || input.genesis_deployment_slot != 0
            {
                return Err(Error::new(format!(
                    "raw {role} live hash, authority, or genesis slot is forbidden with --deployment-set-journal"
                )));
            }
            if carried && input.observed_programdata.is_some() {
                return Err(Error::new(format!(
                    "--{flag_role}-observed-programdata is forbidden for CarryForward; the authenticated snapshot is the sole owner"
                )));
            }
        }
        if record_publication
            .as_deref()
            .is_some_and(|value| value != "transaction")
        {
            return Err(Error::new(
                "--deployment-set-journal requires --record-publication transaction",
            ));
        }
    }
    let checked_upgrade_set = match upgrade_set_journal {
        Some(path) => Some(
            upgrade::authenticate_complete_deployment_set_for_prepare_live(
                &absolute(Some(path), "--deployment-set-journal")?,
                &required(deployment_set_rpc_url.take(), "--rpc-url")?,
                &required(
                    deployment_set_devnet_acknowledgment.take(),
                    "--i-mean-devnet",
                )?,
                &absolute(deployment_set_solana_cli.take(), "--solana-cli")?,
            )?,
        ),
        None => {
            if deployment_set_rpc_url.is_some()
                || deployment_set_devnet_acknowledgment.is_some()
                || deployment_set_solana_cli.is_some()
            {
                return Err(Error::new(
                    "--rpc-url, --i-mean-devnet, and --solana-cli are valid only with --deployment-set-journal",
                ));
            }
            None
        }
    };
    if let Some(set) = &checked_upgrade_set {
        for (label, supplied) in [
            ("--registry-program-id", registry_program.is_some()),
            ("--registry-elf", registry_elf.is_some()),
            ("--registry-sha256", registry_sha256.is_some()),
            (
                "--registry-semantic-release-id",
                registry_semantic_release.is_some(),
            ),
            ("--core-program-id", core_program.is_some()),
            ("--core-elf", core_elf.is_some()),
            ("--core-sha256", core_sha256.is_some()),
            (
                "--core-semantic-release-id",
                core_semantic_release.is_some(),
            ),
            (
                "--core-bootstrap-upgrade-authority",
                core_bootstrap_upgrade_authority.is_some(),
            ),
            ("--claims-program-id", claims_program.is_some()),
            ("--claims-elf", claims_elf.is_some()),
            ("--claims-sha256", claims_sha256.is_some()),
            (
                "--claims-semantic-release-id",
                claims_semantic_release.is_some(),
            ),
            ("--trading-program-id", trading_program.is_some()),
            ("--trading-elf", trading_elf.is_some()),
            ("--trading-sha256", trading_sha256.is_some()),
            (
                "--trading-semantic-release-id",
                trading_semantic_release.is_some(),
            ),
            ("--resolution-program-id", resolution_program.is_some()),
            ("--resolution-elf", resolution_elf.is_some()),
            ("--resolution-sha256", resolution_sha256.is_some()),
            (
                "--resolution-semantic-release-id",
                resolution_semantic_release.is_some(),
            ),
            ("--custody-program-id", custody_program.is_some()),
            ("--custody-elf", custody_elf.is_some()),
            ("--custody-sha256", custody_sha256.is_some()),
            (
                "--custody-semantic-release-id",
                custody_semantic_release.is_some(),
            ),
            ("--rent-credit-program-id", rent_credit_program.is_some()),
            ("--rent-credit-elf", rent_credit_elf.is_some()),
            ("--rent-credit-sha256", rent_credit_sha256.is_some()),
            (
                "--rent-credit-semantic-release-id",
                rent_credit_semantic_release.is_some(),
            ),
        ] {
            if supplied {
                return Err(Error::new(format!(
                    "{label} is forbidden with --deployment-set-journal; checked evidence is the sole owner"
                )));
            }
        }
        if record_publication
            .as_deref()
            .is_some_and(|value| value != "transaction")
        {
            return Err(Error::new(
                "--deployment-set-journal requires --record-publication transaction",
            ));
        }
        record_publication = Some("transaction".into());
        let retained = plan::pubkey(&set.retained_upgrade_authority)?;
        for (role, flag_role, input) in [
            ("registry", "registry", &mut deployments.registry),
            ("rent", "rent-credit", &mut deployments.rent_credit),
            ("custody", "custody", &mut deployments.custody),
            ("resolution", "resolution", &mut deployments.resolution),
            ("claims", "claims", &mut deployments.claims),
            ("trading", "trading", &mut deployments.trading),
            ("core", "core", &mut deployments.core),
        ] {
            if input.expected_live_elf_sha256.is_some()
                || input.expected_upgrade_authority.is_some()
                || input.genesis_deployment_slot != 0
            {
                return Err(Error::new(format!(
                    "raw {role} live hash, authority, or genesis slot is forbidden with --deployment-set-journal"
                )));
            }
            let pin = set
                .roles
                .iter()
                .find(|pin| pin.role == role)
                .expect("authenticated set contains every role");
            match pin.disposition {
                model::CheckedDeploymentDispositionV1::CarryForward => {
                    if input.observed_programdata.is_some() {
                        return Err(Error::new(format!(
                            "--{flag_role}-observed-programdata is forbidden for CarryForward; the authenticated snapshot is the sole owner"
                        )));
                    }
                    let encoded = pin.carried_programdata_base64.as_deref().ok_or_else(|| {
                        Error::new(format!(
                            "authenticated CarryForward {role} omitted ProgramData bytes"
                        ))
                    })?;
                    input.observed_programdata_bytes =
                        Some(BASE64.decode(encoded).map_err(|_| {
                            Error::new(format!(
                                "authenticated CarryForward {role} ProgramData is not base64"
                            ))
                        })?);
                }
                model::CheckedDeploymentDispositionV1::Upgrade => {
                    if input.observed_programdata.is_none() {
                        return Err(Error::new(format!(
                            "--deployment-set-journal requires --{flag_role}-observed-programdata for receipt-backed Upgrade"
                        )));
                    }
                }
            }
            input.expected_live_elf_sha256 = Some(pin.live_elf_sha256.clone());
            input.expected_upgrade_authority = Some(retained);
        }
        let role = |name: &str| {
            set.roles
                .iter()
                .find(|pin| pin.role == name)
                .expect("authenticated set contains every role")
        };
        let registry = role("registry");
        registry_program = Some(registry.program_id.clone());
        registry_elf = Some(registry.checked_candidate_elf_path.clone());
        registry_sha256 = Some(registry.checked_candidate_elf_sha256.clone());
        registry_semantic_release = Some(registry.semantic_release_id.clone());
        let core = role("core");
        core_program = Some(core.program_id.clone());
        core_elf = Some(core.checked_candidate_elf_path.clone());
        core_sha256 = Some(core.checked_candidate_elf_sha256.clone());
        core_semantic_release = Some(core.semantic_release_id.clone());
        core_bootstrap_upgrade_authority = Some(set.retained_upgrade_authority.clone());
        let claims = role("claims");
        claims_program = Some(claims.program_id.clone());
        claims_elf = Some(claims.checked_candidate_elf_path.clone());
        claims_sha256 = Some(claims.checked_candidate_elf_sha256.clone());
        claims_semantic_release = Some(claims.semantic_release_id.clone());
        let trading = role("trading");
        trading_program = Some(trading.program_id.clone());
        trading_elf = Some(trading.checked_candidate_elf_path.clone());
        trading_sha256 = Some(trading.checked_candidate_elf_sha256.clone());
        trading_semantic_release = Some(trading.semantic_release_id.clone());
        let resolution = role("resolution");
        resolution_program = Some(resolution.program_id.clone());
        resolution_elf = Some(resolution.checked_candidate_elf_path.clone());
        resolution_sha256 = Some(resolution.checked_candidate_elf_sha256.clone());
        resolution_semantic_release = Some(resolution.semantic_release_id.clone());
        let custody = role("custody");
        custody_program = Some(custody.program_id.clone());
        custody_elf = Some(custody.checked_candidate_elf_path.clone());
        custody_sha256 = Some(custody.checked_candidate_elf_sha256.clone());
        custody_semantic_release = Some(custody.semantic_release_id.clone());
        let rent = role("rent");
        rent_credit_program = Some(rent.program_id.clone());
        rent_credit_elf = Some(rent.checked_candidate_elf_path.clone());
        rent_credit_sha256 = Some(rent.checked_candidate_elf_sha256.clone());
        rent_credit_semantic_release = Some(rent.semantic_release_id.clone());
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
        checked_upgrade_set,
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
        "checked_upgrade_set_final_sha256": prepared.checked_upgrade_set
            .as_ref()
            .map(|set| set.final_set_sha256.as_str()),
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

fn campaign_usage_v1() -> String {
    format!(
        "\n  dclutch-local-successor-bootstrap campaign --rpc-url URL [{ack} GENESIS_HASH] \
         --plan ABSOLUTE_JSON [--evidence ABSOLUTE_JSON] [--through STAGE] [--execute] \
         [--keypair-core-upgrade-authority ABSOLUTE_KEYPAIR_JSON]\n\
         \n  dclutch-local-successor-bootstrap campaign --founding-only --rpc-url URL \
         [{ack} GENESIS_HASH] --plan ABSOLUTE_JSON --market ABSOLUTE_JSON \
         --keypair-campaign-payer ABSOLUTE_KEYPAIR_JSON \
         --keypair-collateral-mint ABSOLUTE_KEYPAIR_JSON \
         --keypair-collateral-wallet ABSOLUTE_KEYPAIR_JSON \
         --keypair-founding-beneficiary ABSOLUTE_KEYPAIR_JSON \
         --founding-founder PUBKEY \
         --keypair-founding-projection-witness ABSOLUTE_KEYPAIR_JSON \
         --keypair-founding-source-funder ABSOLUTE_KEYPAIR_JSON \
         --substituted-founder PUBKEY [--evidence ABSOLUTE_JSON] [--execute]\n\n\
         The campaign command is the EXTERNAL-CLUSTER driver. Default is an infrastructure-only \
         administration preflight through activation. Its only possible signer is the Core upgrade \
         authority, loaded lazily only when execution has an incomplete admitted stage. \
         --founding-only is a disjoint path: publication, profile initialization, and activation \
         must already read Complete before any key file opens. It never accepts an upgrade-authority \
         path. Its campaign payer is disposable after terminal completion; its five created signer \
         coordinates must start vacant and are not wallets to pre-fund. The founder and substituted \
         founder are public identities and never keypair files. Default is PREFLIGHT: read-only RPC \
         is enforced; --execute opts into writing.\n\nORIGIN. A loopback origin needs no ceremony. \
         Any other origin is refused unless {ack} names devnet's genesis hash in full, and the \
         cluster's own getGenesisHash is checked against it at connect. {help}\n\nSTAGES: \
         {stages}. Every stage detects its own completion by reading the chain. substrate never \
         writes. Under decision 0012 every slot, authority, Loader owner, privilege, and complete \
         live ELF mismatch is fail-closed. There is no --keypair-seed on this public driver.",
        ack = campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME,
        help = campaign::acknowledgment_help(),
        stages = campaign::StageV1::ORDER
            .iter()
            .map(|stage| stage.name())
            .collect::<Vec<_>>()
            .join(", "),
    )
}

fn usage() {
    usage_supervisor();
    println!("{}", local_mutable::usage());
    println!("{}", release_capture::usage());
    println!("{}", upgrade::usage());
    println!("{}", terminal_lifecycle::usage());
    println!("{}", terminal_lifecycle::owned_loopback_usage());
    println!("{}", terminal_sequence::usage());
    println!("{}", terminal_sequence::owned_loopback_usage());
    println!("{}", user_position_admission::usage());
    println!("{}", user_position_admission::local_usage());
    println!("{}", pyth_vaa_provisioning::usage());
    println!("{}", terminal_exterior_pyth::usage());
    println!(
        "\n  dclutch-local-successor-bootstrap \
         local-private-validator-pyth-provider-closure-v1 \
         --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON \
         --local-validator-profile ABSOLUTE_JSON \
         --finalized-capture ABSOLUTE_JSON --output ABSOLUTE_NEW_JSON\n"
    );
    println!("{}", private_activity::usage());
    println!("{}", private_lifecycle::usage());
    println!("{}", private_lifecycle::direct_payout_schedule_usage());
    println!("{}", flagship_resolution::usage());
    println!("{}", flagship_resolution::owned_loopback_usage());
    println!("{}", sponsored_push::usage());
    println!("{}", sponsored_push::owned_loopback_usage());
    println!("{}", wallet_terminal::usage());
    println!("{}", wallet_terminal_payout_exterior::usage());
    println!("{}", direct_trade::usage());
    println!("{}", direct_trade_producer::usage());
    println!("{}", campaign_usage_v1());
    println!(
        "\n{direct_market_usage}\n  dclutch-local-successor-bootstrap ledger-census \
         --rpc-url URL [{ack} GENESIS_HASH] --mint PUBKEY --payer PUBKEY --hoard PUBKEY \
         --aggregate PUBKEY --claim-unit-atoms U64 --stage NAME --output ABSOLUTE_JSON \
         [--token LABEL=PUBKEY]... [--position LABEL=PUBKEY]... [--watch LABEL=PUBKEY]... \
         [--prior ABSOLUTE_JSON] [--declared-collateral-delta I128] [--declared-hoard-delta I128]\n\
         \nThe market producers authenticate the permanent devnet deployment and take one bounded, \
         read-only finalized snapshot before printing a MarketRunInput document for the \
         campaign's --market flag. Both Direct fee flags are required and have no default: \
         devnet-market the Pyth range-protection flagship (live PriceUpdateV2 body, window width \
         refused below the measured 1,252 s cadence floor), graduation-market the relayed \
         graduation market over explicitly supplied venue facts. demo-market is a retired \
         local-only fixture and always refuses rather than inventing Direct authority. \
         ledger-census takes one \
         conservation-ledger census against a live cluster (reads only, enforced) and exits \
         nonzero on any violated law; --prior reloads a previous census so the delta laws \
         evaluate across invocations.",
        ack = campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME,
        direct_market_usage = direct_market_usage_v1(),
    );
}

fn usage_supervisor() {
    println!(
        "Usage:\n  dclutch-local-successor-bootstrap prepare --account-dir ABSOLUTE_NEW_DIR --output ABSOLUTE_NEW_JSON --deployment-set-journal ABSOLUTE_JSON --rpc-url https://api.devnet.solana.com --i-mean-devnet DEVNET_GENESIS --solana-cli ABSOLUTE_EXECUTABLE --custody-observed-programdata ABSOLUTE_BODY --resolution-observed-programdata ABSOLUTE_BODY --claims-observed-programdata ABSOLUTE_BODY --trading-observed-programdata ABSOLUTE_BODY --core-observed-programdata ABSOLUTE_BODY\n  dclutch-local-successor-bootstrap run --spec ABSOLUTE_JSON [--keypair-seed 64_LOWERCASE_HEX]\n  dclutch-local-successor-bootstrap demo-market (always refuses: retired local-only fixture)\n\nThe checked deployment-set form is the only prepare admission for the permanent devnet set. Registry and Rent are exact CarryForward rows sourced only from the authenticated one-context snapshot; their raw program, ELF, ProgramData, semantic, slot, authority, and publication flags are refused. Custody, Resolution, Claims, Trading, and Core require exact complete Upgrade receipts and hostile current ProgramData bodies. Prepare first reruns the key-free live finalized audit, then rehashes all evidence and reproduces the existing Registry/Rent ArtifactRelease records and singleton profile byte-for-byte. demo-market cannot authenticate permanent-devnet Direct facts and refuses instead of inventing them."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn founding_campaign_cli_v1() -> Vec<String> {
        let founder = Pubkey::new_from_array([0x31; 32]).to_string();
        let substituted = Pubkey::new_from_array([0x32; 32]).to_string();
        let mut arguments = vec![
            "--founding-only".to_owned(),
            "--rpc-url".to_owned(),
            "http://127.0.0.1:20890/".to_owned(),
            "--plan".to_owned(),
            "/campaign-plan-must-not-be-read.json".to_owned(),
            "--market".to_owned(),
            "/campaign-market-must-not-be-read.json".to_owned(),
            "--founding-founder".to_owned(),
            founder,
            "--substituted-founder".to_owned(),
            substituted,
        ];
        for role in campaign::FOUNDING_REQUIRED_ROLES {
            arguments.extend([
                format!("--keypair-{role}"),
                format!("/campaign-{role}-must-not-be-read.json"),
            ]);
        }
        arguments
    }

    fn remove_campaign_argument_v1(arguments: &mut Vec<String>, label: &str) {
        let index = arguments
            .iter()
            .position(|value| value == label)
            .expect("fixture argument");
        arguments.drain(index..=index + 1);
    }

    #[test]
    fn campaign_cli_modes_are_disjoint_and_default_admin_stops_at_activation() {
        let admin = parse_campaign_args_v1(vec![
            "--rpc-url".into(),
            "http://127.0.0.1:20890/".into(),
            "--plan".into(),
            "/campaign-plan-must-not-be-read.json".into(),
        ])
        .expect("key-free administration parse");
        assert_eq!(admin.mode, campaign::CampaignModeV1::Administration);
        assert_eq!(admin.through, campaign::StageV1::Activation);
        assert!(admin.keypairs.is_empty());
        assert!(admin.market_path.is_none());

        let founding = parse_campaign_args_v1(founding_campaign_cli_v1())
            .expect("exact founding-only surface");
        assert_eq!(founding.mode, campaign::CampaignModeV1::FoundingOnly);
        assert_eq!(founding.through, campaign::StageV1::Founding);
        assert_eq!(
            founding.keypairs.len(),
            campaign::FOUNDING_REQUIRED_ROLES.len()
        );
        assert_ne!(founding.founding_founder, founding.substituted_founder);
    }

    #[test]
    fn founding_only_cli_refuses_authority_alias_missing_and_legacy_secret_paths() {
        let mut missing = founding_campaign_cli_v1();
        remove_campaign_argument_v1(&mut missing, "--keypair-campaign-payer");
        assert!(
            parse_campaign_args_v1(missing)
                .expect_err("missing payer path")
                .0
                .contains("campaign-payer")
        );

        let mut authority = founding_campaign_cli_v1();
        authority.extend([
            "--keypair-core-upgrade-authority".into(),
            "/upgrade-authority-must-not-be-read.json".into(),
        ]);
        assert!(
            parse_campaign_args_v1(authority)
                .expect_err("upgrade authority path")
                .0
                .contains("refuses --keypair-core-upgrade-authority")
        );

        for legacy in [
            "--keypair-founding-founder",
            "--keypair-substituted-founder",
        ] {
            let mut arguments = founding_campaign_cli_v1();
            arguments.extend([legacy.into(), "/legacy-secret-must-not-be-read.json".into()]);
            let refusal = parse_campaign_args_v1(arguments).expect_err("legacy secret path");
            assert!(refusal.0.contains("names no campaign role"), "{refusal:?}");
        }

        let mut alias = founding_campaign_cli_v1();
        let founder = alias
            .iter()
            .position(|value| value == "--founding-founder")
            .and_then(|index| alias.get(index + 1))
            .cloned()
            .expect("founder value");
        let substituted = alias
            .iter()
            .position(|value| value == "--substituted-founder")
            .expect("substituted flag");
        alias[substituted + 1] = founder;
        assert!(
            parse_campaign_args_v1(alias)
                .expect_err("actor alias")
                .0
                .contains("distinct public identities")
        );

        let mut prefix = founding_campaign_cli_v1();
        prefix.extend(["--through".into(), "activation".into()]);
        assert!(
            parse_campaign_args_v1(prefix)
                .expect_err("founding prefix")
                .0
                .contains("requires --through founding")
        );
    }

    #[test]
    fn campaign_help_names_exact_eight_identity_founding_manifest() {
        let help = campaign_usage_v1();
        for required in [
            "--founding-only",
            "--keypair-campaign-payer",
            "--keypair-collateral-mint",
            "--keypair-collateral-wallet",
            "--keypair-founding-beneficiary",
            "--founding-founder PUBKEY",
            "--keypair-founding-projection-witness",
            "--keypair-founding-source-funder",
            "--substituted-founder PUBKEY",
        ] {
            assert!(help.contains(required), "help omitted {required}");
        }
        assert!(!help.contains("--keypair-founding-founder"));
        assert!(!help.contains("--keypair-substituted-founder"));
        assert!(help.contains("never accepts an upgrade-authority path"));
    }

    fn checked(extra: &[&str]) -> Vec<String> {
        let mut arguments = vec![
            "--deployment-set-journal".to_owned(),
            "/this-file-must-not-be-read.json".to_owned(),
        ];
        arguments.extend(extra.iter().map(|value| (*value).to_owned()));
        arguments
    }

    #[test]
    fn checked_prepare_cli_refuses_raw_infrastructure_before_evidence_or_rpc() {
        for hostile in [
            vec!["--registry-program-id", "11111111111111111111111111111111"],
            vec!["--registry-observed-programdata", "/tmp/substituted.bin"],
            vec!["--rent-credit-live-elf-sha256", "11"],
            vec![
                "--core-expected-upgrade-authority",
                "11111111111111111111111111111111",
            ],
            vec!["--record-publication", "genesis"],
        ] {
            let refusal = run_prepare(checked(&hostile)).expect_err("raw checked input refuses");
            assert!(
                refusal.0.contains("forbidden") || refusal.0.contains("requires"),
                "{}",
                refusal.0
            );
            assert!(
                !refusal.0.contains("this-file-must-not-be-read"),
                "the hostile flag must refuse before evidence I/O: {}",
                refusal.0
            );
        }
    }

    #[test]
    fn checked_prepare_cli_has_no_receipt_or_disposition_override_flags() {
        for hostile in [
            vec!["--registry-receipt", "/tmp/fake.json"],
            vec!["--registry-disposition", "upgrade"],
            vec!["--rent-credit-disposition", "upgrade"],
        ] {
            let refusal = run_prepare(checked(&hostile)).expect_err("unknown authority flag");
            assert!(
                refusal.0.contains("unknown prepare argument"),
                "{}",
                refusal.0
            );
        }
    }

    #[test]
    fn usage_names_both_key_free_release_capture_commands() {
        let usage = release_capture::usage();
        assert!(usage.contains("devnet-carry-forward-capture-v1"));
        assert!(usage.contains("devnet-prepare-programdata-capture-v1"));
        assert!(usage.contains("devnet-permanent-substrate-capture-v1"));
        assert!(usage.contains("read-only and key-free"));
    }

    #[test]
    fn public_terminal_surface_has_one_canonical_six_stage_owner() {
        assert_eq!(PUBLIC_TERMINAL_COMMANDS_V1, ["devnet-terminal-sequence-v1"]);
        let canonical = terminal_sequence::usage();
        assert!(canonical.contains(PUBLIC_TERMINAL_COMMANDS_V1[0]));
        assert!(canonical.contains("unsigned durable next action before any key"));
        assert!(canonical.contains("persists the signed packet"));
        assert!(!terminal_lifecycle::usage().contains("terminal-lifecycle-plan"));
    }

    #[test]
    fn owned_loopback_resolution_and_terminal_commands_are_visible_and_disjoint() {
        assert_eq!(
            OWNED_LOOPBACK_TERMINAL_COMMANDS_V1,
            [
                "local-private-validator-flagship-resolution-v1",
                "local-private-validator-terminal-sequence-v1",
            ]
        );
        let resolution = flagship_resolution::owned_loopback_usage();
        let terminal = terminal_sequence::owned_loopback_usage();
        assert!(resolution.contains(OWNED_LOOPBACK_TERMINAL_COMMANDS_V1[0]));
        assert!(terminal.contains(OWNED_LOOPBACK_TERMINAL_COMMANDS_V1[1]));
        assert!(!resolution.contains("--i-mean-devnet"));
        assert!(!terminal.contains("--i-mean-devnet"));
        assert!(resolution.contains("refuses every external origin"));
        assert!(terminal.contains("refuses every external origin"));
        assert!(!flagship_resolution::usage().contains(OWNED_LOOPBACK_TERMINAL_COMMANDS_V1[0]));
        assert!(!terminal_sequence::usage().contains(OWNED_LOOPBACK_TERMINAL_COMMANDS_V1[1]));
    }

    #[test]
    fn owned_loopback_wallet_payout_input_is_visible_and_disjoint() {
        let local = terminal_lifecycle::owned_loopback_usage();
        assert!(
            local.contains(terminal_lifecycle::OWNED_LOOPBACK_WALLET_TERMINAL_INPUT_COMMAND_V1)
        );
        assert!(local.contains("refuses devnet, mainnet-beta"));
        assert!(!local.contains("--i-mean-devnet"));
        assert!(
            !terminal_lifecycle::usage()
                .contains(terminal_lifecycle::OWNED_LOOPBACK_WALLET_TERMINAL_INPUT_COMMAND_V1)
        );
    }

    #[test]
    fn direct_market_cli_surface_has_one_devnet_authority_path() {
        let mut arguments = DirectCompilerArgumentsV1::default();
        for flag in [
            "--plan",
            "--rpc-url",
            campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME,
            "--direct-fee-basis-points",
            "--direct-fee-recipient",
        ] {
            assert!(arguments.slot(flag).is_some(), "missing {flag}");
        }
        for retired in [
            "--direct-execution-config",
            "--direct-activation-deadline-slot",
            "--direct-root-rent-minimum-lamports",
        ] {
            assert!(arguments.slot(retired).is_none(), "admitted {retired}");
        }
    }

    #[test]
    fn market_commands_refuse_retired_and_duplicate_authority_flags_during_parse() {
        let devnet_retired = run_devnet_market(vec![
            "--direct-execution-config".to_owned(),
            "/this/file-must-not-be-read".to_owned(),
        ])
        .expect_err("retired devnet-market authority must be unknown");
        assert!(
            devnet_retired.0.contains("unknown devnet-market argument"),
            "{}",
            devnet_retired.0
        );

        let graduation_retired = run_graduation_market(vec![
            "--direct-activation-deadline-slot".to_owned(),
            "1".to_owned(),
        ])
        .expect_err("retired graduation-market authority must be unknown");
        assert!(
            graduation_retired
                .0
                .contains("unknown graduation-market argument"),
            "{}",
            graduation_retired.0
        );

        let duplicate = run_devnet_market(vec![
            "--rpc-url".to_owned(),
            "https://api.devnet.solana.com".to_owned(),
            "--rpc-url".to_owned(),
            "https://example.invalid".to_owned(),
        ])
        .expect_err("duplicate authority coordinate must refuse during parse");
        assert_eq!(duplicate.0, "--rpc-url may be supplied only once");
    }

    #[test]
    fn direct_market_help_names_exact_surface_and_no_retired_scalar_or_file() {
        let usage = direct_market_usage_v1();
        for required in [
            "--plan ABSOLUTE_JSON",
            "--rpc-url URL",
            "--i-mean-devnet GENESIS_HASH",
            "--direct-fee-basis-points U16",
            "--direct-fee-recipient PUBKEY",
        ] {
            assert!(usage.contains(required), "help omitted {required}");
        }
        for retired in [
            "--direct-execution-config",
            "--direct-activation-deadline-slot",
            "--direct-root-rent-minimum-lamports",
        ] {
            assert!(!usage.contains(retired), "help retained {retired}");
        }
    }

    #[test]
    fn retired_local_demo_refuses_before_parsing_or_reading_arguments() {
        let refusal = run_demo_market(vec![
            "--plan".to_owned(),
            "/this/demo-plan-must-not-be-read.json".to_owned(),
        ])
        .expect_err("retired local demo must refuse");
        assert_eq!(refusal.0, DEMO_MARKET_REFUSAL_V1);
    }

    #[test]
    fn direct_market_cli_refuses_loopback_before_plan_or_rpc_access() {
        let refusal = DirectCompilerArgumentsV1 {
            plan: Some("/this/direct-plan-must-not-be-read.json".to_owned()),
            rpc_url: Some("http://127.0.0.1:8899".to_owned()),
            acknowledgment: None,
            fee_basis_points: Some("0".to_owned()),
            fee_recipient: Some(Pubkey::new_unique().to_string()),
        }
        .load(Pubkey::new_unique())
        .err()
        .expect("production Direct planner must refuse loopback");
        assert!(refusal.0.contains("devnet-only"), "{}", refusal.0);
    }

    #[test]
    fn direct_market_cli_refuses_missing_acknowledgment_before_plan_or_rpc_access() {
        let refusal = DirectCompilerArgumentsV1 {
            plan: Some("/this/direct-plan-must-not-be-read.json".to_owned()),
            rpc_url: Some("https://api.devnet.solana.com".to_owned()),
            acknowledgment: None,
            fee_basis_points: Some("0".to_owned()),
            fee_recipient: Some(Pubkey::new_unique().to_string()),
        }
        .load(Pubkey::new_unique())
        .err()
        .expect("external origin without devnet acknowledgment must refuse");
        assert!(
            refusal
                .0
                .contains(campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME),
            "{}",
            refusal.0
        );
    }

    #[test]
    fn direct_market_cli_refuses_invalid_fee_before_plan_or_rpc_access() {
        let refusal = DirectCompilerArgumentsV1 {
            plan: Some("/this/direct-plan-must-not-be-read.json".to_owned()),
            rpc_url: Some("https://api.devnet.solana.com".to_owned()),
            acknowledgment: Some(cluster::DEVNET_GENESIS_HASH.to_owned()),
            fee_basis_points: Some("not-a-u16".to_owned()),
            fee_recipient: Some(Pubkey::new_unique().to_string()),
        }
        .load(Pubkey::new_unique())
        .err()
        .expect("malformed fee must refuse before evidence or RPC");
        assert_eq!(refusal.0, "--direct-fee-basis-points must be a decimal u16");
    }
}
