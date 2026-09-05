#![forbid(unsafe_code)]

use std::{collections::BTreeMap, env, error::Error as StdError, fmt, io::Write, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use solana_sdk::pubkey::Pubkey;

mod aggregate_retirement_exterior;
mod aggregate_retirement_journal;
mod campaign;
mod capability_seal_close;
mod capability_seal_devnet;
mod chaos_fault;
mod claims_custody_replay;
mod closure_receipt_projection;
mod cluster;
mod collateral_release;
mod core_bump_projection;
mod direct_capability_activation;
mod direct_close_maker;
mod direct_fee_settlement;
mod direct_hot_route_manifest;
mod direct_market;
mod direct_resolution_campaign;
mod direct_terminal_children;
mod direct_ticket;
mod direct_trade;
mod direct_trade_producer;
mod direct_trade_setup;
mod direct_trade_setup_journal;
mod direct_trade_token_setup;
mod evidence_refresh;
mod family_hot_campaign;
mod flagship_resolution;
mod fractional_market;
mod funding_readiness;
mod general_capability_activation;
mod general_devnet_market;
mod general_market;
mod general_session;
mod general_settlement_fixture;
mod general_successor_plan;
mod infrastructure_succession;
mod local_mutable;
mod release_lineage;
mod series_consume_campaign;
mod series_lifecycle_campaign;
mod series_permit_expiry_campaign;
mod series_terminal_campaign;
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
mod rational_market;
mod recovery_crank;
mod relayed;
mod release_capture;
mod release_identity;
mod rpc;
mod runtime;
mod seed;
mod selected_capability;
mod source_abort_exterior;
mod spline_product;
mod sponsored_push;
mod sponsored_release_observation;
mod sponsored_schedule;
mod structured_market;
mod terminal_exterior_pyth;
mod terminal_lifecycle;
mod terminal_sequence;
mod upgrade;
mod user_position_admission;
mod user_position_close;
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

/// The extracted payout derivation's refusals, carried through unchanged.
impl From<dclutch_operator::wallet_terminal_payout::Error> for Error {
    fn from(error: dclutch_operator::wallet_terminal_payout::Error) -> Self {
        Self(error.to_string())
    }
}

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
        Some("devnet-sponsored-market") => run_devnet_sponsored_market(arguments.collect()),
        Some("devnet-general-market") => run_devnet_general_market(arguments.collect()),
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
        Some(command) if command == upgrade::ALREADY_CURRENT_COMMAND_V1 => {
            upgrade::run_already_current(arguments.collect())
        }
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
        Some(command) if command == user_position_close::COMMAND_V1 => {
            user_position_close::run(arguments.collect())
        }
        Some(command) if command == user_position_close::COMMAND_DEVNET_V1 => {
            user_position_close::run_devnet_v1(arguments.collect())
        }
        Some(command) if command == direct_terminal_children::COMMAND_V1 => {
            direct_terminal_children::run(arguments.collect())
        }
        Some(command) if command == direct_terminal_children::COMMAND_DEVNET_V1 => {
            direct_terminal_children::run_devnet_v1(arguments.collect())
        }
        Some("local-private-validator-wallet-terminal-payout-v1") => {
            wallet_terminal_payout_exterior::run(arguments.collect())
        }
        Some(command) if command == wallet_terminal_payout_exterior::COMMAND_DEVNET_V1 => {
            wallet_terminal_payout_exterior::run_devnet_v1(arguments.collect())
        }
        Some(command) if command == aggregate_retirement_exterior::COMMAND_V1 => {
            aggregate_retirement_exterior::run_owned_loopback(arguments.collect())
        }
        Some(command) if command == aggregate_retirement_exterior::COMMAND_DEVNET_V1 => {
            aggregate_retirement_exterior::run_devnet(arguments.collect())
        }
        Some(command) if command == evidence_refresh::REFRESH_EVIDENCE_COMMAND_V1 => {
            evidence_refresh::run_devnet(arguments.collect())
        }
        Some(command) if command == evidence_refresh::LOCAL_REFRESH_EVIDENCE_COMMAND_V1 => {
            evidence_refresh::run_owned_loopback(arguments.collect())
        }
        Some("devnet-direct-trade-v1") => direct_trade::run_devnet(arguments.collect()),
        Some(general_session::DEVNET_GENERAL_SESSION_COMMAND_V1) => {
            general_session::run_devnet(arguments.collect())
        }
        Some(command) if command == family_hot_campaign::GENERAL_COMMAND_V1 => {
            family_hot_campaign::run(arguments.collect(), family_hot_campaign::FamilyV1::General)
        }
        Some(command) if command == series_consume_campaign::SERIES_CONSUME_COMMAND_V1 => {
            series_consume_campaign::run(arguments.collect())
        }
        Some(command)
            if command == series_lifecycle_campaign::SERIES_LIFECYCLE_PREFIX_COMMAND_V1 =>
        {
            series_lifecycle_campaign::run(arguments.collect())
        }
        Some(command) if command == series_permit_expiry_campaign::COMMAND_V1 => {
            series_permit_expiry_campaign::run(arguments.collect())
        }
        Some(command)
            if command == series_terminal_campaign::SERIES_TERMINAL_CAMPAIGN_COMMAND_V1 =>
        {
            series_terminal_campaign::run(arguments.collect())
        }
        Some(command) if command == family_hot_campaign::SERIES_COMMAND_V1 => {
            family_hot_campaign::run(arguments.collect(), family_hot_campaign::FamilyV1::Series)
        }
        Some(command) if command == recovery_crank::COMMAND_V1 => {
            recovery_crank::run_owned_loopback_v1(arguments.collect())
        }
        Some(command) if command == recovery_crank::COMMAND_DEVNET_V1 => {
            recovery_crank::run_devnet_v1(arguments.collect())
        }
        Some(command) if command == claims_custody_replay::COMMAND_V1 => {
            claims_custody_replay::run_owned_loopback_v1(arguments.collect())
        }
        Some(command) if command == claims_custody_replay::COMMAND_DEVNET_V1 => {
            claims_custody_replay::run_devnet_v1(arguments.collect())
        }
        Some(command) if command == capability_seal_close::COMMAND_V1 => {
            capability_seal_close::run_owned_loopback_v1(arguments.collect())
        }
        Some(command) if command == capability_seal_close::COMMAND_DEVNET_V1 => {
            capability_seal_close::run_devnet_v1(arguments.collect())
        }
        Some(command) if command == capability_seal_devnet::COMMAND_DEVNET_V1 => {
            capability_seal_devnet::run_devnet(arguments.collect())
        }
        Some(command) if command == direct_close_maker::COMMAND_V1 => {
            direct_close_maker::run_owned_loopback_v1(arguments.collect())
        }
        Some(command) if command == direct_close_maker::COMMAND_DEVNET_V1 => {
            direct_close_maker::run_devnet_v1(arguments.collect())
        }
        Some(command) if command == direct_fee_settlement::COMMAND_V1 => {
            direct_fee_settlement::run_owned_loopback_v1(arguments.collect())
        }
        Some(command) if command == direct_fee_settlement::COMMAND_DEVNET_V1 => {
            direct_fee_settlement::run_devnet_v1(arguments.collect())
        }
        Some(command) if command == infrastructure_succession::COMMAND_V1 => {
            infrastructure_succession::run_owned_loopback_v1(arguments.collect())
        }
        Some(command) if command == infrastructure_succession::COMMAND_DEVNET_V1 => {
            infrastructure_succession::run_devnet_v1(arguments.collect())
        }
        Some(command) if command == release_lineage::COMMAND_V1 => {
            release_lineage::run_owned_loopback_v1(arguments.collect())
        }
        Some(command) if command == release_lineage::COMMAND_DEVNET_V1 => {
            release_lineage::run_devnet_v1(arguments.collect())
        }
        Some("local-private-validator-direct-trade-v1") => {
            direct_trade::run_owned_loopback(arguments.collect())
        }
        Some("local-private-validator-direct-trade-produce-v1") => {
            direct_trade_producer::run_owned_loopback(arguments.collect())
        }
        Some(command) if command == direct_trade_producer::DEVNET_SESSION_PRODUCER_COMMAND_V1 => {
            direct_trade_producer::run_devnet_session(arguments.collect())
        }
        Some(command) if command == direct_trade_producer::DEVNET_DIRECT_PRODUCER_COMMAND_V1 => {
            direct_trade_producer::run_devnet_direct(arguments.collect())
        }
        Some(command) if command == direct_ticket::DIRECT_TICKET_AUTHOR_COMMAND_V1 => {
            direct_ticket::run(arguments.collect())
        }
        Some(command)
            if command == direct_hot_route_manifest::CHECKED_EXECUTION_RELEASE_COMMAND_V1 =>
        {
            direct_hot_route_manifest::run_checked_execution_release(arguments.collect())
        }
        Some(command) if command == direct_hot_route_manifest::HOT_ROUTE_MANIFEST_COMMAND_V3 => {
            direct_hot_route_manifest::run_hot_route_manifest(arguments.collect())
        }
        Some(command)
            if command == direct_capability_activation::DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1 =>
        {
            direct_capability_activation::run_devnet(arguments.collect())
        }
        Some(command)
            if command
                == direct_capability_activation::LOCAL_DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1 =>
        {
            direct_capability_activation::run_owned_loopback(arguments.collect())
        }
        Some(command)
            if command
                == general_capability_activation::GENERAL_CAPABILITY_ACTIVATION_COMMAND_V1 =>
        {
            general_capability_activation::run_owned_loopback(arguments.collect())
        }
        Some(command)
            if command
                == general_capability_activation::DEVNET_GENERAL_CAPABILITY_ACTIVATION_COMMAND_V1 =>
        {
            general_capability_activation::run_devnet(arguments.collect())
        }
        Some(command) if command == general_successor_plan::DEVNET_EXECUTE_COMMAND_V1 => {
            general_successor_plan::run_execute_devnet(arguments.collect())
        }
        Some(command) if command == general_successor_plan::DEVNET_LOOKUP_TABLE_COMMAND_V1 => {
            general_successor_plan::run_lookup_table_devnet(arguments.collect())
        }
        Some(command) if command == general_successor_plan::COMMAND_V1 => {
            general_successor_plan::run(arguments.collect())
        }
        Some("flagship-resolution-v1") => flagship_resolution::run(arguments.collect()),
        Some("devnet-sponsored-push-v1") => sponsored_push::run_devnet(arguments.collect()),
        Some(command) if command == sponsored_schedule::COMMAND_V1 => {
            sponsored_schedule::run(arguments.collect())
        }
        Some(command) if command == sponsored_push::INPUT_COMMAND_DEVNET_V1 => {
            sponsored_push::run_devnet_input(arguments.collect())
        }
        Some(command) if command == sponsored_push::INPUT_COMMAND_LOOPBACK_V1 => {
            sponsored_push::run_owned_loopback_input(arguments.collect())
        }
        Some(command) if command == source_abort_exterior::COMMAND_V1 => {
            source_abort_exterior::run(arguments.collect())
        }
        Some(command) if command == source_abort_exterior::INTERRUPTION_AUDIT_COMMAND_V1 => {
            source_abort_exterior::run_interruption_audit(arguments.collect())
        }
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
        Some(command) if command == direct_resolution_campaign::COMMAND_V1 => {
            direct_resolution_campaign::run_owned_loopback_v1(arguments.collect())
        }
        Some(command) if command == spline_product::COMMAND_V1 => {
            spline_product::run(arguments.collect())
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
    let mut infrastructure_lineage = None;
    let mut through = None;
    let mut founding_founder = None;
    let mut substituted_founder = None;
    let mut execute = false;
    let mut founding_only = false;
    let mut recover_finalized_founding = false;
    let mut keypairs = std::collections::BTreeMap::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        // Valueless flags are matched before anything demands a value.
        if matches!(
            argument.as_str(),
            "--execute" | "--founding-only" | "--recover-finalized-founding"
        ) {
            let (seen, label) = match argument.as_str() {
                "--execute" => (&mut execute, "--execute"),
                "--founding-only" => (&mut founding_only, "--founding-only"),
                _ => (
                    &mut recover_finalized_founding,
                    "--recover-finalized-founding",
                ),
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
            "--infrastructure-lineage-evidence" => &mut infrastructure_lineage,
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
    // The repair is a founding act and a writing one. Refusing the two other
    // shapes here keeps the flag from ever being a decoration on a read.
    if recover_finalized_founding && (mode != campaign::CampaignModeV1::FoundingOnly || !execute) {
        return Err(Error::new(
            "--recover-finalized-founding is a founding repair: it requires --founding-only and --execute",
        ));
    }
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
            campaign::authenticate_administration_through_v1(through).map_err(|_| {
                Error::new(
                    "administration mode is infrastructure-only and stops at activation; pass --founding-only for a Market founding",
                )
            })?;
            if market_path.is_some() || founding_founder.is_some() || substituted_founder.is_some()
            {
                return Err(Error::new(
                    "administration mode refuses --market, --founding-founder, and --substituted-founder; pass --founding-only for a Market founding",
                ));
            }
            if infrastructure_lineage.is_some()
                && (!execute || through != campaign::StageV1::Activation)
            {
                return Err(Error::new(
                    "--infrastructure-lineage-evidence requires executed administration through activation; a prefix or read-only projection is not complete lineage evidence",
                ));
            }
            if let Some(role) = keypairs.keys().find(|role| {
                ![
                    seed::role::CORE_UPGRADE_AUTHORITY,
                    seed::role::CAMPAIGN_PAYER,
                ]
                .contains(&role.as_str())
            }) {
                return Err(Error::new(format!(
                    "administration mode refuses --keypair-{role}; its signer paths are the Core authority and, only for succession, a distinct campaign payer"
                )));
            }
        }
        campaign::CampaignModeV1::FoundingOnly => {
            if infrastructure_lineage.is_some() {
                return Err(Error::new(
                    "--founding-only refuses --infrastructure-lineage-evidence; the administration campaign is the sole owner of the infrastructure lineage artifact",
                ));
            }
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
        infrastructure_lineage_path: match infrastructure_lineage {
            None => None,
            Some(path) => Some(absolute(Some(path), "--infrastructure-lineage-evidence")?),
        },
        founding_founder,
        substituted_founder,
        keypairs,
        execute,
        recover_finalized_founding,
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

/// Everything `devnet-general-market` takes that `devnet-sponsored-market`
/// does not, plus the three flags both need.
///
/// It is a sibling of `DirectCompilerArgumentsV1` rather than an extension of
/// it, because the two families do not share one fact: a General market has no
/// Direct fee policy, and a Direct market has no accelerator. Sharing the
/// struct would make each family's flags silently admissible on the other.
#[derive(Default)]
struct GeneralCompilerArgumentsV1 {
    plan: Option<String>,
    rpc_url: Option<String>,
    acknowledgment: Option<String>,
    accelerator_program: Option<String>,
    accelerator_elf: Option<String>,
    accelerator_semantic_release_id: Option<String>,
    accelerator_upgrade_authority: Option<String>,
    policy: Option<String>,
    compiler_release: Option<String>,
    toolchain: Option<String>,
    translation_validation: Option<String>,
    selection_policy: Option<String>,
    quote_surplus_beneficiary: Option<String>,
}

impl GeneralCompilerArgumentsV1 {
    fn slot(&mut self, argument: &str) -> Option<&mut Option<String>> {
        match argument {
            "--plan" => Some(&mut self.plan),
            "--rpc-url" => Some(&mut self.rpc_url),
            campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME => Some(&mut self.acknowledgment),
            "--general-accelerator-program-id" => Some(&mut self.accelerator_program),
            "--general-accelerator-elf" => Some(&mut self.accelerator_elf),
            "--general-accelerator-semantic-release-id" => {
                Some(&mut self.accelerator_semantic_release_id)
            }
            "--general-accelerator-upgrade-authority" => {
                Some(&mut self.accelerator_upgrade_authority)
            }
            "--general-policy" => Some(&mut self.policy),
            "--general-compiler-release" => Some(&mut self.compiler_release),
            "--general-toolchain" => Some(&mut self.toolchain),
            "--general-translation-validation" => Some(&mut self.translation_validation),
            "--general-selection-policy" => Some(&mut self.selection_policy),
            "--general-quote-surplus-beneficiary" => Some(&mut self.quote_surplus_beneficiary),
            _ => None,
        }
    }

    /// The accelerator's upgrade authority is REQUIRED, and the reason is that
    /// its absence is a claim.
    ///
    /// `plan::release_facts` derives `Immutable` from an absent authority, so
    /// a caller who simply forgot the flag would mint a release asserting the
    /// accelerator can never be redeployed — and the market's certificates
    /// would then pin an artifact whose supersession nothing watches for. The
    /// operator states `immutable` in words or names the key.
    fn expected_upgrade_authority(&self) -> Result<Option<Pubkey>> {
        match self.accelerator_upgrade_authority.as_deref() {
            None => Err(Error::new(
                "--general-accelerator-upgrade-authority is required: pass the key the deployment \
                 must be upgradeable under, or the literal `immutable` to assert it carries none. \
                 An omitted flag would mint an Immutable release for a mutable program",
            )),
            Some("immutable") => Ok(None),
            Some(value) => Ok(Some(plan::pubkey(value)?)),
        }
    }

    fn load(
        self,
    ) -> Result<(
        PathBuf,
        String,
        Option<String>,
        general_devnet_market::GeneralDevnetCompilerArgumentsV1,
    )> {
        let expected_upgrade_authority = self.expected_upgrade_authority()?;
        let plan = absolute(self.plan, "--plan")?;
        let rpc_url = required(self.rpc_url, "--rpc-url")?;
        let acknowledgment = self.acknowledgment;
        let arguments = general_devnet_market::GeneralDevnetCompilerArgumentsV1 {
            accelerator: general_devnet_market::GeneralDevnetAcceleratorArgumentsV1 {
                program: parse_pubkey(
                    self.accelerator_program,
                    "--general-accelerator-program-id",
                )?,
                built_elf: absolute(self.accelerator_elf, "--general-accelerator-elf")?,
                semantic_release_id: hex32(
                    self.accelerator_semantic_release_id,
                    "--general-accelerator-semantic-release-id",
                )?,
                expected_upgrade_authority,
            },
            evidence: general_devnet_market::GeneralDevnetEvidenceArgumentsV1 {
                compiler_release: absolute(self.compiler_release, "--general-compiler-release")?,
                toolchain: absolute(self.toolchain, "--general-toolchain")?,
                translation_validation: absolute(
                    self.translation_validation,
                    "--general-translation-validation",
                )?,
                selection_policy: absolute(self.selection_policy, "--general-selection-policy")?,
            },
            policy: absolute(self.policy, "--general-policy")?,
            quote_surplus_beneficiary: parse_pubkey(
                self.quote_surplus_beneficiary,
                "--general-quote-surplus-beneficiary",
            )?,
        };
        Ok((plan, rpc_url, acknowledgment, arguments))
    }
}

/// Which family a devnet Pyth range-protection market selects.
///
/// The graph is identical in all three; the only difference is which closure
/// the selection seam attaches to it.
#[derive(Clone, Copy, Eq, PartialEq)]
enum DevnetMarketFamilyV1 {
    /// Pull-oracle Pyth, Direct-selected.
    Direct,
    /// Sponsored-push Pyth, Direct-selected.
    SponsoredDirect,
    /// Sponsored-push Pyth, General-selected.
    SponsoredGeneral,
}

impl DevnetMarketFamilyV1 {
    const fn command(self) -> &'static str {
        match self {
            Self::Direct => "devnet-market",
            Self::SponsoredDirect => "devnet-sponsored-market",
            Self::SponsoredGeneral => "devnet-general-market",
        }
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
         dclutch-local-successor-bootstrap devnet-sponsored-market --registry-program-id PUBKEY \
         --plan ABSOLUTE_JSON --rpc-url URL {ack} GENESIS_HASH \
         --direct-fee-basis-points U16 --direct-fee-recipient PUBKEY \
         --price-update ABSOLUTE_FILE --window-start UNIX_SECONDS [--window-width-seconds U32] \
         [--max-age-seconds U32] [--cut-denominator U64] [--cuts I128,..] [--coefficients U64,..] \
         [--product NAME] [--coordinate-domain NAME] [--feed LABEL] [--generation U64]\n  \
         dclutch-local-successor-bootstrap devnet-general-market --registry-program-id PUBKEY \
         --plan ABSOLUTE_JSON --rpc-url URL {ack} GENESIS_HASH \
         --general-accelerator-program-id PUBKEY --general-accelerator-elf ABSOLUTE_FILE \
         --general-accelerator-semantic-release-id HEX64 \
         --general-accelerator-upgrade-authority PUBKEY|immutable \
         --general-policy ABSOLUTE_JSON --general-selection-policy ABSOLUTE_FILE \
         --general-compiler-release ABSOLUTE_FILE --general-toolchain ABSOLUTE_FILE \
         --general-translation-validation ABSOLUTE_FILE \
         --general-quote-surplus-beneficiary PUBKEY \
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
    run_devnet_pyth_market(arguments, DevnetMarketFamilyV1::Direct)
}

fn run_devnet_sponsored_market(arguments: Vec<String>) -> Result<()> {
    run_devnet_pyth_market(arguments, DevnetMarketFamilyV1::SponsoredDirect)
}

/// The devnet flagship's market graph with GENERAL selected instead of Direct.
///
/// The same document `devnet-sponsored-market` emits, compiled through the
/// same capability-neutral seam, differing in exactly one thing: the closure
/// attached to it is General's, and its four deployment identities are read
/// off a real accelerator deployment rather than projected off the plan.
fn run_devnet_general_market(arguments: Vec<String>) -> Result<()> {
    run_devnet_pyth_market(arguments, DevnetMarketFamilyV1::SponsoredGeneral)
}

fn run_devnet_pyth_market(arguments: Vec<String>, family: DevnetMarketFamilyV1) -> Result<()> {
    let mut registry = None;
    let mut price_update = None;
    let mut window_start = None;
    let mut window_width = None;
    let mut max_age = None;
    let mut cut_denominator = None;
    let mut cuts = None;
    let mut band_anchor = None;
    let mut band_volatility = None;
    let mut band_window_slots = None;
    let mut band_half_widths = None;
    let mut band_max_cell_share = None;
    let mut coefficients = None;
    let mut product_name = None;
    let mut coordinate_domain_name = None;
    let mut feed_label = None;
    let mut generation = None;
    let mut recovery_rungs = None;
    let mut direct = DirectCompilerArgumentsV1::default();
    let mut general = GeneralCompilerArgumentsV1::default();
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
            "--band-anchor" => Some(&mut band_anchor),
            "--band-volatility-bps" => Some(&mut band_volatility),
            "--band-window-slots" => Some(&mut band_window_slots),
            "--band-plausible-half-widths" => Some(&mut band_half_widths),
            "--band-max-cell-share-bps" => Some(&mut band_max_cell_share),
            "--coefficients" => Some(&mut coefficients),
            "--product" => Some(&mut product_name),
            "--coordinate-domain" => Some(&mut coordinate_domain_name),
            "--feed" => Some(&mut feed_label),
            "--generation" => Some(&mut generation),
            "--recovery-rungs" => Some(&mut recovery_rungs),
            // A family's flags are admissible only on that family. A General
            // market has no Direct fee policy and a Direct market has no
            // accelerator; letting either through would accept a stated belief
            // and then compile something that never read it.
            _ => match family {
                DevnetMarketFamilyV1::SponsoredGeneral => general.slot(&argument),
                DevnetMarketFamilyV1::Direct | DevnetMarketFamilyV1::SponsoredDirect => {
                    direct.slot(&argument)
                }
            },
        }
        .ok_or_else(|| Error::new(format!("unknown {} argument: {argument}", family.command())))?;
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
    // All five or none: a partial band refuses naming what was left out. The
    // devnet flagship's cuts default to 12000,18000, which is exactly the
    // partition that resolves into one cell every time -- so this market is
    // foundable only once its author says what they believe.
    let founding_band = match (
        &band_anchor,
        &band_volatility,
        &band_window_slots,
        &band_half_widths,
        &band_max_cell_share,
    ) {
        (None, None, None, None, None) => None,
        (Some(a), Some(v), Some(w), Some(h), Some(c)) => {
            Some(crate::model::FoundingBandInputV1::spot_band(
                decimal::<i128>(Some(a.clone()), "--band-anchor")?,
                decimal::<u32>(Some(v.clone()), "--band-volatility-bps")?,
                decimal::<u64>(Some(w.clone()), "--band-window-slots")?,
                decimal::<u32>(Some(h.clone()), "--band-plausible-half-widths")?,
                decimal::<u32>(Some(c.clone()), "--band-max-cell-share-bps")?,
            ))
        }
        _ => {
            return Err(Error::new(
                "an incomplete founding band was stated. --band-anchor, \
                 --band-volatility-bps, --band-window-slots, \
                 --band-plausible-half-widths and --band-max-cell-share-bps are \
                 required together: the band is the author's belief about the \
                 outcome and no part of it has a default",
            ));
        }
    };
    let spec = market::DevnetPythMarketSpecV1 {
        founding_band,
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
        // `--recovery-rungs BPS:SECONDS_AFTER_PREVIOUS` buys a funded ordered
        // ladder. Absent is the no-recovery market this producer has always
        // compiled, and the flag is REQUIRED to name at least one rung: a
        // caller who typed it meant to buy something, and a policy funding no
        // attempt is the no-recovery market spelled at greater length.
        recovery: match &recovery_rungs {
            None => None,
            Some(raw) => Some(local_mutable::parse_recovery_rungs_v1(raw)?),
        },
    };
    // THE PROVIDER RELEASE IS OBSERVED, NOT TYPED. A market pins its provider
    // release at founding and `authenticate_provider_program_pin` compares it to
    // the chain by exact equality, so a constant that has fallen behind founds a
    // market whose every capture refuses `0x8014 ReleaseSuperseded` -- which is
    // what happened to cohort-13 and cohort-14, four and five days after Pyth
    // redeployed their devnet Receiver. The observation is made BEFORE the
    // family's own compiler runs, so a provider that moved refuses here, free,
    // instead of after a founding has spent its lamports.
    let sponsored_release = match family {
        DevnetMarketFamilyV1::Direct => None,
        DevnetMarketFamilyV1::SponsoredDirect | DevnetMarketFamilyV1::SponsoredGeneral => {
            let (url, acknowledgment) = match family {
                DevnetMarketFamilyV1::SponsoredGeneral => {
                    (general.rpc_url.clone(), general.acknowledgment.clone())
                }
                _ => (direct.rpc_url.clone(), direct.acknowledgment.clone()),
            };
            let origin = cluster::ClusterOriginV1::parse(
                &required(url, "--rpc-url")?,
                acknowledgment.as_deref(),
            )?;
            let mut rpc = rpc::Rpc::connect_cluster(&origin, rpc::WritePolicyV1::ReadsOnly)?;
            let observed =
                sponsored_release_observation::observed_devnet_sponsored_release_v1(&mut rpc, 0)?;
            eprintln!("{}", observed.report());
            Some(observed.release)
        }
    };
    let input = match family {
        DevnetMarketFamilyV1::SponsoredGeneral => {
            let release = sponsored_release.ok_or_else(|| {
                Error::new("a sponsored General market compiled without observing its provider")
            })?;
            let (plan, rpc_url, acknowledgment, general) = general.load()?;
            general_devnet_market::devnet_general_market_input(
                &plan,
                &rpc_url,
                acknowledgment.as_deref(),
                registry,
                spec,
                &general,
                release,
            )?
        }
        DevnetMarketFamilyV1::SponsoredDirect => {
            let release = sponsored_release.ok_or_else(|| {
                Error::new("a sponsored Direct market compiled without observing its provider")
            })?;
            let direct = direct.load(registry)?;
            market::devnet_sponsored_market_input(spec, direct.compiler(), release)?
        }
        DevnetMarketFamilyV1::Direct => {
            let direct = direct.load(registry)?;
            market::devnet_market_input(spec, direct.compiler())?
        }
    };
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
/// `LABEL=I128`, for `--declared-class-delta`.
fn labeled_class_delta_v1(value: &str) -> Result<(String, i128)> {
    let (label, delta) = value.split_once('=').ok_or_else(|| {
        Error::new(format!(
            "--declared-class-delta takes LABEL=I128, got {value:?}"
        ))
    })?;
    let atoms = delta.parse::<i128>().map_err(|_| {
        Error::new(format!(
            "--declared-class-delta {label} must be a decimal i128, got {delta:?}"
        ))
    })?;
    Ok((label.to_owned(), atoms))
}

/// What this census claims each compartment class moved since `--prior`.
///
/// With no `--declared-class-delta` the claim stays INAPPLICABLE and says why.
/// `unchanged()` there would be a claim -- that not one atom changed class --
/// and a census run by an observer who did not drive the transfers has no
/// standing to make it on that caller's behalf.
///
/// A caller that DID drive them has standing, and naming one class is how it
/// says so: from that point every class it does not name is a claim of zero,
/// which is the strong statement L8 exists to check. So the flag both supplies
/// the numbers and transfers the standing, and neither can arrive without the
/// other.
fn census_class_claim_v1(deltas: BTreeMap<String, i128>) -> Result<ledger::ClassClaimV1> {
    if deltas.is_empty() {
        return Ok(ledger::ClassClaimV1::inapplicable(
            "external census: the transactions between boundaries were not driven by this \
             ledger, and it refuses to guess which compartments they crossed. A caller that \
             drove them states them with --declared-class-delta LABEL=ATOMS, after which every \
             unnamed class is a declaration of zero.",
        ));
    }
    ledger::ClassClaimV1::declaring(deltas)
}

/// What this census claims about the lamports its caller's transactions moved.
///
/// `--declared-fees-lamports` is what makes L7 applicable at all: it is the
/// term the law cannot derive, and a caller that can state it is a caller that
/// drove the transactions. The unwatched pair only refines a claim that already
/// exists, so either of them alone refuses rather than defaulting the fee to
/// zero -- a fee of zero is a statement about the chain, not a missing value.
fn census_lamport_claim_v1(
    fees: Option<String>,
    unwatched: Option<String>,
    note: Option<String>,
) -> Result<ledger::LamportClaimV1> {
    let Some(fees) = fees else {
        if unwatched.is_some() || note.is_some() {
            return Err(Error::new(
                "--declared-unwatched-lamports and --declared-unwatched-note refine a lamport \
                 claim this census was not given; pass --declared-fees-lamports, which is what \
                 makes L7 applicable",
            ));
        }
        return Ok(ledger::LamportClaimV1::inapplicable(
            "external census: the transactions between boundaries were not driven by this \
             ledger, and it refuses to guess their fees. A caller that drove them states them \
             with --declared-fees-lamports.",
        ));
    };
    let fees = fees
        .parse::<u64>()
        .map_err(|_| Error::new("--declared-fees-lamports must be a decimal u64"))?;
    let claim = ledger::LamportClaimV1::fees(fees);
    let Some(unwatched) = unwatched else {
        if note.is_some() {
            return Err(Error::new(
                "--declared-unwatched-note describes lamports this census did not declare; pass \
                 --declared-unwatched-lamports with it",
            ));
        }
        return Ok(claim);
    };
    let unwatched = unwatched
        .parse::<u64>()
        .map_err(|_| Error::new("--declared-unwatched-lamports must be a decimal u64"))?;
    Ok(claim.with_unwatched(unwatched, note.unwrap_or_default()))
}

fn run_ledger_census(arguments: Vec<String>) -> Result<()> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut mint = None;
    let mut payer = None;
    let mut hoard = None;
    let mut aggregate = None;
    let mut market = None;
    let mut claim_unit = None;
    let mut stage = None;
    let mut declared_collateral = None;
    let mut declared_hoard = None;
    let mut declared_fees = None;
    let mut declared_unwatched = None;
    let mut declared_unwatched_note = None;
    let mut declared_classes: BTreeMap<String, i128> = BTreeMap::new();
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
            "--declared-class-delta" => {
                let (label, delta) = labeled_class_delta_v1(&value)?;
                if declared_classes.insert(label.clone(), delta).is_some() {
                    return Err(Error::new(format!(
                        "--declared-class-delta {label} may be supplied only once"
                    )));
                }
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
            "--market" => &mut market,
            "--claim-unit-atoms" => &mut claim_unit,
            "--stage" => &mut stage,
            "--declared-collateral-delta" => &mut declared_collateral,
            "--declared-hoard-delta" => &mut declared_hoard,
            "--declared-fees-lamports" => &mut declared_fees,
            "--declared-unwatched-lamports" => &mut declared_unwatched,
            "--declared-unwatched-note" => &mut declared_unwatched_note,
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
    // OPTIONAL, and the option is the honest shape: a census taken before a
    // founding commits has no Market to bind, and a census that omits it keeps
    // exactly the behaviour it had. What binding it buys is that L4 can say
    // "this Market is terminal" from the CHAIN rather than from the operator.
    if let Some(address) = market {
        census.track_market(parse_pubkey(Some(address), "--market")?);
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
        census_lamport_claim_v1(declared_fees, declared_unwatched, declared_unwatched_note)?,
        census_class_claim_v1(declared_classes)?,
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
        dclutch_source::relay::decode::RelayedObservableV1::DbcMigrationProgressV1,
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
    let mut observed_upgrade_authority_raw = None;
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
    // THE EIGHTH PUBLICATION, kept out of `RoleDeploymentsV1` on purpose: that
    // struct is "the seven roles' deployment sources" and the accelerator is
    // not a role. It borrows the roles' flag GRAMMAR because the facts are the
    // same facts, and nothing else.
    let mut general_accelerator_deployment = plan::RoleDeploymentInputV1::default();
    let mut general_accelerator_program = None;
    let mut general_accelerator_elf = None;
    let mut general_accelerator_sha256 = None;
    let mut general_accelerator_semantic_release = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        if let Some(rest) = argument.strip_prefix("--") {
            if let Some(role) = rest.strip_suffix("-live-elf-sha256")
                && let Some(target) = prepare_deployment_target(
                    &mut deployments,
                    &mut general_accelerator_deployment,
                    role,
                )
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
                && let Some(target) = prepare_deployment_target(
                    &mut deployments,
                    &mut general_accelerator_deployment,
                    role,
                )
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
                && let Some(target) = prepare_deployment_target(
                    &mut deployments,
                    &mut general_accelerator_deployment,
                    role,
                )
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
                && let Some(target) = prepare_deployment_target(
                    &mut deployments,
                    &mut general_accelerator_deployment,
                    role,
                )
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
            "--observed-upgrade-authority" => &mut observed_upgrade_authority_raw,
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
            "--general-accelerator-program-id" => &mut general_accelerator_program,
            "--general-accelerator-elf" => &mut general_accelerator_elf,
            "--general-accelerator-sha256" => &mut general_accelerator_sha256,
            "--general-accelerator-semantic-release-id" => {
                &mut general_accelerator_semantic_release
            }
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
    // ASSEMBLED HERE, before any evidence is read, because a half-supplied flag
    // group is a shape error in the ARGUMENTS and must refuse where the other
    // shape errors do. Deferring it into the `PrepareArgs` literal put it
    // behind `--rpc-url is required`, so the caller who mistyped one of four
    // flags was told about a fifth.
    let general_accelerator = general_accelerator_prepare_input(
        general_accelerator_program,
        general_accelerator_elf,
        general_accelerator_sha256,
        general_accelerator_semantic_release,
        general_accelerator_deployment,
    )?;
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
                // An AlreadyCurrent role's live bytes ARE the candidate, so the
                // plan projection wants exactly what it wants for an Upgrade: the
                // freshly observed ProgramData, never the carry-forward snapshot.
                // The only difference is upstream, in how the role was satisfied.
                model::CheckedDeploymentDispositionV1::Upgrade
                | model::CheckedDeploymentDispositionV1::AlreadyCurrent => {
                    if input.observed_programdata.is_none() {
                        return Err(Error::new(format!(
                            "--deployment-set-journal requires --{flag_role}-observed-programdata for a receipt-backed Upgrade or an AlreadyCurrent role"
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
    // A cohort deployed MUTABLE and succeeding nothing declares the authority
    // its observed ProgramData carries. It is a declaration the observation
    // must match, not a way around the check: `role_deployment` still refuses
    // when the account disagrees with what was stated.
    let observed_upgrade_authority = match observed_upgrade_authority_raw {
        None => None,
        Some(raw) => Some(parse_pubkey(Some(raw), "--observed-upgrade-authority")?),
    };
    let args = plan::PrepareArgs {
        observed_upgrade_authority,
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
        // OPTIONAL, AND LEGAL BESIDE `--deployment-set-journal`. The journal is
        // the sole owner of the SEVEN checked roles and refuses every raw role
        // flag; it names no accelerator, so it can neither supply this group
        // nor contradict it. A cohort-14 prepare is a checked prepare AND
        // carries this publication, so refusing the combination would make the
        // step unreachable exactly where it is needed.
        general_accelerator,
        record_publication: match record_publication.as_deref() {
            None => plan::RecordPublicationV1::Genesis,
            Some(value) => plan::RecordPublicationV1::parse(value)?,
        },
        deployments,
    };
    let path = args.plan_path.clone();
    let prepared = plan::prepare(args)?;
    // The public cut's `checkedReleases` row, emitted verbatim so nobody types
    // it. The browser will not compute a release-set id, a gate digest or a
    // sealed-set digest, and a hand-typed row is a mirror: the moment it drifts
    // the site says a market is fillable that is not, or the reverse. The map is
    // keyed exactly as `publicCutStaging.ts` keys it, with exactly the two
    // fields it admits, so the cut splices this object in without rewriting it.
    // An unsealed plan emits an EMPTY map rather than no key -- "none are
    // sealed" is an answer, and an absent key is a question.
    let cut_fragment = serde_json::json!({
        "schema": "dclutch-public-cut-checked-releases-fragment-v1",
        "checkedReleases": match prepared.checked_upgrade_set.as_ref() {
            Some(set) => serde_json::json!({
                prepared.release_set_id.clone(): {
                    "gateDigest": set.checked_release_gate_sha256.clone(),
                    "sealedSet": set.final_set_sha256.clone(),
                }
            }),
            None => serde_json::json!({}),
        },
    });
    let fragment_path = PathBuf::from(format!("{}.checked-releases.json", path.display()));
    crate::release_capture::write_json_atomic_new(&fragment_path, &cut_fragment)?;
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
        "release_set_id": prepared.release_set_id,
        "public_cut_checked_releases_fragment": fragment_path.display().to_string(),
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
/// Resolve a `--<role>-<suffix>` flag to the deployment input it fills.
///
/// The accelerator is resolved here rather than inside [`deployment_target`]
/// so that function keeps meaning exactly "one of the seven cohort roles".
/// This is the only place the two vocabularies meet, and the flag spellings
/// fall out of the suffix grammar rather than being written a second time:
/// `--general-accelerator-observed-programdata`,
/// `--general-accelerator-live-elf-sha256`,
/// `--general-accelerator-expected-upgrade-authority` and
/// `--general-accelerator-genesis-deployment-slot` all exist because this arm
/// does. Note the divergence from `devnet-general-market`, which spells the
/// authority `--general-accelerator-upgrade-authority`: two commands, two
/// grammars, and `prepare`'s is the one with seven other users.
fn prepare_deployment_target<'a>(
    deployments: &'a mut plan::RoleDeploymentsV1,
    general_accelerator: &'a mut plan::RoleDeploymentInputV1,
    role: &str,
) -> Option<&'a mut plan::RoleDeploymentInputV1> {
    if role == "general-accelerator" {
        return Some(general_accelerator);
    }
    deployment_target(deployments, role)
}

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

/// Assemble the accelerator group, or refuse a HALF-SUPPLIED one.
///
/// All four scalars absent means no accelerator publication, which is every
/// cohort before the 14th and is not an error. All four present means one.
/// Anything between is a typo that would otherwise publish nothing and report
/// success -- the failure mode this repository calls silent success -- so it is
/// refused by naming exactly which flags are missing.
fn general_accelerator_prepare_input(
    program: Option<String>,
    elf: Option<String>,
    sha256: Option<String>,
    semantic_release_id: Option<String>,
    deployment: plan::RoleDeploymentInputV1,
) -> Result<Option<plan::GeneralAcceleratorPrepareInputV1>> {
    let supplied = [
        ("--general-accelerator-program-id", program.is_some()),
        ("--general-accelerator-elf", elf.is_some()),
        ("--general-accelerator-sha256", sha256.is_some()),
        (
            "--general-accelerator-semantic-release-id",
            semantic_release_id.is_some(),
        ),
    ];
    let present = supplied.iter().filter(|(_, given)| *given).count();
    let deployment_supplied = deployment.observed_programdata.is_some()
        || deployment.expected_live_elf_sha256.is_some()
        || deployment.expected_upgrade_authority.is_some()
        || deployment.genesis_deployment_slot != 0;
    if present == 0 {
        if deployment_supplied {
            return Err(Error::new(
                "a --general-accelerator-* deployment flag was supplied without \
                 --general-accelerator-program-id, --general-accelerator-elf, \
                 --general-accelerator-sha256 and --general-accelerator-semantic-release-id",
            ));
        }
        return Ok(None);
    }
    if present != supplied.len() {
        let missing: Vec<&str> = supplied
            .iter()
            .filter(|(_, given)| !*given)
            .map(|(label, _)| *label)
            .collect();
        return Err(Error::new(format!(
            "the General accelerator publication needs its whole flag group; missing {}",
            missing.join(", ")
        )));
    }
    Ok(Some(plan::GeneralAcceleratorPrepareInputV1 {
        program: parse_pubkey(program, "--general-accelerator-program-id")?,
        elf: absolute(elf, "--general-accelerator-elf")?,
        elf_sha256: required(sha256, "--general-accelerator-sha256")?,
        semantic_release_id: required(
            semantic_release_id,
            "--general-accelerator-semantic-release-id",
        )?,
        deployment,
    }))
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

/// One 64-hex-digit identity from the command line.
///
/// Was nested inside `run_graduation_market`; `devnet-general-market` needs
/// the identical parse for the accelerator's semantic release id, and two
/// spellings of "64 hex digits" is one more than the number of ways this can
/// be got wrong.
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

fn parse_pubkey(value: Option<String>, label: &str) -> Result<Pubkey> {
    plan::pubkey(&required(value, label)?)
}

fn campaign_usage_v1() -> String {
    format!(
        "\n  dclutch-local-successor-bootstrap campaign --rpc-url URL [{ack} GENESIS_HASH] \
         --plan ABSOLUTE_JSON [--evidence ABSOLUTE_JSON] \
         [--infrastructure-lineage-evidence ABSOLUTE_JSON] [--through STAGE] [--execute] \
         [--keypair-core-upgrade-authority ABSOLUTE_KEYPAIR_JSON] \
         [--keypair-campaign-payer ABSOLUTE_KEYPAIR_JSON]\n\
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
         administration preflight through activation. The Core upgrade authority is loaded lazily \
         only when execution has an incomplete admitted stage. A checked-local succession also \
         requires --keypair-campaign-payer: this must be a different wallet because the Core/Registry \
         consent key is readonly in the ceremony while the fee payer is writable. The succession \
         stage performs one real Registry Loader Upgrade, publishes its observed-slot artifact, and \
         creates V2 before activation; plans without the checked-local pin treat it as not applicable. \
         The optional standalone lineage output is admitted only by executed administration through \
         activation; it re-derives source, checked artifact, V1/V2 profile, Registry/Rent record, and \
         activated release identities from finalized chain state and never replaces a differing file. \
         --founding-only is a disjoint path: publication, profile initialization, succession, and \
         activation must already read Complete before any key file opens. It never accepts an upgrade-authority \
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
    println!("{}", direct_resolution_campaign::usage());
    println!("{}", aggregate_retirement_exterior::usage());
    println!("{}", aggregate_retirement_exterior::devnet_usage());
    println!("{}", source_abort_exterior::usage());
    println!("{}", source_abort_exterior::interruption_audit_usage());
    println!("{}", user_position_admission::usage());
    println!("{}", user_position_admission::local_usage());
    println!("{}", user_position_close::usage());
    println!("{}", direct_terminal_children::usage());
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
    println!("{}", claims_custody_replay::usage());
    println!("{}", claims_custody_replay::devnet_usage());
    println!("{}", direct_fee_settlement::usage());
    println!("{}", direct_close_maker::usage());
    println!("{}", capability_seal_close::usage());
    println!("{}", capability_seal_devnet::usage());
    println!("{}", general_successor_plan::lookup_table_usage());
    println!("{}", general_successor_plan::execute_usage());
    println!("{}", general_session::usage());
    println!("{}", release_lineage::usage());
    println!("{}", infrastructure_succession::usage());
    println!("{}", flagship_resolution::usage());
    println!("{}", flagship_resolution::owned_loopback_usage());
    println!("{}", sponsored_push::usage());
    println!("{}", sponsored_push::owned_loopback_usage());
    println!("{}", sponsored_push::input_usage());
    println!("{}", sponsored_schedule::usage());
    println!("{}", sponsored_push::input_owned_loopback_usage());
    println!("{}", wallet_terminal::usage());
    println!("{}", wallet_terminal_payout_exterior::usage());
    println!("{}", wallet_terminal_payout_exterior::devnet_usage());
    println!("{}", direct_trade::usage());
    println!("{}", direct_trade_producer::usage());
    println!("{}", direct_trade_producer::devnet_session_usage());
    println!("{}", direct_trade_producer::devnet_direct_usage());
    println!("{}", direct_ticket::usage());
    println!(
        "{}",
        direct_hot_route_manifest::checked_execution_release_usage()
    );
    println!("{}", direct_hot_route_manifest::hot_route_manifest_usage());
    println!("{}", direct_capability_activation::usage());
    println!("{}", direct_capability_activation::owned_loopback_usage());
    println!("{}", general_capability_activation::usage());
    println!("{}", general_successor_plan::usage());
    println!("{}", series_terminal_campaign::usage());
    println!("{}", campaign_usage_v1());
    println!(
        "\n{direct_market_usage}\n  dclutch-local-successor-bootstrap ledger-census \
         --rpc-url URL [{ack} GENESIS_HASH] --mint PUBKEY --payer PUBKEY --hoard PUBKEY \
         --aggregate PUBKEY --claim-unit-atoms U64 --stage NAME --output ABSOLUTE_JSON \
         [--market PUBKEY] [--token LABEL=PUBKEY]... [--position LABEL=PUBKEY]... [--watch LABEL=PUBKEY]... \
         [--prior ABSOLUTE_JSON] [--declared-collateral-delta I128] [--declared-hoard-delta I128] \
         [--declared-class-delta LABEL=I128]... [--declared-fees-lamports U64] \
         [--declared-unwatched-lamports U64 --declared-unwatched-note TEXT]\n\
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
         evaluate across invocations. L7 and L8 are inapplicable to a bare invocation, which \
         is an external observer that did not drive the transfers between its two boundaries: \
         a caller that DID drive them states them with --declared-fees-lamports and \
         --declared-class-delta, and from the first declared class every unnamed one is a \
         declaration of zero. Declared class labels are the census's own compartment names.",
        ack = campaign::DEVNET_ACKNOWLEDGMENT_FLAG_NAME,
        direct_market_usage = direct_market_usage_v1(),
    );
}

fn usage_supervisor() {
    println!(
        "Usage:\n  dclutch-local-successor-bootstrap prepare --account-dir ABSOLUTE_NEW_DIR --output ABSOLUTE_NEW_JSON --deployment-set-journal ABSOLUTE_JSON --rpc-url https://api.devnet.solana.com --i-mean-devnet DEVNET_GENESIS --solana-cli ABSOLUTE_EXECUTABLE --custody-observed-programdata ABSOLUTE_BODY --resolution-observed-programdata ABSOLUTE_BODY --claims-observed-programdata ABSOLUTE_BODY --trading-observed-programdata ABSOLUTE_BODY --core-observed-programdata ABSOLUTE_BODY [--general-accelerator-program-id PUBKEY --general-accelerator-elf ABSOLUTE_SO --general-accelerator-sha256 64_LOWERCASE_HEX --general-accelerator-semantic-release-id 64_LOWERCASE_HEX --general-accelerator-observed-programdata ABSOLUTE_BODY --general-accelerator-live-elf-sha256 64_LOWERCASE_HEX --general-accelerator-expected-upgrade-authority PUBKEY]\n  dclutch-local-successor-bootstrap run --spec ABSOLUTE_JSON [--keypair-seed 64_LOWERCASE_HEX]\n  dclutch-local-successor-bootstrap demo-market (always refuses: retired local-only fixture)\n\nThe checked deployment-set form is the only prepare admission for the permanent devnet set. Registry and Rent are exact CarryForward rows sourced only from the authenticated one-context snapshot; their raw program, ELF, ProgramData, semantic, slot, authority, and publication flags are refused. Custody, Resolution, Claims, Trading, and Core require exact complete Upgrade receipts and hostile current ProgramData bodies. Prepare first reruns the key-free live finalized audit, then rehashes all evidence and reproduces the existing Registry/Rent ArtifactRelease records and singleton profile byte-for-byte. demo-market cannot authenticate permanent-devnet Direct facts and refuses instead of inventing them. The --general-accelerator-* group is OPTIONAL and legal beside --deployment-set-journal: the journal owns the seven checked roles and names no accelerator, so it can neither supply this publication nor contradict it. Supply the whole group or none of it; a half-supplied group refuses by naming the missing flags. It publishes an eighth ArtifactRelease record whose FINALIZATION is the cohort's own observation of the accelerator's deployment, which is what a General-manifest market needs before it can be founded. Its semantic release id is operator-stated, because no protocol-owned derivation exists for a role outside the seven."
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
        assert!(admin.infrastructure_lineage_path.is_none());

        let lineage_path = PathBuf::from("/current-source-infrastructure-lineage.json");
        let executed = parse_campaign_args_v1(vec![
            "--rpc-url".into(),
            "http://127.0.0.1:20890/".into(),
            "--plan".into(),
            "/campaign-plan-must-not-be-read.json".into(),
            "--evidence".into(),
            "/campaign-evidence.json".into(),
            "--infrastructure-lineage-evidence".into(),
            lineage_path.display().to_string(),
            "--execute".into(),
        ])
        .expect("executed administration lineage surface");
        assert_eq!(
            executed.infrastructure_lineage_path.as_ref(),
            Some(&lineage_path)
        );
        assert!(executed.execute);

        let succession = parse_campaign_args_v1(vec![
            "--rpc-url".into(),
            "http://127.0.0.1:20890/".into(),
            "--plan".into(),
            "/campaign-plan-must-not-be-read.json".into(),
            "--through".into(),
            "succession".into(),
            "--keypair-core-upgrade-authority".into(),
            "/core-authority-must-not-be-read.json".into(),
            "--keypair-campaign-payer".into(),
            "/succession-payer-must-not-be-read.json".into(),
        ])
        .expect("succession administration surface");
        assert_eq!(succession.through, campaign::StageV1::Succession);
        assert_eq!(succession.keypairs.len(), 2);

        let mut projection_with_lineage = vec![
            "--rpc-url".into(),
            "http://127.0.0.1:20890/".into(),
            "--plan".into(),
            "/campaign-plan-must-not-be-read.json".into(),
            "--infrastructure-lineage-evidence".into(),
            "/lineage-must-not-be-written.json".into(),
        ];
        assert!(
            parse_campaign_args_v1(projection_with_lineage.clone())
                .expect_err("read-only lineage projection")
                .0
                .contains("requires executed administration through activation")
        );
        projection_with_lineage.push("--execute".into());
        projection_with_lineage.extend(["--through".into(), "succession".into()]);
        assert!(
            parse_campaign_args_v1(projection_with_lineage)
                .expect_err("prefix lineage projection")
                .0
                .contains("requires executed administration through activation")
        );

        let founding = parse_campaign_args_v1(founding_campaign_cli_v1())
            .expect("exact founding-only surface");
        assert_eq!(founding.mode, campaign::CampaignModeV1::FoundingOnly);
        assert_eq!(founding.through, campaign::StageV1::Founding);
        assert_eq!(
            founding.keypairs.len(),
            campaign::FOUNDING_REQUIRED_ROLES.len()
        );
        assert_ne!(founding.founding_founder, founding.substituted_founder);

        let mut founding_with_lineage = founding_campaign_cli_v1();
        founding_with_lineage.extend([
            "--infrastructure-lineage-evidence".into(),
            "/lineage-must-not-be-written.json".into(),
        ]);
        assert!(
            parse_campaign_args_v1(founding_with_lineage)
                .expect_err("founding lineage ownership")
                .0
                .contains("--founding-only refuses --infrastructure-lineage-evidence")
        );
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

    /// THE ACCELERATOR GROUP IS LEGAL BESIDE THE JOURNAL, and half of it is not.
    ///
    /// Both halves matter. `--deployment-set-journal` refuses every raw ROLE
    /// flag by name, and a cohort-14 prepare is a checked prepare that also
    /// carries this publication -- so if the journal refused the group too, the
    /// step would be unreachable exactly where it is needed. And a group
    /// supplied in part must refuse rather than publish nothing and exit zero,
    /// which is this repository's named silent-success failure.
    ///
    /// Every refusal here is a PARSE refusal, asserted by the fixture's own
    /// unreadable journal path never appearing in the message.
    #[test]
    fn the_general_accelerator_prepare_group_is_whole_or_absent() {
        let program = Pubkey::new_from_array([9; 32]).to_string();
        let whole: Vec<&str> = vec![
            "--general-accelerator-program-id",
            &program,
            "--general-accelerator-elf",
            "/general-accelerator-must-not-be-read.so",
            "--general-accelerator-sha256",
            "00000000000000000000000000000000000000000000000000000000000000ff",
            "--general-accelerator-semantic-release-id",
            "00000000000000000000000000000000000000000000000000000000000000aa",
        ];
        // The whole group parses beside the journal: it gets past the flag
        // table and dies on the journal file, which is where every other
        // checked-prepare argument dies too.
        let refusal = run_prepare(checked(&whole)).expect_err("the journal file is unreadable");
        assert!(
            !refusal.0.contains("unknown prepare argument"),
            "the accelerator group must parse: {}",
            refusal.0
        );
        assert!(
            !refusal.0.contains("forbidden"),
            "the journal owns the seven roles and names no accelerator: {}",
            refusal.0
        );

        // Each single flag on its own names the three that are missing.
        for index in [0_usize, 2, 4, 6] {
            let partial: Vec<&str> = whole[index..index + 2].to_vec();
            let refusal =
                run_prepare(checked(&partial)).expect_err("a half-supplied group must refuse");
            assert!(
                refusal.0.contains("needs its whole flag group"),
                "{}",
                refusal.0
            );
            assert!(
                !refusal.0.contains("this-file-must-not-be-read"),
                "the refusal must precede any evidence read: {}",
                refusal.0
            );
        }

        // A deployment flag with no group at all is the same defect one level
        // down: the suffix grammar accepts `general-accelerator` as a target,
        // so this would otherwise be silently discarded.
        let refusal = run_prepare(checked(&[
            "--general-accelerator-observed-programdata",
            "/general-accelerator-programdata-must-not-be-read.bin",
        ]))
        .expect_err("a lone deployment flag must refuse");
        assert!(
            refusal.0.contains("deployment flag was supplied without"),
            "{}",
            refusal.0
        );
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

    // ---------- what `ledger-census` declares, and to whom ----------

    /// One census of a market whose collateral sits in Custody compartments.
    ///
    /// Deliberately thin: no Hoard admitted, no aggregate, no positions, so
    /// L2/L3/L4 record themselves inapplicable and the only law with anything
    /// to say about a class is L8. `tracked_collateral` equals `mint_supply`
    /// and both are the class table's own sum, so a movement BETWEEN classes
    /// leaves L1 and L5 green — which is precisely the blindness L8 exists for.
    fn census_observation_v1(
        stage: &str,
        class_atoms: BTreeMap<String, u64>,
        classes: ledger::ClassClaimV1,
    ) -> ledger::ObservationV1 {
        let tracked: u64 = class_atoms.values().sum();
        ledger::ObservationV1 {
            stage: stage.to_owned(),
            slot: 1,
            declared_collateral_delta: 0,
            declared_hoard_delta: 0,
            lamports: ledger::LamportClaimV1::inapplicable("this census states no lamport claim"),
            payer_lamports: 1_000_000,
            mint_supply: tracked,
            token_atoms: BTreeMap::new(),
            class_atoms,
            declared_class_deltas: classes,
            tracked_collateral: tracked,
            hoard_atoms: 0,
            outcome_count: 0,
            aggregate_supply: Vec::new(),
            position_balances: BTreeMap::new(),
            position_totals: Vec::new(),
            accounts: BTreeMap::new(),
            market_phase: None,
            verdicts: Vec::new(),
        }
    }

    /// A bare census stays INAPPLICABLE for L8; a declared one makes a real
    /// claim; a label the census does not report refuses by name.
    #[test]
    fn declared_class_deltas_make_a_real_claim_and_an_unknown_label_refuses() {
        let bare = census_class_claim_v1(BTreeMap::new()).expect("a census that declares nothing");
        assert!(bare.deltas.is_empty());
        assert!(
            bare.inapplicable
                .as_deref()
                .is_some_and(|reason| reason.contains("--declared-class-delta")),
            "{:?}",
            bare.inapplicable
        );

        let declared = census_class_claim_v1(BTreeMap::from([
            ("FeeVault".to_owned(), 250_i128),
            ("unclassified".to_owned(), -250_i128),
        ]))
        .expect("the census's own compartment labels");
        assert_eq!(declared.inapplicable, None);
        assert_eq!(declared.deltas.get("FeeVault"), Some(&250));
        assert_eq!(declared.deltas.get("unclassified"), Some(&-250));

        // A near miss is the dangerous one: it would leave the compartment the
        // caller MEANT declared at zero.
        let refusal = census_class_claim_v1(BTreeMap::from([("feevault".to_owned(), 250_i128)]))
            .err()
            .expect("a class this census does not report must refuse");
        assert!(refusal.0.contains("feevault"), "{}", refusal.0);
        assert!(refusal.0.contains("FeeVault"), "{}", refusal.0);
        assert!(refusal.0.contains("unclassified"), "{}", refusal.0);

        assert_eq!(
            labeled_class_delta_v1("HoardPrincipal=-7").expect("LABEL=I128"),
            ("HoardPrincipal".to_owned(), -7)
        );
        assert!(labeled_class_delta_v1("HoardPrincipal").is_err());
        assert!(labeled_class_delta_v1("HoardPrincipal=seven").is_err());
    }

    /// The lamport declarations reach L7 the same way, and neither half of the
    /// unwatched pair can arrive without the fee that makes the law applicable.
    #[test]
    fn declared_lamports_make_a_real_claim_and_a_bare_unwatched_term_refuses() {
        let bare = census_lamport_claim_v1(None, None, None).expect("a census that declares none");
        assert!(
            bare.inapplicable
                .as_deref()
                .is_some_and(|reason| reason.contains("--declared-fees-lamports")),
            "{:?}",
            bare.inapplicable
        );

        let declared = census_lamport_claim_v1(
            Some("15000".to_owned()),
            Some("2039280".to_owned()),
            Some("the cycle's own address lookup table".to_owned()),
        )
        .expect("fees plus a described unwatched term");
        assert_eq!(declared.inapplicable, None);
        assert_eq!(declared.fees_lamports, 15_000);
        assert_eq!(declared.unwatched_lamports, 2_039_280);
        assert!(declared.unwatched_note.contains("lookup table"));

        assert!(census_lamport_claim_v1(None, Some("1".to_owned()), None).is_err());
        assert!(census_lamport_claim_v1(Some("1".to_owned()), None, Some("x".to_owned())).is_err());
        assert!(census_lamport_claim_v1(Some("negative-one".to_owned()), None, None).is_err());
    }

    /// THE NEGATIVE CONTROL, through this command's own path.
    ///
    /// The journey proved L8 can fire. This proves the flag that feeds it can
    /// REACH it from `ledger-census`, where the law was unconditionally
    /// inapplicable until now: a caller declares the compartment it knows it
    /// debited, 250 atoms arrive in a compartment it did not name — which the
    /// claim states moved zero — and L8 goes red naming `FeeVault`. L1 and L5
    /// stay green beside it, because the total never moved.
    #[test]
    fn a_class_declared_unchanged_that_moved_reds_l8_through_the_census_flags() {
        let mut census = ledger::ConservationLedgerV1::new(
            Pubkey::new_from_array([0xc1; 32]),
            Pubkey::new_from_array([0xd1; 32]),
        );
        census.restore_observations(vec![census_observation_v1(
            "before",
            BTreeMap::from([
                ("HoardPrincipal".to_owned(), 10_000),
                ("FeeVault".to_owned(), 0),
            ]),
            ledger::ClassClaimV1::inapplicable("the first census has no predecessor"),
        )]);
        let declared =
            census_class_claim_v1(BTreeMap::from([("HoardPrincipal".to_owned(), -250_i128)]))
                .expect("--declared-class-delta HoardPrincipal=-250");
        let verdicts = census.evaluate(&census_observation_v1(
            "after",
            BTreeMap::from([
                ("HoardPrincipal".to_owned(), 9_750),
                ("FeeVault".to_owned(), 250),
            ]),
            declared,
        ));
        let named = |law: &str| {
            verdicts
                .iter()
                .find(|verdict| verdict.law == law)
                .unwrap_or_else(|| panic!("{law} must be evaluated"))
                .clone()
        };
        let l8 = named("L8");
        assert_eq!(l8.status, "violated", "{}", l8.detail);
        assert!(
            l8.detail.contains("FeeVault moved +250 atoms"),
            "{}",
            l8.detail
        );
        assert!(!l8.detail.contains("HoardPrincipal moved"), "{}", l8.detail);
        assert_eq!(named("L1").status, "holds");
        assert_eq!(named("L5").status, "holds");
    }
}
