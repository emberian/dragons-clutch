//! Driving the real `dclutch-relayer` binary.
//!
//! The campaign never reimplements the daemon's observation, signing or
//! submission: it writes the daemon's own config, hands it its own key files,
//! and runs the same binary an operator would. What crosses the process
//! boundary is exactly what would cross it in production — a TOML file, an
//! artifact directory, and transactions on a loopback RPC.

use std::path::{Path, PathBuf};
use std::process::Command;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;

use crate::{Error, Result};

/// Write one Solana-convention 64-byte keypair file the daemon can load.
pub(crate) fn write_keypair_file(
    directory: &Path,
    name: &str,
    keypair: &Keypair,
) -> Result<PathBuf> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join(name);
    let bytes = keypair.to_bytes().to_vec();
    std::fs::write(&path, serde_json::to_string(&bytes)?)?;
    Ok(path)
}

/// Everything the rendered daemon config pins.
pub(crate) struct DaemonConfigV1 {
    pub(crate) path: PathBuf,
    pub(crate) output_dir: PathBuf,
    pub(crate) set_name: String,
}

/// Render the daemon's TOML for the rehearsal.
///
/// The observed cluster is the twin (loopback, its real genesis hash), the
/// attested identity is mainnet's — the rehearsal-twin table makes that loud —
/// and the submit endpoint is the successor validator, with the Market's
/// address lookup table routing the full-body append under the packet limit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_config(
    work: &Path,
    twin_rpc_url: &str,
    twin_genesis_base58: &str,
    attested_cluster_base58: &str,
    attestation_keypair: &Path,
    fee_payer_keypair: &Path,
    submit_endpoint: &str,
    relay_program: Pubkey,
    market: Pubkey,
    generation: u64,
    relayer_key_set_raw: Pubkey,
    relayer_key_set_staging: Pubkey,
    lookup_table: Option<(Pubkey, &[Pubkey])>,
    relay_family_id: [u8; 32],
    decoding_rules_id: [u8; 32],
    positions: &[(Pubkey, Pubkey, u16, Vec<u32>)],
) -> Result<DaemonConfigV1> {
    let output_dir = work.join("relayer-out");
    std::fs::create_dir_all(&output_dir)?;
    let mut text = String::new();
    text.push_str(&format!(
        "output_dir = \"{}\"\npoll_interval_seconds = 5\n\n[observed_cluster]\nrpc_endpoints = [\"{}\"]\nexpected_genesis_hash = \"{}\"\nrequest_timeout_seconds = 30\n\n[observed_cluster.rehearsal_twin]\nattested_cluster_id = \"{}\"\n\n[keys]\nattestation_keypair_path = \"{}\"\nfee_payer_keypair_path = \"{}\"\n\n[submit]\nendpoint = \"{}\"\nallow_public_submission = false\nrelay_program_id = \"{}\"\nmarket = \"{}\"\ngeneration = {}\nrelayer_key_set = \"{}\"\nrelayer_key_set_staging_vacancy = \"{}\"\ncompute_unit_limit = 400000\n",
        output_dir.display(),
        twin_rpc_url,
        twin_genesis_base58,
        attested_cluster_base58,
        attestation_keypair.display(),
        fee_payer_keypair.display(),
        submit_endpoint,
        relay_program,
        market,
        generation,
        relayer_key_set_raw,
        relayer_key_set_staging,
    ));
    if let Some((table, addresses)) = lookup_table {
        text.push_str(&format!(
            "\n[submit.address_lookup_table]\nkey = \"{table}\"\naddresses = [\n"
        ));
        for address in addresses {
            text.push_str(&format!("    \"{address}\",\n"));
        }
        text.push_str("]\n");
    }
    let set_name = "dbc-graduation".to_owned();
    text.push_str(&format!(
        "\n[[account_sets]]\nname = \"{set_name}\"\nrelay_family_id = \"{}\"\ndecoding_rules_id = \"{}\"\n",
        hex_lower(&relay_family_id),
        hex_lower(&decoding_rules_id),
    ));
    for (key, owner, inline_len, admitted) in positions {
        text.push_str(&format!(
            "\n[[account_sets.positions]]\nkey = \"{key}\"\nexpected_owner = \"{owner}\"\ninline_len = {inline_len}\n"
        ));
        if !admitted.is_empty() {
            let list: Vec<String> = admitted.iter().map(u32::to_string).collect();
            text.push_str(&format!("admitted_data_lens = [{}]\n", list.join(", ")));
        }
    }
    let path = work.join("relayer.toml");
    std::fs::write(&path, text)?;
    Ok(DaemonConfigV1 {
        path,
        output_dir,
        set_name,
    })
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// What one daemon invocation left behind.
pub(crate) struct DryRunArtifactsV1 {
    pub(crate) slot_dir: PathBuf,
    pub(crate) observed_slot: u64,
    #[allow(dead_code)]
    pub(crate) stdout: String,
}

fn run_relayer(
    relayer_bin: &Path,
    work: &Path,
    label: &str,
    arguments: &[String],
) -> Result<String> {
    let output = Command::new(relayer_bin)
        .args(arguments)
        .current_dir(work)
        .output()
        .map_err(|error| Error::new(format!("could not run {}: {error}", relayer_bin.display())))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    std::fs::write(work.join(format!("relayer-{label}.stdout")), &stdout)?;
    std::fs::write(work.join(format!("relayer-{label}.stderr")), &stderr)?;
    if !output.status.success() {
        return Err(Error::new(format!(
            "dclutch-relayer {label} failed ({}):\n{}",
            output.status,
            stderr.lines().rev().take(15).collect::<Vec<_>>().join("\n")
        )));
    }
    Ok(stdout)
}

/// One observation cycle, dry run: observe the twin, sign, write artifacts.
pub(crate) fn observe_dry_run(
    relayer_bin: &Path,
    work: &Path,
    config: &DaemonConfigV1,
) -> Result<DryRunArtifactsV1> {
    let stdout = run_relayer(
        relayer_bin,
        work,
        "dry-run",
        &[
            "run".into(),
            "--config".into(),
            config.path.display().to_string(),
            "--dry-run".into(),
            "--cycles".into(),
            "1".into(),
        ],
    )?;
    // The artifact directory is the daemon's own output contract: one
    // directory per set per cycle, named by the observed slot.
    let set_dir = config.output_dir.join("artifacts").join(&config.set_name);
    let mut newest: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(&set_dir).map_err(|error| {
        Error::new(format!(
            "no dry-run artifacts under {}: {error}",
            set_dir.display()
        ))
    })? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(slot_text) = name.strip_prefix("slot-")
            && let Ok(slot) = slot_text.parse::<u64>()
            && newest.as_ref().is_none_or(|(best, _)| slot > *best)
        {
            newest = Some((slot, entry.path()));
        }
    }
    let (observed_slot, slot_dir) =
        newest.ok_or_else(|| Error::new("the dry run produced no slot directory"))?;
    Ok(DryRunArtifactsV1 {
        slot_dir,
        observed_slot,
        stdout,
    })
}

/// Submit the recorded observation: append x set-count, then seal.
pub(crate) fn submit_artifacts(
    relayer_bin: &Path,
    work: &Path,
    config: &DaemonConfigV1,
    slot_dir: &Path,
) -> Result<String> {
    run_relayer(
        relayer_bin,
        work,
        "submit-artifacts",
        &[
            "submit-artifacts".into(),
            "--config".into(),
            config.path.display().to_string(),
            "--slot-dir".into(),
            slot_dir.display().to_string(),
        ],
    )
}
