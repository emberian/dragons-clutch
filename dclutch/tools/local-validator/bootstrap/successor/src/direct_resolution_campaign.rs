//! One C-09 handoff from real provider terminalization to physical fund close.
//!
//! The flagship provider driver and terminal sequence remain the semantic
//! owners. This module gives the complete-life supervisor one read-only act
//! that reopens both owners, reauthenticates their finalized transactions and
//! chain poststates, and joins them without converting a generic terminal flag
//! into evidence.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
    str::FromStr as _,
};

use dclutch_source::resolution::SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3;
use serde::Serialize;
use solana_program::pubkey::Pubkey;

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    flagship_resolution::{
        DirectResolutionTerminalEvidenceV1, authenticate_direct_resolution_terminal_v1,
    },
    rpc::{Rpc, WritePolicyV1},
    terminal_sequence::{DirectResolutionCloseEvidenceV1, authenticate_direct_resolution_close_v1},
};

pub(crate) const COMMAND_V1: &str = "local-private-validator-direct-resolution-handoff-v1";
const EVIDENCE_SCHEMA_V1: &str = "dclutch-owned-loopback-direct-resolution-c09-handoff-v1";

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-direct-resolution-handoff-v1 \
     --rpc-url http://127.0.0.1:PORT --input ABSOLUTE_JSON \
     --checkpoint ABSOLUTE_JSON --close-journal ABSOLUTE_JSON \
     --output ABSOLUTE_NEW_JSON\n\
     \nRead-only C-09 handoff. Reopens the exact flagship provider input/checkpoint \
     and finalized DCLRFCQ1 journal, reauthenticates their transaction history and standing \
     chain facts, proves the same Market/Source/certificate/generation/selector crosses the seam, \
     and writes one no-clobber evidence file. It opens no key and sends nothing."
}

#[derive(Debug)]
struct ArgumentsV1 {
    rpc_url: String,
    input: PathBuf,
    checkpoint: PathBuf,
    close_journal: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DirectResolutionCampaignEvidenceV1 {
    schema: &'static str,
    status: &'static str,
    cluster: &'static str,
    terminal: DirectResolutionTerminalEvidenceV1,
    fund_close: DirectResolutionCloseEvidenceV1,
    first_valid_provider_observation: bool,
    permissionless_completion: bool,
    fund_closure_conserves_lamports: bool,
}

pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    let input = canonical_regular(&arguments.input, "Resolution input")?;
    let checkpoint = canonical_regular(&arguments.checkpoint, "Resolution checkpoint")?;
    let close_journal = canonical_regular(&arguments.close_journal, "Resolution close journal")?;
    let output = canonical_new_output(&arguments.output)?;
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, None)?;
    ExpectedClusterV1::OwnedLoopback.authenticate(&origin)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;

    let terminal = authenticate_direct_resolution_terminal_v1(&mut rpc, &input, &checkpoint)?;
    let source_state = Pubkey::from_str(&terminal.source_state)
        .map_err(|error| Error::new(format!("Resolution Source state: {error}")))?;
    let resolution_program = Pubkey::from_str(&terminal.resolution_program)
        .map_err(|error| Error::new(format!("Resolution program: {error}")))?;
    let closure_sequence = terminal
        .terminal_sequence
        .checked_add(1)
        .ok_or_else(|| Error::new("Resolution closure sequence overflowed"))?;
    let expected_receipt = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &closure_sequence.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;
    let market = Pubkey::from_str(&terminal.market)
        .map_err(|error| Error::new(format!("Resolution Market: {error}")))?;
    let fund_close = authenticate_direct_resolution_close_v1(
        &mut rpc,
        &origin,
        &close_journal,
        market,
        expected_receipt,
    )?;
    let evidence = join_direct_resolution_evidence_v1(terminal, fund_close)?;
    write_json_new(&output, &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

fn join_direct_resolution_evidence_v1(
    terminal: DirectResolutionTerminalEvidenceV1,
    fund_close: DirectResolutionCloseEvidenceV1,
) -> Result<DirectResolutionCampaignEvidenceV1> {
    if terminal.market != fund_close.market
        || terminal.source_state != fund_close.source_state
        || terminal.certificate != fund_close.terminal_certificate
        || terminal.generation != fund_close.generation
        || terminal.terminal_sequence != fund_close.terminal_sequence
        || terminal.selector != fund_close.selector
        || terminal.route != "primary"
        || terminal.certificate_kind != "resolution-success"
        || terminal.attempt_index != 0
        || !fund_close.permissionless
    {
        return Err(Error::new(
            "Resolution terminalization and DCLRFCQ1 closure do not name one exact first-valid Market life",
        ));
    }
    Ok(DirectResolutionCampaignEvidenceV1 {
        schema: EVIDENCE_SCHEMA_V1,
        status: "finalized",
        cluster: "owned-loopback",
        terminal,
        fund_close,
        first_valid_provider_observation: true,
        permissionless_completion: true,
        fund_closure_conserves_lamports: true,
    })
}

fn parse(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut input = None;
    let mut checkpoint = None;
    let mut close_journal = None;
    let mut output = None;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{flag} requires a value")))?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            "--input" => &mut input,
            "--checkpoint" => &mut checkpoint,
            "--close-journal" => &mut close_journal,
            "--output" => &mut output,
            _ => return Err(Error::new(format!("unknown {COMMAND_V1} argument: {flag}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{flag} may be supplied only once")));
        }
    }
    let required = |value: Option<String>, label: &str| {
        value.ok_or_else(|| Error::new(format!("{label} is required")))
    };
    Ok(ArgumentsV1 {
        rpc_url: required(rpc_url, "--rpc-url")?,
        input: PathBuf::from(required(input, "--input")?),
        checkpoint: PathBuf::from(required(checkpoint, "--checkpoint")?),
        close_journal: PathBuf::from(required(close_journal, "--close-journal")?),
        output: PathBuf::from(required(output, "--output")?),
    })
}

fn canonical_regular(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be an absolute path")));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("inspect {label} {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Error::new(format!("{label} must be one ordinary file")));
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| Error::new(format!("canonicalize {label}: {error}")))?;
    if canonical != path {
        return Err(Error::new(format!(
            "{label} path must already be canonical"
        )));
    }
    Ok(canonical)
}

fn canonical_new_output(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() || path.exists() {
        return Err(Error::new("--output must be one absent absolute path"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("--output omitted its parent"))?;
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|error| Error::new(format!("canonicalize output parent: {error}")))?;
    if canonical_parent != parent {
        return Err(Error::new("--output parent must already be canonical"));
    }
    Ok(path.to_path_buf())
}

fn write_json_new(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| Error::new(format!("create C-09 handoff {}: {error}", path.display())))?;
    serde_json::to_writer_pretty(&mut output, value)?;
    output.write_all(b"\n")?;
    output.sync_all()?;
    if let Some(parent) = path.parent() {
        OpenOptions::new().read(true).open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn terminal() -> DirectResolutionTerminalEvidenceV1 {
        DirectResolutionTerminalEvidenceV1 {
            input_sha256: "11".repeat(32),
            checkpoint_sha256: "22".repeat(32),
            market: Pubkey::new_from_array([1; 32]).to_string(),
            source_state: Pubkey::new_from_array([2; 32]).to_string(),
            source_state_sha256: "33".repeat(32),
            certificate: Pubkey::new_from_array([3; 32]).to_string(),
            certificate_sha256: "44".repeat(32),
            resolution_program: Pubkey::new_from_array([4; 32]).to_string(),
            generation: 5,
            terminal_sequence: 6,
            selector: 7,
            attempt_index: 0,
            route: "primary",
            certificate_kind: "resolution-success",
            finalized_receipts: vec![json!({"stage":"submit"})],
        }
    }

    fn close(terminal: &DirectResolutionTerminalEvidenceV1) -> DirectResolutionCloseEvidenceV1 {
        DirectResolutionCloseEvidenceV1 {
            journal_sha256: "55".repeat(32),
            journal_state_sha256: "66".repeat(32),
            market: terminal.market.clone(),
            source_state: terminal.source_state.clone(),
            terminal_certificate: terminal.certificate.clone(),
            receipt: Pubkey::new_from_array([8; 32]).to_string(),
            receipt_sha256: "77".repeat(32),
            beneficiary: Pubkey::new_from_array([9; 32]).to_string(),
            generation: terminal.generation,
            terminal_sequence: terminal.terminal_sequence,
            selector: terminal.selector,
            source_refund_lamports: 1,
            ledger_remaining_native_principal: 2,
            ledger_rent_lamports: 3,
            ledger_lamport_surplus: 4,
            refund_lamports: 10,
            permissionless: true,
            finalized_receipt: json!({"signature":"fixture"}),
        }
    }

    #[test]
    fn handoff_requires_one_exact_first_valid_life() {
        let terminal = terminal();
        let close = close(&terminal);
        let evidence = join_direct_resolution_evidence_v1(terminal.clone(), close.clone())
            .expect("one joined life");
        assert!(evidence.first_valid_provider_observation);
        assert!(evidence.permissionless_completion);
        assert!(evidence.fund_closure_conserves_lamports);

        let mut hostile = close.clone();
        hostile.selector += 1;
        assert_eq!(
            join_direct_resolution_evidence_v1(terminal.clone(), hostile)
                .expect_err("selector mismatch must refuse")
                .to_string(),
            "Resolution terminalization and DCLRFCQ1 closure do not name one exact first-valid Market life"
        );
        let mut hostile = close.clone();
        hostile.source_state = Pubkey::new_unique().to_string();
        assert_eq!(
            join_direct_resolution_evidence_v1(terminal.clone(), hostile)
                .expect_err("Source mismatch must refuse")
                .to_string(),
            "Resolution terminalization and DCLRFCQ1 closure do not name one exact first-valid Market life"
        );
        let mut hostile = close;
        hostile.permissionless = false;
        assert_eq!(
            join_direct_resolution_evidence_v1(terminal, hostile)
                .expect_err("a signer-bound close must refuse")
                .to_string(),
            "Resolution terminalization and DCLRFCQ1 closure do not name one exact first-valid Market life"
        );
    }

    #[test]
    fn command_line_refuses_unknown_duplicate_and_missing_inputs() {
        assert_eq!(
            parse(vec!["--unknown".into(), "x".into()])
                .expect_err("unknown argument must refuse")
                .to_string(),
            "unknown local-private-validator-direct-resolution-handoff-v1 argument: --unknown"
        );
        assert_eq!(
            parse(vec![
                "--rpc-url".into(),
                "http://127.0.0.1:8899".into(),
                "--rpc-url".into(),
                "http://127.0.0.1:8900".into(),
            ])
            .expect_err("duplicate argument must refuse")
            .to_string(),
            "--rpc-url may be supplied only once"
        );
        assert_eq!(
            parse(Vec::new())
                .expect_err("missing argument must refuse")
                .to_string(),
            "--rpc-url is required"
        );
    }
}
