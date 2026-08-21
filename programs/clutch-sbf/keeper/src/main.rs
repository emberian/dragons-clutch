//! `clutch-keeper` — the permissionless cranker for the Dragon's Clutch
//! general clearing plane.
//!
//! Four subcommands:
//!
//! * `run` — watch a loopback validator and drive every permissionless step
//!   that is due, forever or until the lifecycle ends.  The keeper holds no
//!   memory between polls, so killing it mid-walk and starting it again is
//!   indistinguishable from a slow poll.
//! * `prime` — replay a prefix of a pregenerated committed plan.  This is the
//!   market-and-orders setup a gate needs, and it is deliberately *not* part
//!   of the crank: placements are participants' business.
//! * `addresses` — print every derived address of one plane, for a gate
//!   script that wants to poll the chain itself.
//! * `fold-wire-probe` — measure how many `FoldResolutionWork` instructions
//!   fit one real cluster packet, and ask a real validator to confirm it.
//!
//! Every action prints exactly one structured line carrying the slot, the
//! action, the W1 route it spent against, and the compute units the bank
//! actually charged next to the limit that was requested.

mod crank;
mod pda;
mod probe;
mod quotes;
mod rpc;
mod wire;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use clutch_batch_policy_identity::{
    batch_policy_digest, general_clearing_v1::GENERAL_CLEARING_POLICY_V1,
};
use clutch_solana_layout::Hash32;
use crank::{Config, Keeper, Step};
use rpc::Rpc;
use serde::Deserialize;
use solana_keypair::{read_keypair_file, Keypair};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

type Res<T> = Result<T, String>;

fn usage() -> ! {
    eprintln!(
        "usage:
  clutch-keeper run --url URL --program ID --realm B58 --market B58 --epoch-index N
                    --payer KEY [--owner KEY]... [--open] [--policy B58]
                    [--deadline-slots N] [--max-actions N] [--exit-when-idle]
                    [--exit-when-blocked] [--poll-ms N]
  clutch-keeper prime --url URL --plan DIR --key KEY... [--steps A-B] [--skip NAME]...
  clutch-keeper addresses --program ID --realm B58 --market B58 --epoch-index N [--policy B58]
  clutch-keeper fold-wire-probe --program ID --realm B58 --market B58 --feed B58
                    --window B58 --terms B58 [--url URL] [--widths 1,2,4,6,8,12]"
    );
    std::process::exit(2);
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }
    let command = args.remove(0);
    let flags = Flags::parse(&args);
    let outcome = match command.as_str() {
        "run" => run(&flags),
        "prime" => prime(&flags),
        "addresses" => addresses(&flags),
        "fold-wire-probe" => fold_wire_probe(&flags),
        _ => usage(),
    };
    if let Err(error) = outcome {
        eprintln!("clutch-keeper: {error}");
        std::process::exit(1);
    }
}

/// A minimal `--flag value` / `--flag` parser; repeated flags accumulate.
struct Flags {
    values: BTreeMap<String, Vec<String>>,
}

impl Flags {
    fn parse(args: &[String]) -> Self {
        let mut values: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut at = 0;
        while at < args.len() {
            let Some(name) = args[at].strip_prefix("--") else {
                at += 1;
                continue;
            };
            let next = args.get(at + 1);
            match next {
                Some(value) if !value.starts_with("--") => {
                    values.entry(name.to_string()).or_default().push(value.clone());
                    at += 2;
                }
                _ => {
                    values.entry(name.to_string()).or_default().push(String::new());
                    at += 1;
                }
            }
        }
        Self { values }
    }

    fn all(&self, name: &str) -> &[String] {
        self.values.get(name).map_or(&[], Vec::as_slice)
    }

    fn one(&self, name: &str) -> Res<String> {
        self.values
            .get(name)
            .and_then(|values| values.first())
            .cloned()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("--{name} is required"))
    }

    fn optional(&self, name: &str) -> Option<String> {
        self.values
            .get(name)
            .and_then(|values| values.first())
            .cloned()
            .filter(|value| !value.is_empty())
    }

    fn present(&self, name: &str) -> bool {
        self.values.contains_key(name)
    }

    fn number(&self, name: &str, fallback: u64) -> Res<u64> {
        match self.optional(name) {
            Some(text) => text
                .parse()
                .map_err(|_| format!("--{name} must be a number, got {text}")),
            None => Ok(fallback),
        }
    }
}

fn hash_arg(flags: &Flags, name: &str) -> Res<Hash32> {
    Ok(Hash32::from_bytes(wire::base58_decode_32(
        &flags.one(name)?,
    )?))
}

fn policy_digest(flags: &Flags) -> Res<Hash32> {
    match flags.optional("policy") {
        Some(text) => Ok(Hash32::from_bytes(wire::base58_decode_32(&text)?)),
        None => Ok(Hash32::from_bytes(
            batch_policy_digest(&GENERAL_CLEARING_POLICY_V1)
                .map_err(|error| format!("the frozen general-clearing policy did not digest: {error:?}"))?
                .0,
        )),
    }
}

fn load_keys(paths: &[String]) -> Res<Vec<Keypair>> {
    let mut out = Vec::with_capacity(paths.len());
    for path in paths {
        out.push(
            read_keypair_file(path).map_err(|error| format!("{path} is not a keypair: {error}"))?,
        );
    }
    Ok(out)
}

fn config(flags: &Flags) -> Res<Config> {
    let program_id_b58 = flags.one("program")?;
    Ok(Config {
        program_id: wire::base58_decode_32(&program_id_b58)?,
        program_id_b58,
        realm: hash_arg(flags, "realm")?,
        market: hash_arg(flags, "market")?,
        epoch_index: flags.number("epoch-index", 0)?,
        policy: policy_digest(flags)?,
        freeze_deadline_slots: flags.number("deadline-slots", 120)?,
        may_open: flags.present("open"),
    })
}

// --- run -----------------------------------------------------------------

fn run(flags: &Flags) -> Res<()> {
    let rpc = Rpc::new(&flags.one("url")?)?;
    let cfg = config(flags)?;
    let mut paths = vec![flags.one("payer")?];
    paths.extend_from_slice(flags.all("owner"));
    let keys = load_keys(&paths)?;
    let mut keeper = Keeper::new(cfg.clone(), rpc.clone(), keys)?;

    let max_actions = flags.number("max-actions", u64::MAX)?;
    let poll = Duration::from_millis(flags.number("poll-ms", 400)?);
    let exit_when_idle = flags.present("exit-when-idle");
    let exit_when_blocked = flags.present("exit-when-blocked");

    ensure_ready(&rpc, &keeper, flags)?;

    println!(
        "keeper start payer={} program={} market={} epoch_index={} epoch={} open={}",
        keeper.payer_address(),
        cfg.program_id_b58,
        wire::base58(&cfg.market.bytes()),
        cfg.epoch_index,
        wire::base58(&keeper.epoch_id().bytes()),
        cfg.may_open
    );

    let mut taken = 0_u64;
    let mut recent: Vec<String> = Vec::new();
    let mut idle_reason = String::new();
    let mut blocked_reason = String::new();

    // The one action that is not part of the poll ladder: opening the epoch
    // and its page, which only happens when the operator asked for it.
    while let Some(act) = keeper.open_if_absent()? {
        guard_progress(&mut recent, &act)?;
        take(&rpc, &act)?;
        taken += 1;
    }

    loop {
        if taken >= max_actions {
            println!("keeper stop reason=max-actions actions={taken}");
            return Ok(());
        }
        match keeper.next()? {
            Step::Done => {
                println!("keeper stop reason=lifecycle-complete actions={taken}");
                return Ok(());
            }
            Step::Blocked { reason } => {
                if reason != blocked_reason {
                    println!("keeper blocked actions={taken} reason=\"{reason}\"");
                    blocked_reason = reason;
                }
                if exit_when_blocked {
                    return Ok(());
                }
                thread::sleep(poll);
            }
            Step::Idle { reason, wait_until } => {
                // A keeper waiting out a thousand-slot candidate window would
                // otherwise print a line every poll for several minutes; the
                // reason is what carries information, so only a change does.
                if reason != idle_reason {
                    println!(
                        "keeper idle slot={} actions={taken} wait_until={} reason=\"{reason}\"",
                        rpc.slot()?,
                        wait_until.map_or_else(|| "-".to_string(), |slot| slot.to_string())
                    );
                    idle_reason = reason;
                }
                if exit_when_idle {
                    return Ok(());
                }
                thread::sleep(poll);
            }
            Step::Act(act) => {
                guard_progress(&mut recent, &act)?;
                take(&rpc, &act)?;
                taken += 1;
            }
        }
    }
}

/// How many recent actions the cycle guard remembers.
const PROGRESS_WINDOW: usize = 8;

/// Refuse to keep going when the ladder stops making progress.
///
/// A repeat counter is not enough: the failure this catches in practice was
/// two *different* actions alternating -- `FreezeEntitlement` recreating the
/// pot that `CloseGeneralPot` had just removed -- which never repeats the
/// same action twice in a row and would spin forever under an equality
/// check.  Seeing one action twice inside a short window is the signal.
fn guard_progress(recent: &mut Vec<String>, act: &crank::Act) -> Res<()> {
    let key = format!("{}|{}", act.name, act.detail);
    if recent.iter().filter(|seen| **seen == key).count() >= 2 {
        return Err(format!(
            "{} ({}) came round for the third time in {PROGRESS_WINDOW} actions; the ladder is \
             cycling rather than progressing and the keeper refuses to spin. Recent: {}",
            act.name,
            act.detail,
            recent.join(" -> ")
        ));
    }
    recent.push(key);
    if recent.len() > PROGRESS_WINDOW {
        recent.remove(0);
    }
    Ok(())
}

/// The program's own log lines, folded into one diagnosable string.
fn program_logs(rpc: &Rpc, signature: &str) -> String {
    let lines = rpc.logs(signature);
    if lines.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n  program log:");
    for line in lines {
        out.push_str("\n    ");
        out.push_str(&line);
    }
    out
}

/// Check the endpoint answers, and top the payer up when asked to.
///
/// A cranker that cannot pay rent is not a cranker.  The floor is asked for
/// explicitly (`--fund`) so a keeper never quietly drains a faucet.
fn ensure_ready(rpc: &Rpc, keeper: &Keeper, flags: &Flags) -> Res<()> {
    if !rpc.healthy() {
        return Err("the loopback endpoint is not answering getHealth".to_string());
    }
    let Some(floor) = flags.optional("fund") else {
        return Ok(());
    };
    let floor: u64 = floor
        .parse()
        .map_err(|_| "--fund must be a lamport amount".to_string())?;
    let held = rpc.lamports(&keeper.payer_address())?;
    if held < floor {
        rpc.airdrop(&keeper.payer_address(), floor - held)?;
        println!(
            "keeper funded payer={} from={held} to={floor}",
            keeper.payer_address()
        );
    }
    Ok(())
}

/// Submit one action, classify its outcome, and print the one honest line.
fn take(rpc: &Rpc, act: &crank::Act) -> Res<()> {
    let confirmation = rpc.submit_and_confirm(&act.transaction)?;
    let quote = &act.quote;
    let charged = confirmation.compute_units;
    let headroom = charged.map(|used| i64::from(quote.limit_cu) - i64::try_from(used).unwrap_or(0));
    let (result, note) = if confirmation.accepted() {
        ("accepted".to_string(), String::new())
    } else if let Some(code) = confirmation.custom_code {
        if act.benign.contains(&code) {
            (
                "already-done".to_string(),
                format!(
                    " code=0x{code:04x} because=\"{}\"",
                    crank::benign_reason(code)
                ),
            )
        } else {
            return Err(format!(
                "{} ({}) refused with an unclassified code 0x{code:04x}: {}{}",
                act.name,
                act.detail,
                confirmation.error,
                program_logs(rpc, &confirmation.signature)
            ));
        }
    } else {
        return Err(format!(
            "{} ({}) failed without a custom code: {}{}",
            act.name,
            act.detail,
            confirmation.error,
            program_logs(rpc, &confirmation.signature)
        ));
    };
    println!(
        "slot={} action={} {} authority={} quote={} route={} limit_cu={} cu={} headroom_cu={} \
         ledgers={} bytes={} result={}{} sig={}",
        confirmation.slot,
        act.name,
        act.detail,
        if act.permissionless {
            "permissionless"
        } else {
            "owner-signed"
        },
        if quote.quoted() { "W1" } else { "UNQUOTED" },
        quote.route,
        quote.limit_cu,
        charged.map_or_else(|| "-".to_string(), |used| used.to_string()),
        headroom.map_or_else(|| "-".to_string(), |value| value.to_string()),
        quote.ledgers,
        act.transaction.len(),
        result,
        note,
        confirmation.signature
    );
    Ok(())
}

// --- prime ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Plan {
    steps: Vec<PlanStep>,
}

#[derive(Debug, Deserialize)]
struct PlanStep {
    name: String,
    tx: String,
    kind: String,
    #[serde(default)]
    expect_code: Option<u64>,
    #[serde(default)]
    wait_slot: Option<u64>,
}

fn prime(flags: &Flags) -> Res<()> {
    let rpc = Rpc::new(&flags.one("url")?)?;
    let plan_dir = PathBuf::from(flags.one("plan")?);
    let keys = load_keys(flags.all("key"))?;
    if keys.is_empty() {
        return Err("prime needs at least one --key".to_string());
    }
    let plan: Plan = serde_json::from_slice(
        &std::fs::read(plan_dir.join("committed.json"))
            .map_err(|error| format!("could not read the plan: {error}"))?,
    )
    .map_err(|error| format!("the plan did not parse: {error}"))?;

    let (first, last) = match flags.optional("steps") {
        Some(range) => {
            let (low, high) = range
                .split_once('-')
                .ok_or_else(|| format!("--steps wants A-B, got {range}"))?;
            (
                low.parse::<usize>().map_err(|_| "--steps A is not a number")?,
                high.parse::<usize>()
                    .map_err(|_| "--steps B is not a number")?,
            )
        }
        None => (1, plan.steps.len()),
    };
    let skip: Vec<&str> = flags
        .all("skip")
        .iter()
        .map(String::as_str)
        .collect();

    let refs: Vec<&Keypair> = keys.iter().collect();
    let mut submitted = 0_u32;
    for (ordinal, step) in plan.steps.iter().enumerate() {
        let number = ordinal + 1;
        if number < first || number > last {
            continue;
        }
        if skip.iter().any(|name| step.name.contains(name)) {
            println!("prime skip step={} name={}", number, step.name);
            continue;
        }
        if let Some(target) = step.wait_slot {
            wait_for_slot(&rpc, target)?;
        }
        let encoded = std::fs::read_to_string(plan_dir.join(&step.tx))
            .map_err(|error| format!("{}: could not read {}: {error}", step.name, step.tx))?;
        let mut transaction = BASE64
            .decode(encoded.trim())
            .map_err(|error| format!("{}: transaction did not decode: {error}", step.name))?;
        wire::set_blockhash(&mut transaction, &rpc.blockhash()?)?;
        wire::sign(&mut transaction, &refs)?;
        let confirmation = rpc.submit_and_confirm(&transaction)?;
        match step.kind.as_str() {
            "accept" if confirmation.accepted() => {}
            "accept" => {
                return Err(format!(
                    "{}: expected success, got {}",
                    step.name, confirmation.error
                ))
            }
            "refuse" => {
                let expected = step
                    .expect_code
                    .ok_or_else(|| format!("{}: refusal has no expect_code", step.name))?;
                if confirmation.custom_code != Some(expected) {
                    return Err(format!(
                        "{}: expected Custom({expected:#06x}), got {}",
                        step.name, confirmation.error
                    ));
                }
            }
            other => return Err(format!("{}: prime does not drive step kind {other}", step.name)),
        }
        println!(
            "prime step={number} name={} kind={} slot={} cu={} sig={}",
            step.name,
            step.kind,
            confirmation.slot,
            confirmation
                .compute_units
                .map_or_else(|| "-".to_string(), |used| used.to_string()),
            confirmation.signature
        );
        submitted += 1;
    }
    println!("prime done submitted={submitted} range={first}-{last}");
    Ok(())
}

fn wait_for_slot(rpc: &Rpc, target: u64) -> Res<()> {
    let mut now = rpc.slot()?;
    while now < target {
        thread::sleep(Duration::from_millis(250));
        now = rpc.slot()?;
    }
    Ok(())
}

// --- addresses -----------------------------------------------------------

fn addresses(flags: &Flags) -> Res<()> {
    let cfg = config(flags)?;
    let epoch_id = clutch_solana_layout::canonical_epoch_id(cfg.market, cfg.epoch_index);
    let mut deriver = pda::Deriver::new(&cfg.program_id_b58);
    let market_account = deriver.find(&[
        clutch_sbf::seeds::SEED_MARKET,
        &cfg.realm.bytes(),
        &cfg.market.bytes(),
    ])?;
    let epoch = deriver.epoch(cfg.market, cfg.epoch_index)?;
    let window = deriver.window(cfg.market, cfg.epoch_index)?;
    let page = deriver.page(epoch_id, 0)?;
    let pot = deriver.pot(epoch_id)?;
    let policy = deriver.find(&[
        clutch_sbf::seeds::SEED_BATCH_POLICY,
        &epoch_id.bytes(),
        &cfg.policy.bytes(),
    ])?;
    println!(
        "{{\"epoch_id\":\"{}\",\"policy_digest\":\"{}\",\"market_account\":\"{}\",\
         \"epoch\":\"{}\",\"window\":\"{}\",\"page0\":\"{}\",\"pot\":\"{}\",\
         \"policy_artifact\":\"{}\"}}",
        wire::base58(&epoch_id.bytes()),
        wire::base58(&cfg.policy.bytes()),
        market_account.address,
        epoch.address,
        window.address,
        page.address,
        pot.address,
        policy.address
    );
    Ok(())
}

// --- fold-wire-probe -----------------------------------------------------

fn fold_wire_probe(flags: &Flags) -> Res<()> {
    let widths: Vec<u8> = match flags.optional("widths") {
        Some(text) => text
            .split(',')
            .map(|part| {
                part.trim()
                    .parse::<u8>()
                    .map_err(|_| format!("--widths wants numbers, got {part}"))
            })
            .collect::<Res<Vec<u8>>>()?,
        None => (1..=13).collect(),
    };
    let rpc = match flags.optional("url") {
        Some(url) => Some(Rpc::new(&url)?),
        None => None,
    };
    let plane = probe::PlaneIds {
        program_id_b58: flags.one("program")?,
        realm: hash_arg(flags, "realm")?,
        market: hash_arg(flags, "market")?,
        feed: hash_arg(flags, "feed")?,
        window: hash_arg(flags, "window")?,
        terms: hash_arg(flags, "terms")?,
    };
    let answer = probe::run(&plane, &widths, rpc.as_ref())?;
    println!(
        "fold-wire-probe packet_budget_bytes={} base_bytes={} per_fold_bytes={}",
        wire::PACKET_BUDGET_BYTES,
        answer.base_bytes,
        answer.per_fold_bytes
    );
    for row in &answer.rows {
        println!(
            "folds={} bytes={} fits_packet={} admits_on_compute={} limit_cu={} quote={} transport={}",
            row.folds,
            row.bytes,
            row.fits,
            row.admits_on_compute,
            row.limit_cu,
            if row.quoted { "W1" } else { "DERIVED" },
            match row.transport {
                Some(probe::Transport::Admitted) => "admitted",
                Some(probe::Transport::Refused) => "refused",
                None => "not-asked",
            }
        );
    }
    println!(
        "fold_wire_answer largest_fitting_folds={} sealed_plan_width=12 sealed_plan_fits={}",
        answer.largest_fitting,
        answer
            .rows
            .iter()
            .find(|row| row.folds == 12)
            .map_or_else(|| "unmeasured".to_string(), |row| row.fits.to_string())
    );
    let plan = answer.plan;
    println!(
        "fold_wire_plan instructions_per_packet={} records_per_instruction={} \
         records_per_packet={} transactions_for_32_records={} packet_cu={} packet_limit_cu={}",
        plan.instructions_per_packet,
        plan.records_per_instruction,
        plan.records_per_packet,
        plan.transactions_for_max_work,
        plan.packet_cu,
        plan.packet_limit_cu
            .map_or_else(|| "STOP_HEADROOM".to_string(), |limit| limit.to_string())
    );
    Ok(())
}

/// Guard: the binary must never be handed a keypair it then points at a
/// cluster.  [`Rpc::new`] enforces the loopback rule; this only keeps the
/// intent visible where a reader looks for it.
#[allow(dead_code)]
fn loopback_only(path: &Path) -> bool {
    path.exists()
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_keypair::Signer;

    #[test]
    fn flags_accumulate_repeats_and_recognize_bare_switches() {
        let args: Vec<String> = ["--owner", "a.json", "--owner", "b.json", "--open"]
            .iter()
            .map(|value| (*value).to_string())
            .collect();
        let flags = Flags::parse(&args);
        assert_eq!(flags.all("owner"), ["a.json".to_string(), "b.json".to_string()]);
        assert!(flags.present("open"));
        assert!(!flags.present("closed"));
        assert!(flags.one("owner").is_ok());
        assert!(flags.one("open").is_err(), "a bare switch has no value");
    }

    #[test]
    fn the_default_policy_is_the_frozen_general_clearing_digest() {
        let flags = Flags::parse(&[]);
        let digest = policy_digest(&flags).expect("the frozen policy digests");
        assert_ne!(digest, Hash32::ZERO);
        // And an explicit override wins.
        let explicit = vec!["--policy".to_string(), wire::base58(&[7; 32])];
        let flags = Flags::parse(&explicit);
        assert_eq!(
            policy_digest(&flags).expect("an explicit digest parses"),
            Hash32::from_bytes([7; 32])
        );
    }

    #[test]
    fn a_keeper_needs_at_least_its_own_payer() {
        let rpc = Rpc::new("http://127.0.0.1:9000").expect("loopback");
        let cfg = Config {
            program_id_b58: wire::base58(&[1; 32]),
            program_id: [1; 32],
            realm: Hash32::from_bytes([2; 32]),
            market: Hash32::from_bytes([3; 32]),
            epoch_index: 1,
            policy: Hash32::from_bytes([4; 32]),
            freeze_deadline_slots: 100,
            may_open: false,
        };
        assert!(Keeper::new(cfg.clone(), rpc.clone(), Vec::new()).is_err());
        assert!(Keeper::new(cfg, rpc, vec![Keypair::new()]).is_ok());
    }

    #[test]
    fn the_payer_address_is_the_first_key() {
        let rpc = Rpc::new("http://127.0.0.1:9000").expect("loopback");
        let payer = Keypair::new();
        let expected = payer.pubkey().to_string();
        let cfg = Config {
            program_id_b58: wire::base58(&[1; 32]),
            program_id: [1; 32],
            realm: Hash32::from_bytes([2; 32]),
            market: Hash32::from_bytes([3; 32]),
            epoch_index: 1,
            policy: Hash32::from_bytes([4; 32]),
            freeze_deadline_slots: 100,
            may_open: false,
        };
        let keeper = Keeper::new(cfg, rpc, vec![payer, Keypair::new()]).expect("binds");
        assert_eq!(keeper.payer_address(), expected);
    }
}
