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
    let (_, observed) =
        rpc.finalized_observed_accounts(&addresses, route.minimum_finalized_slot())?;
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
