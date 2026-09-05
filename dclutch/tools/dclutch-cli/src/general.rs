//! Read-only General successor planning for an external wallet.
//!
//! The route file is untrusted routing input. The shared General producer owns
//! its grammar, reacquires every named account in one finalized snapshot,
//! derives the canonical request and lifecycle from authenticated state, and
//! compiles an unsigned v0 transaction. This adapter contributes only the
//! public `dclutch general plan` spelling and RPC transport.

use std::path::PathBuf;

use dclutch_operator::general_successor as producer;

use crate::{DEFAULT_RPC_URL_V1, Error, RPC_URL_ENV_V1, Result, rpc};

/// Run the complete General command family.
pub fn run(arguments: Vec<String>) -> Result<()> {
    let (command, rest) = arguments.split_first().ok_or_else(|| Error::new(usage()))?;
    match command.as_str() {
        "plan" => plan(rest.to_vec()),
        "help" | "-h" | "--help" => {
            print!("{}", usage());
            Ok(())
        }
        other => Err(Error::new(format!(
            "unknown General command `{other}`. Run `dclutch general --help`."
        ))),
    }
}

fn plan(arguments: Vec<String>) -> Result<()> {
    let arguments = PlanArgumentsV1::parse(arguments)?;
    let route_bytes = producer::read_bounded_route_file_v1(&arguments.route).map_err(lift)?;
    let route = producer::parse_route_v1(&route_bytes).map_err(lift)?;
    let addresses = route.snapshot_addresses().map_err(lift)?;
    let observed = rpc::fetch_observed_accounts_v1(
        &arguments.rpc,
        &addresses,
        route.minimum_finalized_slot(),
    )?;
    let observed_slot = observed
        .first()
        .map(|account| account.observation.slot)
        .ok_or_else(|| Error::new("General snapshot was empty"))?;
    let recent_blockhash = rpc::fetch_latest_finalized_blockhash_v1(&arguments.rpc, observed_slot)?;
    let document = producer::produce_plan_v5(&route, observed, recent_blockhash).map_err(lift)?;
    producer::write_new_plan_v5(&arguments.output, &document).map_err(lift)?;

    println!(
        "Wrote an unsigned General `{}` plan for market {} to {}.",
        document.action(),
        document.market(),
        arguments.output.display()
    );
    println!(
        "It is bound to one finalized snapshot at slot {} and requires {} wallet signature(s).",
        document.observed_slot(),
        document.required_signers().len()
    );
    println!(
        "No key was read and nothing was simulated or submitted. Review promptly: its recent blockhash expires."
    );
    Ok(())
}

fn lift(error: producer::Error) -> Error {
    Error::new(error.to_string())
}

#[derive(Clone)]
struct PlanArgumentsV1 {
    rpc: String,
    route: PathBuf,
    output: PathBuf,
}

impl core::fmt::Debug for PlanArgumentsV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PlanArgumentsV1")
            .field("rpc", &rpc::origin(&self.rpc))
            .field("route", &self.route)
            .field("output", &self.output)
            .finish()
    }
}

impl PlanArgumentsV1 {
    fn parse(arguments: Vec<String>) -> Result<Self> {
        let mut rpc = None;
        let mut route = None;
        let mut output = None;
        let mut iterator = arguments.into_iter();
        while let Some(argument) = iterator.next() {
            let slot = match argument.as_str() {
                "--rpc" => &mut rpc,
                "--route" => &mut route,
                "--output" => &mut output,
                other => {
                    return Err(Error::new(format!(
                        "unknown General plan argument `{other}`. Run `dclutch general --help`."
                    )));
                }
            };
            if slot.is_some() {
                return Err(Error::new(format!(
                    "General plan argument `{argument}` was repeated"
                )));
            }
            *slot = Some(iterator.next().ok_or_else(|| {
                Error::new(format!("General plan argument `{argument}` needs a value"))
            })?);
        }
        let rpc = rpc.unwrap_or_else(|| {
            std::env::var(RPC_URL_ENV_V1).unwrap_or_else(|_| DEFAULT_RPC_URL_V1.to_owned())
        });
        let route = absolute_path_v1(route, "--route")?;
        let output = absolute_path_v1(output, "--output")?;
        Ok(Self { rpc, route, output })
    }
}

fn absolute_path_v1(value: Option<String>, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value.ok_or_else(|| Error::new(format!("missing {label}")))?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be an absolute path")));
    }
    Ok(path)
}

/// Explain the one General act this binary performs.
#[must_use]
pub fn usage() -> &'static str {
    "dclutch general — build an unsigned General wallet handoff.\n\
     \n\
     USAGE\n\
     \n\
       dclutch general plan --route ABSOLUTE.json --output ABSENT-ABSOLUTE.json [--rpc URL]\n\
     \n\
     The route identifies a current General action and its accounts; it is not\n\
     trusted as state. This command reacquires every account together at\n\
     finalized commitment, hostile-decodes the authenticated artifacts and\n\
     lifecycle, and writes one mode-0600 JSON handoff containing an unsigned\n\
     Solana v0 transaction. The output path must not exist.\n\
     \n\
     This command reads no key, signs nothing, simulates nothing, and submits\n\
     nothing. A wallet must review and sign the output before its recent\n\
     blockhash expires.\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_arguments_are_exact_absolute_and_credential_redacted() {
        let error = PlanArgumentsV1::parse(vec![
            "--route".into(),
            "relative.json".into(),
            "--output".into(),
            "/tmp/plan.json".into(),
        ])
        .expect_err("relative route");
        assert!(error.to_string().contains("--route must be an absolute"));

        let parsed = PlanArgumentsV1::parse(vec![
            "--rpc".into(),
            "https://rpc.example/v1/SECRET".into(),
            "--route".into(),
            "/tmp/route.json".into(),
            "--output".into(),
            "/tmp/plan.json".into(),
        ])
        .expect("exact arguments");
        assert!(!format!("{parsed:?}").contains("SECRET"));
    }

    #[test]
    fn plan_arguments_refuse_repetition_and_key_bearing_options() {
        let repeated = PlanArgumentsV1::parse(vec![
            "--route".into(),
            "/tmp/a.json".into(),
            "--route".into(),
            "/tmp/b.json".into(),
            "--output".into(),
            "/tmp/plan.json".into(),
        ])
        .expect_err("repeated route");
        assert!(repeated.to_string().contains("was repeated"));

        let key = PlanArgumentsV1::parse(vec![
            "--route".into(),
            "/tmp/a.json".into(),
            "--output".into(),
            "/tmp/plan.json".into(),
            "--keypair".into(),
            "/tmp/key.json".into(),
        ])
        .expect_err("key option");
        assert!(key.to_string().contains("unknown"));
    }
}
