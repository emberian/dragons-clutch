//! The first host caller of Trading's family-neutral capability-seal producer.
//!
//! # Why this command exists
//!
//! `dclutch_operator::capability_seal_v1::capability_seal_instruction_v1` has
//! composed the permissionless `DCLTSEL1` outer for ANY family's descriptor and
//! action since `bdce0dc8e`. Its only caller tree-wide was one program-test
//! (`programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs`),
//! so on a real chain the route existed, the producer existed, and nothing
//! could send it: `devnet-general-session` reported cohort-15's General seal at
//! fixed coordinate 38 as *producible and unproduced*, which is the
//! producer-missing shape one level up -- a builder, a schema and a refusal all
//! built, and only the test path ever exercised.
//!
//! # Why it consumes a frame report rather than deriving its own frame
//!
//! The seal's four seeds and the 39-account frame it must be handed are exactly
//! what `devnet-general-session` already derives from the Market's own records.
//! A second derivation here would be a second author for one frame, and the two
//! would agree right up until one of them stopped being maintained. So this
//! command reads that command's own report.
//!
//! Transcription is safe here for one specific reason, and it is the reason the
//! producer is a builder rather than a struct literal:
//! `capability_seal_instruction_v1` DERIVES the seal address from the seeds and
//! refuses `SealCoordinate` when the frame it was handed names a different one.
//! A report edited by hand, or a report of a different market, therefore cannot
//! produce a truthful verdict at an address no hot action derives -- it refuses
//! before a transaction is built. This command adds the two conjuncts a report
//! alone cannot carry: that the report is this schema, and that the frame it
//! states is the exact canonical width.

use std::path::{Path, PathBuf};

use dclutch_capability_program_contract::hot_v3::{
    HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
};
use dclutch_operator::capability_seal_v1::{
    CapabilitySealInstructionInputV1, capability_seal_instruction_v1,
};
use serde_json::{Value, json};
use solana_program::pubkey::Pubkey;
use solana_sdk::{signature::Keypair, signer::Signer};

use crate::{
    Error, Result,
    campaign::read_keypair_file,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1},
    general_session::FRAME_REPORT_SCHEMA_V1,
    plan::pubkey,
    rpc::{Rpc, WritePolicyV1},
};

/// The devnet arm's command name.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-capability-seal-v1";

fn refusal(code: &str, reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: [{code}] {}", reason.as_ref()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn usage() -> String {
    format!(
        "dclutch-local-successor-bootstrap {COMMAND_DEVNET_V1} \
         --rpc-url URL {DEVNET_ACKNOWLEDGMENT_FLAG} GENESIS_HASH \
         --frame-report ABSOLUTE_JSON --payer PUBKEY --evidence ABSOLUTE_NEW_JSON \
         [--payer-keypair ABSOLUTE_JSON --execute]\n     \
         Composes the permissionless validated-artifact seal for the family, \
         descriptor and action one devnet-general-session frame report states, \
         through the same builder that derives the seal address and refuses a \
         frame naming a different one. Without --execute this opens no key and \
         sends nothing. With it, the seal account is read back off the chain \
         before this command reports success."
    )
}

struct ArgumentsV1 {
    rpc_url: String,
    acknowledgment: String,
    frame_report: PathBuf,
    payer: Pubkey,
    payer_keypair: Option<PathBuf>,
    evidence: PathBuf,
    execute: bool,
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut frame_report = None;
    let mut payer = None;
    let mut payer_keypair = None;
    let mut evidence = None;
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        if flag == "--execute" {
            if execute {
                return Err(refusal("input/repeated-flag", flag));
            }
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| refusal("input/missing-value", format!("{flag}; usage: {}", usage())))?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--frame-report" => &mut frame_report,
            "--payer" => &mut payer,
            "--payer-keypair" => &mut payer_keypair,
            "--evidence" => &mut evidence,
            other => return Err(refusal("input/unknown-flag", other)),
        };
        if slot.replace(value).is_some() {
            return Err(refusal("input/repeated-flag", flag));
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| refusal("input/missing-flag", format!("{name}; usage: {}", usage())))
    };
    if execute && payer_keypair.is_none() {
        return Err(refusal(
            "input/missing-flag",
            "--execute requires --payer-keypair",
        ));
    }
    Ok(ArgumentsV1 {
        rpc_url: required(rpc_url, "--rpc-url")?,
        acknowledgment: required(acknowledgment, DEVNET_ACKNOWLEDGMENT_FLAG)?,
        frame_report: PathBuf::from(required(frame_report, "--frame-report")?),
        payer: pubkey(&required(payer, "--payer")?)?,
        payer_keypair: payer_keypair.map(PathBuf::from),
        evidence: PathBuf::from(required(evidence, "--evidence")?),
        execute,
    })
}

/// The four seeds and the frame one seal instruction is composed from.
#[derive(Debug)]
struct SealFrameV1 {
    market: String,
    fixed: Vec<Pubkey>,
    descriptor_digest: [u8; 32],
    action: u32,
    trading_semantic_release: [u8; 32],
    trading_program: Pubkey,
    registry_program: Pubkey,
}

fn identity_v1(value: &Value, field: &str) -> Result<[u8; 32]> {
    let text = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| refusal("report/shape", format!("capabilitySeal.{field}")))?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(refusal(
            "report/identity",
            format!("capabilitySeal.{field} is not 32 lowercase hexadecimal bytes"),
        ));
    }
    let mut output = [0_u8; 32];
    for (index, slot) in output.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|error| refusal("report/identity", error.to_string()))?;
    }
    Ok(output)
}

fn address_v1(value: &Value, field: &str) -> Result<Pubkey> {
    pubkey(
        value
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| refusal("report/shape", format!("capabilitySeal.{field}")))?,
    )
}

/// Read exactly the frame and seeds one `devnet-general-session` report states.
fn read_frame_report_v1(path: &Path) -> Result<SealFrameV1> {
    if !path.is_absolute() {
        return Err(refusal("input/relative", "--frame-report must be absolute"));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| refusal("input/unreadable", format!("frame report: {error}")))?;
    let report: Value = serde_json::from_slice(&bytes)
        .map_err(|error| refusal("report/json", error.to_string()))?;
    if report.get("schema").and_then(Value::as_str) != Some(FRAME_REPORT_SCHEMA_V1) {
        return Err(refusal(
            "report/schema",
            format!("--frame-report is not one {FRAME_REPORT_SCHEMA_V1} document"),
        ));
    }
    let seal = report
        .get("capabilitySeal")
        .ok_or_else(|| refusal(
            "report/no-seal",
            "this frame report predates the seal seeds; re-run devnet-general-session",
        ))?;
    let accounts = report
        .get("accounts")
        .and_then(Value::as_array)
        .ok_or_else(|| refusal("report/shape", "accounts"))?;
    // THE FRAME IS TAKEN BY COORDINATE, NEVER BY POSITION IN THE ARRAY.
    //
    // The report's `accounts` array carries the strategy evidence and the
    // caller authorities after the fixed frame, and each row states its own
    // coordinate. Reading the first 39 rows would be right today and silently
    // wrong the first time a row is inserted.
    let mut fixed = vec![Pubkey::default(); HOT_FIXED_ACCOUNT_COUNT_V3];
    let mut seen = vec![false; HOT_FIXED_ACCOUNT_COUNT_V3];
    for row in accounts {
        let coordinate = row
            .get("coordinate")
            .and_then(Value::as_u64)
            .ok_or_else(|| refusal("report/shape", "accounts[].coordinate"))?;
        let Ok(index) = usize::try_from(coordinate) else {
            continue;
        };
        if index >= HOT_FIXED_ACCOUNT_COUNT_V3 {
            continue;
        }
        if seen[index] {
            return Err(refusal(
                "report/duplicate-coordinate",
                format!("the frame report states fixed coordinate {index} twice"),
            ));
        }
        seen[index] = true;
        fixed[index] = address_v1(row, "address")?;
    }
    if let Some(missing) = seen.iter().position(|value| !*value) {
        return Err(refusal(
            "report/partial-frame",
            format!("the frame report states no fixed coordinate {missing}"),
        ));
    }
    Ok(SealFrameV1 {
        market: report
            .get("market")
            .and_then(Value::as_str)
            .unwrap_or("unstated")
            .to_owned(),
        fixed,
        descriptor_digest: identity_v1(seal, "descriptorDigest")?,
        action: u32::try_from(
            seal.get("action")
                .and_then(Value::as_u64)
                .ok_or_else(|| refusal("report/shape", "capabilitySeal.action"))?,
        )
        .map_err(|error| refusal("report/action", error.to_string()))?,
        trading_semantic_release: identity_v1(seal, "tradingSemanticRelease")?,
        trading_program: address_v1(seal, "tradingProgram")?,
        registry_program: address_v1(seal, "registryProgram")?,
    })
}

/// Run one devnet capability seal.
pub(crate) fn run_devnet(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    if arguments.evidence.exists() {
        return Err(refusal(
            "output/exists",
            format!("refusing to overwrite {}", arguments.evidence.display()),
        ));
    }
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, Some(&arguments.acknowledgment))?;
    ExpectedClusterV1::Devnet.authenticate(&origin)?;
    let frame = read_frame_report_v1(&arguments.frame_report)?;
    let composed = capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
        trading_program: frame.trading_program,
        registry_program: frame.registry_program,
        trading_semantic_release: frame.trading_semantic_release,
        descriptor_digest: frame.descriptor_digest,
        action: frame.action,
        fixed_frame: &frame.fixed,
        payer: arguments.payer,
    })
    .map_err(|error| {
        refusal(
            "seal/builder",
            format!(
                "the seal producer refused this frame: {error:?}; the frame's coordinate \
                 {HOT_CAPABILITY_SEAL_ACCOUNT_V3} is {}",
                frame
                    .fixed
                    .get(HOT_CAPABILITY_SEAL_ACCOUNT_V3)
                    .copied()
                    .unwrap_or_default()
            ),
        )
    })?;
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&origin, policy)?;
    let before = rpc.account(composed.seal)?;
    println!("market               {}", frame.market);
    println!("descriptor digest    {}", hex(&frame.descriptor_digest));
    println!("action               {}", frame.action);
    println!("trading semantic     {}", hex(&frame.trading_semantic_release));
    println!("seal (DERIVED)       {}", composed.seal);
    println!("bump                 {}", composed.bump);
    println!("accounts             {}", composed.instruction.accounts.len());
    println!(
        "seal before          {}",
        before.as_ref().map_or_else(
            || "absent".to_owned(),
            |account| format!("{} bytes, owner {}", account.data.len(), account.owner)
        )
    );
    let mut evidence = json!({
        "schema": "dclutch-devnet-capability-seal-evidence-v1",
        "cluster": "devnet",
        "rpcUrl": origin.redacted_url(),
        "frameReport": arguments.frame_report.display().to_string(),
        "market": frame.market,
        "seal": composed.seal.to_string(),
        "bump": composed.bump,
        "descriptorDigest": hex(&frame.descriptor_digest),
        "action": frame.action,
        "tradingSemanticRelease": hex(&frame.trading_semantic_release),
        "tradingProgram": frame.trading_program.to_string(),
        "registryProgram": frame.registry_program.to_string(),
        "payer": arguments.payer.to_string(),
        "accounts": composed.instruction.accounts.len(),
        "executed": arguments.execute,
    });
    if !arguments.execute {
        std::fs::write(
            &arguments.evidence,
            format!("{}\n", serde_json::to_string_pretty(&evidence)?),
        )
        .map_err(|error| Error::new(format!("seal evidence: {error}")))?;
        println!("dry run; no key was opened and nothing was sent");
        return Ok(());
    }
    let path = arguments
        .payer_keypair
        .as_deref()
        .ok_or_else(|| refusal("input/missing-flag", "--execute requires --payer-keypair"))?;
    let payer = Keypair::new_from_array(read_keypair_file(path, "seal payer")?);
    if payer.pubkey() != arguments.payer {
        return Err(refusal(
            "input/payer-mismatch",
            format!(
                "--payer names {} and the keypair holds {}; the frame was composed for the \
                 stated one",
                arguments.payer,
                payer.pubkey()
            ),
        ));
    }
    let sent = rpc.send(
        "general capability seal",
        std::slice::from_ref(&composed.instruction),
        &payer,
    )?;
    if let Some(error) = sent.error.as_ref() {
        return Err(refusal(
            "seal/on-chain",
            format!("the seal refused on chain: {error}"),
        ));
    }
    // THE CHAIN IS ASKED WHETHER THE SEAL EXISTS, NOT THE SEND.
    //
    // A send that landed and left coordinate 38 vacant is the one failure this
    // must not report as success, because every later reader of that
    // coordinate would find the same absence and blame something else.
    let after = rpc
        .account(composed.seal)?
        .ok_or_else(|| {
            refusal(
                "seal/absent-after",
                format!(
                    "the seal transaction landed and {} is still absent",
                    composed.seal
                ),
            )
        })?;
    if after.owner != frame.trading_program || after.data.is_empty() {
        return Err(refusal(
            "seal/shape-after",
            format!(
                "the seal at {} reads {} bytes owned by {}; Trading is {}",
                composed.seal,
                after.data.len(),
                after.owner,
                frame.trading_program
            ),
        ));
    }
    println!("signature            {}", sent.signature);
    println!("slot                 {}", sent.slot);
    println!(
        "compute units        {}",
        sent.compute_units_consumed
            .map_or_else(|| "unreported".to_owned(), |value| value.to_string())
    );
    println!(
        "seal after           {} bytes, owner {}, {} lamports",
        after.data.len(),
        after.owner,
        after.lamports
    );
    evidence["signature"] = json!(sent.signature);
    evidence["slot"] = json!(sent.slot);
    evidence["computeUnitsConsumed"] = json!(sent.compute_units_consumed);
    evidence["feeLamports"] = json!(sent.fee_lamports);
    evidence["sealAfter"] = json!({
        "bytes": after.data.len(),
        "owner": after.owner.to_string(),
        "lamports": after.lamports,
    });
    std::fs::write(
        &arguments.evidence,
        format!("{}\n", serde_json::to_string_pretty(&evidence)?),
    )
    .map_err(|error| Error::new(format!("seal evidence: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report_v1(seal: Pubkey) -> Value {
        let mut accounts = Vec::new();
        for coordinate in 0..HOT_FIXED_ACCOUNT_COUNT_V3 {
            let address = if coordinate == HOT_CAPABILITY_SEAL_ACCOUNT_V3 {
                seal
            } else {
                Pubkey::new_from_array([u8::try_from(coordinate).expect("byte") + 1; 32])
            };
            accounts.push(json!({
                "coordinate": coordinate,
                "label": "row",
                "address": address.to_string(),
            }));
        }
        // One row past the fixed frame, exactly as a real report carries.
        accounts.push(json!({
            "coordinate": HOT_FIXED_ACCOUNT_COUNT_V3,
            "label": "certificate raw",
            "address": Pubkey::new_from_array([200_u8; 32]).to_string(),
        }));
        json!({
            "schema": FRAME_REPORT_SCHEMA_V1,
            "market": "market",
            "accounts": accounts,
            "capabilitySeal": {
                "address": seal.to_string(),
                "descriptorDigest": "11".repeat(32),
                "action": 7,
                "tradingSemanticRelease": "22".repeat(32),
                "tradingProgram": Pubkey::new_from_array([250_u8; 32]).to_string(),
                "registryProgram": Pubkey::new_from_array([251_u8; 32]).to_string(),
            },
        })
    }

    fn write_report(value: &Value) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "dclutch-seal-report-{}-{}.json",
            std::process::id(),
            value.to_string().len()
        ));
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, value.to_string()).expect("write report");
        path
    }

    /// THE FRAME IS READ BY COORDINATE, AND A GAP IS A REFUSAL.
    ///
    /// A report missing one fixed row would otherwise leave a default zero
    /// pubkey in the frame, which the builder would then reject for the wrong
    /// reason -- `SealCoordinate` rather than "your report is incomplete".
    #[test]
    fn a_frame_report_missing_one_fixed_coordinate_refuses_by_that_name() {
        let mut report = report_v1(Pubkey::new_from_array([99_u8; 32]));
        let accounts = report
            .get_mut("accounts")
            .and_then(Value::as_array_mut)
            .expect("accounts");
        accounts.remove(5);
        let path = write_report(&report);
        let error = read_frame_report_v1(&path).expect_err("partial frame");
        let _ = std::fs::remove_file(&path);
        assert!(error.to_string().contains("report/partial-frame"), "{error}");
    }

    /// A DOCUMENT THAT IS NOT A FRAME REPORT IS NOT ONE.
    #[test]
    fn a_document_of_another_schema_refuses_before_any_field_is_read() {
        let path = write_report(&json!({"schema": "something-else", "accounts": []}));
        let error = read_frame_report_v1(&path).expect_err("wrong schema");
        let _ = std::fs::remove_file(&path);
        assert!(error.to_string().contains("report/schema"), "{error}");
    }

    /// THE BUILDER, NOT THIS COMMAND, DECIDES WHETHER THE FRAME NAMES THE SEAL.
    ///
    /// This is the conjunct that makes consuming a transcribed report safe: the
    /// seeds derive an address, and a frame naming any other one refuses before
    /// a transaction exists. A report whose coordinate 38 was edited by hand
    /// gets `SealCoordinate`, not a truthful verdict at the wrong address.
    #[test]
    fn a_frame_naming_the_wrong_seal_refuses_at_the_builder() {
        let report = report_v1(Pubkey::new_from_array([99_u8; 32]));
        let path = write_report(&report);
        let frame = read_frame_report_v1(&path).expect("read");
        let _ = std::fs::remove_file(&path);
        let error = capability_seal_instruction_v1(CapabilitySealInstructionInputV1 {
            trading_program: frame.trading_program,
            registry_program: frame.registry_program,
            trading_semantic_release: frame.trading_semantic_release,
            descriptor_digest: frame.descriptor_digest,
            action: frame.action,
            fixed_frame: &frame.fixed,
            payer: Pubkey::new_from_array([190_u8; 32]),
        })
        .expect_err("a hand-written seal coordinate");
        assert_eq!(
            error,
            dclutch_operator::capability_seal_v1::CapabilitySealBuilderErrorV1::SealCoordinate
        );
    }
}
