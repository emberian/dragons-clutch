//! Local-successor adapter for the shared read-only General V5 plan producer.
//!
//! Route grammar, snapshot projection, request derivation, v0 compilation and
//! output serialization all belong to `dclutch-general-successor-operator`.
//! This module contributes only this binary's cluster-origin policy, RPC
//! transport and command-line spelling.

use std::path::PathBuf;

use dclutch_general_successor_operator as shared;

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    rpc::{Rpc, WritePolicyV1},
};

pub(crate) const COMMAND_V1: &str = shared::COMMAND_V1;

#[derive(Clone, Debug)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    route: PathBuf,
    output: PathBuf,
}

pub(crate) fn usage() -> String {
    format!(
        "dclutch-local-successor-bootstrap {COMMAND_V1} \
         --rpc-url URL [{DEVNET_ACKNOWLEDGMENT_FLAG} DEVNET_GENESIS] \
         --route ABSOLUTE_JSON --output ABSOLUTE_NEW_JSON\n\
         Read-only: reacquires every untrusted General route address, artifact carrier, state, and \
         lookup table in one finalized snapshot; derives the request and unsigned v0 packet from \
         authenticated current state. It reads no keypair and cannot sign, simulate, submit, or write."
    )
}

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments_v1(arguments)?;
    let route_bytes = shared::read_bounded_route_file_v1(&arguments.route).map_err(lift)?;
    let route = shared::parse_route_v1(&route_bytes).map_err(lift)?;
    let addresses = route.snapshot_addresses().map_err(lift)?;
    let mut rpc = Rpc::connect_cluster(&arguments.origin, WritePolicyV1::ReadsOnly)?;
    // A VACANT STAGING CURSOR IS PART OF THE FRAME, NOT A MISSING ACCOUNT.
    //
    // Nineteen of the thirty-nine fixed coordinates are staging cursors that a
    // closed publication ladder leaves System-owned with zero data, and
    // `getMultipleAccounts` reports every one of them as null. The strict
    // snapshot refused the first of them, so no route to a real General market
    // could ever have reached the producer.
    let (_, observed) = rpc
        .finalized_observed_accounts_admitting_vacant(&addresses, route.minimum_finalized_slot())?;
    let recent_blockhash = rpc.latest_finalized_blockhash()?;
    let document = shared::produce_plan_v5(&route, observed, recent_blockhash).map_err(lift)?;
    shared::write_new_plan_v5(&arguments.output, &document).map_err(lift)
}

fn lift(error: shared::Error) -> Error {
    Error::new(error.to_string())
}

fn parse_arguments_v1(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut route = None;
    let mut output = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--route" => &mut route,
            "--output" => &mut output,
            _ => {
                return Err(Error::new(format!(
                    "unknown {COMMAND_V1} argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let rpc_url = rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?;
    let origin = ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?;
    Ok(ArgumentsV1 {
        origin,
        route: absolute_path_v1(route, "--route")?,
        output: absolute_path_v1(output, "--output")?,
    })
}

fn absolute_path_v1(value: Option<String>, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value.ok_or_else(|| Error::new(format!("{label} is required")))?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_requires_exact_absolute_paths_and_read_only_cluster_arguments() {
        let error = parse_arguments_v1(vec![
            "--rpc-url".into(),
            "http://127.0.0.1:8899".into(),
            "--route".into(),
            "relative.json".into(),
            "--output".into(),
            "/tmp/general-plan.json".into(),
        ])
        .expect_err("relative route");
        assert!(error.to_string().contains("--route must be absolute"));

        let error = parse_arguments_v1(vec![
            "--rpc-url".into(),
            "http://127.0.0.1:8899".into(),
            "--route".into(),
            "/tmp/route.json".into(),
            "--output".into(),
            "/tmp/plan.json".into(),
            "--keypair".into(),
            "/tmp/key.json".into(),
        ])
        .expect_err("key-bearing argument");
        assert!(error.to_string().contains("unknown"));
    }
}

// ---------------------------------------------------------------------------
// The table the plan producer requires and nothing in this tree ever built.
// ---------------------------------------------------------------------------

/// Publish the frozen routing table one General route's own instruction needs.
pub(crate) const DEVNET_LOOKUP_TABLE_COMMAND_V1: &str = "devnet-general-lookup-table-v1";

pub(crate) fn lookup_table_usage() -> String {
    format!(
        "dclutch-local-successor-bootstrap {DEVNET_LOOKUP_TABLE_COMMAND_V1} \
         --rpc-url URL {DEVNET_ACKNOWLEDGMENT_FLAG} GENESIS_HASH \
         --route ABSOLUTE_JSON --evidence ABSOLUTE_NEW_JSON \
         [--payer-keypair ABSOLUTE_JSON --execute]\n     \
         Computes the exact sorted deduplicated address set \
         compile_general_hot_v0 requires of this route's lookup table, and \
         publishes one frozen table holding exactly it. Without --execute this \
         opens no key, sends nothing, and prints the set. The route's own payer \
         signs, because the set is relative to that payer."
    )
}

struct LookupArgumentsV1 {
    origin: ClusterOriginV1,
    route: PathBuf,
    payer_keypair: Option<PathBuf>,
    evidence: PathBuf,
    execute: bool,
}

pub(crate) fn run_lookup_table_devnet(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_lookup_arguments_v1(arguments)?;
    if arguments.evidence.exists() {
        return Err(Error::new(format!(
            "refusing to overwrite {}",
            arguments.evidence.display()
        )));
    }
    crate::cluster::ExpectedClusterV1::Devnet.authenticate(&arguments.origin)?;
    let route_bytes = shared::read_bounded_route_file_v1(&arguments.route).map_err(lift)?;
    let route = shared::parse_route_v1(&route_bytes).map_err(lift)?;
    let payer = shared::route_payer_v1(&route);
    let addresses = route.snapshot_addresses().map_err(lift)?;
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&arguments.origin, policy)?;
    // The same vacant-tolerant snapshot the plan producer takes, and for the
    // same reason: nineteen of these addresses are staging cursors a closed
    // publication ladder left System-owned with zero data.
    let (_, observed) = rpc
        .finalized_observed_accounts_admitting_vacant(&addresses, route.minimum_finalized_slot())?;
    let required = shared::canonical_lookup_addresses_v1(&route, observed).map_err(lift)?;
    println!("route payer          {payer}");
    println!("snapshot accounts    {}", addresses.len());
    println!("table addresses      {}", required.len());
    let mut evidence = serde_json::json!({
        "schema": "dclutch-devnet-general-lookup-table-evidence-v1",
        "cluster": "devnet",
        "rpcUrl": arguments.origin.redacted_url(),
        "route": arguments.route.display().to_string(),
        "payer": payer.to_string(),
        "addressCount": required.len(),
        "addresses": required.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "executed": arguments.execute,
    });
    if !arguments.execute {
        std::fs::write(
            &arguments.evidence,
            format!("{}\n", serde_json::to_string_pretty(&evidence)?),
        )
        .map_err(|error| Error::new(format!("lookup table evidence: {error}")))?;
        println!("dry run; no key was opened and nothing was sent");
        return Ok(());
    }
    let path = arguments
        .payer_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --payer-keypair"))?;
    let signer = solana_sdk::signature::Keypair::new_from_array(crate::campaign::read_keypair_file(
        path,
        "General lookup table payer",
    )?);
    // THE ROUTE'S PAYER IS THE ONE THAT MAY SIGN THIS.
    //
    // `canonical_general_lookup_addresses_v3` excludes the payer and every
    // instruction signer from the set, so a table published against a
    // different payer holds a different list -- one extra address -- and would
    // refuse at the compiler for a reason that names the table rather than the
    // key that built it.
    if solana_sdk::signer::Signer::pubkey(&signer) != payer {
        return Err(Error::new(format!(
            "the route names payer {payer} and the keypair holds {}; the address set is \
             relative to the payer",
            solana_sdk::signer::Signer::pubkey(&signer)
        )));
    }
    let mut transactions = Vec::new();
    let (_, tables) = crate::market::publish_routing_table_over_v1(
        &mut rpc,
        &signer,
        "GENERAL-HOT",
        &required,
        &mut transactions,
    )?;
    let table = tables
        .first()
        .ok_or_else(|| Error::new("the publication omitted its table"))?;
    println!("lookup table         {}", table.key);
    evidence["lookupTable"] = serde_json::json!(table.key.to_string());
    evidence["transactions"] = serde_json::json!(
        transactions
            .iter()
            .map(|value| serde_json::json!({
                "label": value.label,
                "signature": value.signature,
                "slot": value.slot,
            }))
            .collect::<Vec<_>>()
    );
    std::fs::write(
        &arguments.evidence,
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )
    .map_err(|error| Error::new(format!("lookup table evidence: {error}")))?;
    Ok(())
}

fn parse_lookup_arguments_v1(arguments: Vec<String>) -> Result<LookupArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut route = None;
    let mut payer_keypair = None;
    let mut evidence = None;
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        if flag == "--execute" {
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{flag}; usage: {}", lookup_table_usage())))?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--route" => &mut route,
            "--payer-keypair" => &mut payer_keypair,
            "--evidence" => &mut evidence,
            other => return Err(Error::new(format!("unknown flag {other}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("repeated flag {flag}")));
        }
    }
    let rpc_url = rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?;
    let acknowledgment =
        acknowledgment.ok_or_else(|| Error::new("--i-mean-devnet GENESIS_HASH is required"))?;
    Ok(LookupArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, Some(&acknowledgment))?,
        route: absolute_path_v1(route, "--route")?,
        payer_keypair: payer_keypair.map(PathBuf::from),
        evidence: absolute_path_v1(evidence, "--evidence")?,
        execute,
    })
}

// ---------------------------------------------------------------------------
// The signer the plan document never had.
// ---------------------------------------------------------------------------

/// Produce, sign, simulate and submit one General successor transaction.
pub(crate) const DEVNET_EXECUTE_COMMAND_V1: &str = "devnet-general-successor-execute-v1";

pub(crate) fn execute_usage() -> String {
    format!(
        "dclutch-local-successor-bootstrap {DEVNET_EXECUTE_COMMAND_V1} \
         --rpc-url URL {DEVNET_ACKNOWLEDGMENT_FLAG} GENESIS_HASH \
         --route ABSOLUTE_JSON --plan-output ABSOLUTE_NEW_JSON \
         --evidence ABSOLUTE_NEW_JSON [--payer-keypair ABSOLUTE_JSON --execute]\n     \
         Produces the plan and signs the exact bytes it published. The plan is \
         produced HERE rather than read from a file because its compiled \
         message carries a recent blockhash that dies in about ninety seconds; \
         a produce-then-sign across two invocations races that clock. Without \
         --execute the plan is written and simulated and nothing is sent."
    )
}

struct ExecuteArgumentsV1 {
    origin: ClusterOriginV1,
    route: PathBuf,
    plan_output: PathBuf,
    payer_keypair: Option<PathBuf>,
    evidence: PathBuf,
    execute: bool,
}

pub(crate) fn run_execute_devnet(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_execute_arguments_v1(arguments)?;
    for path in [&arguments.plan_output, &arguments.evidence] {
        if path.exists() {
            return Err(Error::new(format!(
                "refusing to overwrite {}",
                path.display()
            )));
        }
    }
    crate::cluster::ExpectedClusterV1::Devnet.authenticate(&arguments.origin)?;
    let route_bytes = shared::read_bounded_route_file_v1(&arguments.route).map_err(lift)?;
    let route = shared::parse_route_v1(&route_bytes).map_err(lift)?;
    let payer_key = shared::route_payer_v1(&route);
    let addresses = route.snapshot_addresses().map_err(lift)?;
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&arguments.origin, policy)?;
    let (_, observed) = rpc
        .finalized_observed_accounts_admitting_vacant(&addresses, route.minimum_finalized_slot())?;
    let (blockhash, last_valid_block_height) = rpc.recent_blockhash_with_height_v1()?;
    let document = shared::produce_plan_v5(&route, observed, blockhash).map_err(lift)?;
    shared::write_new_plan_v5(&arguments.plan_output, &document).map_err(lift)?;
    let plan: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&arguments.plan_output)?)
            .map_err(|error| Error::new(format!("plan document: {error}")))?;
    let encoded = plan
        .get("transactionBase64")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new("the plan document carries no transactionBase64"))?;
    let unsigned_packet = base64_decode_v1(encoded)?;
    let unsigned: solana_sdk::transaction::VersionedTransaction =
        bincode::deserialize(&unsigned_packet)
            .map_err(|error| Error::new(format!("plan transaction: {error}")))?;
    println!("plan                 {}", arguments.plan_output.display());
    println!("payer                {payer_key}");
    println!("required signers     {}", unsigned.signatures.len());
    println!("lookup table         {}", plan["lookupTable"]);
    println!("heap frame bytes     {}", plan["heapFrameBytes"]);
    println!("observed slot        {}", plan["observedSlot"]);

    // SIMULATION RUNS ON THE UNSIGNED BYTES, WITH sigVerify OFF.
    //
    // The point of simulating here is to learn what the RUNTIME says about this
    // exact message before a signature exists, so a refusal is attributable to
    // the plan rather than to the key that signed it.
    let simulation = rpc.simulate_versioned_v1("General successor", &unsigned_packet)?;
    let simulated_units = simulation
        .get("value")
        .and_then(|value| value.get("unitsConsumed"))
        .and_then(serde_json::Value::as_u64);
    let simulated_error = simulation
        .get("value")
        .and_then(|value| value.get("err"))
        .cloned()
        .filter(|value| !value.is_null());
    println!(
        "simulation           units {}, err {}",
        simulated_units.map_or_else(|| "unreported".to_owned(), |value| value.to_string()),
        simulated_error
            .as_ref()
            .map_or_else(|| "none".to_owned(), ToString::to_string)
    );
    let mut evidence = serde_json::json!({
        "schema": "dclutch-devnet-general-successor-execute-evidence-v1",
        "cluster": "devnet",
        "rpcUrl": arguments.origin.redacted_url(),
        "route": arguments.route.display().to_string(),
        "plan": arguments.plan_output.display().to_string(),
        "market": plan.get("market"),
        "action": plan.get("action"),
        "payer": payer_key.to_string(),
        "lookupTable": plan.get("lookupTable"),
        "observedSlot": plan.get("observedSlot"),
        "admittedInvocationCount": plan.get("admittedInvocationCount"),
        "heapFrameBytes": plan.get("heapFrameBytes"),
        "lifecycle": plan.get("lifecycle"),
        "simulatedUnitsConsumed": simulated_units,
        "simulatedError": simulated_error,
        "executed": arguments.execute,
    });
    let write_evidence = |value: &serde_json::Value| -> Result<()> {
        std::fs::write(
            &arguments.evidence,
            format!("{}\n", serde_json::to_string_pretty(value)?),
        )
        .map_err(|error| Error::new(format!("execute evidence: {error}")))
    };
    if !arguments.execute {
        write_evidence(&evidence)?;
        println!("dry run; no key was opened and nothing was sent");
        return Ok(());
    }
    let path = arguments
        .payer_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --payer-keypair"))?;
    let signer = solana_sdk::signature::Keypair::new_from_array(crate::campaign::read_keypair_file(
        path,
        "General successor payer",
    )?);
    if solana_sdk::signer::Signer::pubkey(&signer) != payer_key {
        return Err(Error::new(format!(
            "the route names payer {payer_key} and the keypair holds {}",
            solana_sdk::signer::Signer::pubkey(&signer)
        )));
    }
    // The plan's own message, signed. Nothing is recompiled: `try_new` signs
    // exactly the message the producer published, and a signer set that does
    // not cover it refuses here rather than on chain.
    let signed = solana_sdk::transaction::VersionedTransaction::try_new(
        unsigned.message.clone(),
        &[&signer],
    )
    .map_err(|error| Error::new(format!("signing the plan's own message: {error}")))?;
    let sent = rpc.submit_and_confirm_versioned_v1(
        "General OpenBatch successor",
        &signed,
        last_valid_block_height,
    )?;
    if let Some(error) = sent.error.as_ref() {
        evidence["signature"] = serde_json::json!(sent.signature);
        evidence["onChainError"] = error.clone();
        write_evidence(&evidence)?;
        return Err(Error::new(format!(
            "the General successor refused on chain: {error} (signature {})",
            sent.signature
        )));
    }
    println!("signature            {}", sent.signature);
    println!("slot                 {}", sent.slot);
    println!(
        "compute units        {}",
        sent.compute_units_consumed
            .map_or_else(|| "unreported".to_owned(), |value| value.to_string())
    );
    evidence["signature"] = serde_json::json!(sent.signature);
    evidence["slot"] = serde_json::json!(sent.slot);
    evidence["computeUnitsConsumed"] = serde_json::json!(sent.compute_units_consumed);
    evidence["feeLamports"] = serde_json::json!(sent.fee_lamports);
    evidence["logs"] = serde_json::json!(sent.logs);
    // THE BATCH STATE IS READ BACK, NOT INFERRED FROM THE SEND.
    let state_address = plan
        .get("lifecycle")
        .and_then(|value| value.get("primary"))
        .and_then(|value| value.get("account"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| Error::new("the plan states no primary lifecycle account"))?
        .parse::<solana_program::pubkey::Pubkey>()
        .map_err(|error| Error::new(format!("primary lifecycle account: {error}")))?;
    let state = rpc.account(state_address)?.ok_or_else(|| {
        Error::new(format!(
            "the transaction landed and the batch state {state_address} is still absent"
        ))
    })?;
    println!(
        "batch state          {state_address}: {} bytes, owner {}, {} lamports",
        state.data.len(),
        state.owner,
        state.lamports
    );
    evidence["batchState"] = serde_json::json!({
        "address": state_address.to_string(),
        "bytes": state.data.len(),
        "owner": state.owner.to_string(),
        "lamports": state.lamports,
        "magic": String::from_utf8_lossy(state.data.get(..8).unwrap_or_default()).to_string(),
    });
    write_evidence(&evidence)
}

fn base64_decode_v1(value: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(value)
        .map_err(|error| Error::new(format!("plan transaction base64: {error}")))
}

fn parse_execute_arguments_v1(arguments: Vec<String>) -> Result<ExecuteArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut route = None;
    let mut plan_output = None;
    let mut payer_keypair = None;
    let mut evidence = None;
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        if flag == "--execute" {
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{flag}; usage: {}", execute_usage())))?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--route" => &mut route,
            "--plan-output" => &mut plan_output,
            "--payer-keypair" => &mut payer_keypair,
            "--evidence" => &mut evidence,
            other => return Err(Error::new(format!("unknown flag {other}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("repeated flag {flag}")));
        }
    }
    let rpc_url = rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?;
    let acknowledgment =
        acknowledgment.ok_or_else(|| Error::new("--i-mean-devnet GENESIS_HASH is required"))?;
    Ok(ExecuteArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, Some(&acknowledgment))?,
        route: absolute_path_v1(route, "--route")?,
        plan_output: absolute_path_v1(plan_output, "--plan-output")?,
        payer_keypair: payer_keypair.map(PathBuf::from),
        evidence: absolute_path_v1(evidence, "--evidence")?,
        execute,
    })
}
