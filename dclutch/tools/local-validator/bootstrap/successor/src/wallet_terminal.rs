//! The wallet-terminal payout SHELL: arguments, files, RPC, and stdout.
//!
//! The derivation this used to carry moved to
//! `dclutch-wallet-terminal-payout-operator` so a browser could compile and
//! run the same code instead of reimplementing it. Nothing about this
//! binary's own path changed: it still parses the same arguments, reads the
//! same file, connects the same RPC, and prints the same JSON. What is gone
//! from this file is only the part that never needed a socket.

use std::{io::Write, path::PathBuf};

use dclutch_wallet_terminal_payout_operator::ObservedAccountValueV1;

// Every item the derivation used to define here is re-exported at its old
// path, so `crate::wallet_terminal::X` still resolves for the modules that
// were reaching it. The move changed where the code lives, not what this
// binary's own modules may call it.
pub(crate) use dclutch_wallet_terminal_payout_operator::wire::*;
use serde::Serialize;

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    rpc::{Rpc, WritePolicyV1},
};

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let (origin, _source, decoded) = command_input(arguments, "wallet-terminal-payout-plan")?;
    let selected = SelectedInputV1::parse(&decoded, LookupTableRequirementV1::Present)?;
    let snapshot = finalized_snapshot(&origin, &selected)?;
    stdout_json(&build_manifest(&selected, &snapshot)?)
}

pub(crate) fn run_alt(arguments: Vec<String>) -> Result<()> {
    let (origin, source, decoded) = command_input(arguments, "wallet-terminal-payout-alt-plan")?;
    let selected = SelectedInputV1::parse(&decoded, LookupTableRequirementV1::Absent)?;
    let snapshot = finalized_snapshot(&origin, &selected)?;
    let report = build_report(&selected, &snapshot)?;
    stdout_json(&build_alt_plan(decoded, &source, &report)?)
}

fn command_input(
    arguments: Vec<String>,
    command: &str,
) -> Result<(ClusterOriginV1, Vec<u8>, PlanInputV1)> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut input = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--input" => &mut input,
            _ => {
                return Err(Error::new(format!(
                    "unknown {command} argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let rpc_url = rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?;
    let origin = ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?;
    let input_path = absolute(input, "--input")?;
    let source = std::fs::read(input_path)?;
    let decoded: PlanInputV1 = serde_json::from_slice(&source)?;
    Ok((origin, source, decoded))
}

fn finalized_snapshot(
    origin: &ClusterOriginV1,
    selected: &SelectedInputV1,
) -> Result<FinalizedSnapshotV1> {
    let addresses = selected.addresses();
    let mut rpc = Rpc::connect_cluster(origin, WritePolicyV1::ReadsOnly)?;
    let floor = rpc.finalized_slot()?;
    let (slot, values) = rpc.finalized_accounts(&addresses, floor)?;
    snapshot_from_rpc(slot, rpc.block_time(slot)?, &addresses, values)
}

fn stdout_json(value: &impl Serialize) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap wallet-terminal-payout-alt-plan --rpc-url URL \\
     [--i-mean-devnet DEVNET_GENESIS_HASH] --input ABSOLUTE_JSON\n  \
     dclutch-local-successor-bootstrap wallet-terminal-payout-plan --rpc-url URL \\
     [--i-mean-devnet DEVNET_GENESIS_HASH] --input ABSOLUTE_JSON\n\nThese commands are read-only. \
     Each reauthenticates one exact Market, Product/composition graph, current \
     Claims/Core/Custody deployments, wallet Position, Custody and token prestates at one finalized \
     account observation. The first emits the owner-authorized create and ordered extensions for \
     this payout's canonical lookup table. After finalization, the second verifies that table and \
     emits the exact payout manifest the SDK and web app can execute. Mainnet-beta is refused \
     unconditionally."
}

fn absolute(value: Option<String>, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value.ok_or_else(|| Error::new(format!("{label} is required")))?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

/// The old `FinalizedSnapshotV1::from_rpc`, kept here because it is the one
/// place this binary's RPC type meets the extracted derivation.
///
/// The crate takes observed VALUES rather than this binary's `RpcAccount`,
/// which is what removes its last tie to a socket. Mapping the four fields is
/// the whole of the adaptation.
pub(crate) fn snapshot_from_rpc(
    slot: u64,
    unix_timestamp: i64,
    keys: &[solana_program::pubkey::Pubkey],
    values: Vec<Option<crate::rpc::RpcAccount>>,
) -> Result<FinalizedSnapshotV1> {
    let observed = values
        .into_iter()
        .map(|value| {
            value.map(|account| ObservedAccountValueV1 {
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            })
        })
        .collect();
    Ok(FinalizedSnapshotV1::from_observed(
        slot,
        unix_timestamp,
        keys,
        observed,
    )?)
}
