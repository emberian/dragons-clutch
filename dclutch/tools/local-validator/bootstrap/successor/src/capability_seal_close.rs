//! The ZeroBump seal close, driven against a live cluster: one stranded seal,
//! one command, one act.
//!
//! # What this is for
//!
//! Exactly one account on devnet is stranded this way: a pre-cohort-8 seal whose
//! persisted bump byte is zero, which no derivation ever produces, so the
//! ordinary close arm cannot read it. The defunct arm exists for it. The offline
//! gate (`dclutch-release-tool seal-probe`) already proves, from a dumped
//! account, that the arm would take it; what did not exist was anything that
//! could actually send the close. The first real close is a devnet act, not a
//! gauntlet one, and this is the command that performs it.
//!
//! # Closer-keeps, which is why the closer signs
//!
//! This route has no recorded beneficiary. Account 1 signs and receives the
//! entire liberated balance -- the funded-crank pattern. That is the opposite of
//! the maker-replay close, whose beneficiary is a `rent_owner` the account
//! itself recorded and which nobody needs to sign for. The seal close puts the
//! beneficiary in a SIGNER slot rather than in a request field precisely because
//! a field naming a refund destination is a field a griefer fills in with
//! somebody else's address.
//!
//! It is permissionless and racing is harmless: the first closer wins the chore
//! and every later attempt refuses by absence. There is no wrong signer here,
//! only one who is too late.
//!
//! # The judgement is the probe's, not this command's
//!
//! Every go/no-go conjunct -- this Program owns it, it is rent-exempt at the
//! exact seal width, the body is defunct-canonical, some candidate reproduces
//! the address, and the release it is sealed under is not live -- is decided by
//! [`probe_defunct_seal_v1`], the same function the offline gate runs, over a
//! value built by the same type. This command adds a socket and a signature and
//! decides nothing. A refusal is printed as the probe's own sentence, naming the
//! conjunct that failed, and nothing is sent.

use std::path::{Path, PathBuf};

use serde_json::json;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
};

use dclutch_vm::capability_seal::{
    CAPABILITY_SEAL_BYTES_V1, CAPABILITY_SEAL_CLOSE_NO_BUMP_CANDIDATE_V1,
    CapabilitySealCloseRequestV1,
};
use dclutch_release_tool::{SealAccountDumpV1, probe_defunct_seal_v1};

use crate::campaign::read_keypair_file;
use crate::cluster::ClusterOriginV1;
use crate::rpc::{Rpc, WritePolicyV1};
use crate::{Error, Result};

/// The owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-capability-seal-close-v1";

/// The devnet command name.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-capability-seal-close-v1";

/// Exact top-level account count of the seal close outer.
const FRAME_ACCOUNTS_V1: usize = 7;

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-capability-seal-close-v1 --rpc-url http://127.0.0.1:PORT --seal ADDRESS --trading-program ADDRESS --live-release HEX64 --activation-cache ADDRESS --trading-programdata ADDRESS --evidence ABSOLUTE_JSON [--execute --closer-keypair ABSOLUTE_JSON]\n\
     dclutch-local-successor-bootstrap devnet-capability-seal-close-v1 --rpc-url URL --i-mean-devnet GENESIS_HASH --seal ADDRESS --trading-program ADDRESS --live-release HEX64 --activation-cache ADDRESS --trading-programdata ADDRESS --evidence ABSOLUTE_JSON [--execute --closer-keypair ABSOLUTE_JSON]\n\
     \nCloses ONE stranded capability seal through the ZeroBump (defunct) arm, the arm that exists for a pre-cohort-8 seal whose persisted bump byte is zero. Closer-keeps: the closer signs and receives the entire liberated balance, so the beneficiary is a signer rather than a request field a griefer could fill in with somebody else's address. Every go/no-go conjunct is decided by the same probe the offline gate runs, over the same type, so this cannot reach a verdict `dclutch-release-tool seal-probe` could not. Without --execute this is a DRY RUN that opens no key and sends nothing, and it still mines the bump candidate and prints the exact instruction. A seal the arm would not take refuses here by the probe's own sentence, naming the conjunct that failed."
}

/// Parsed command line.
#[derive(Debug)]
struct ArgumentsV1 {
    rpc_url: String,
    seal: Pubkey,
    trading_program: Pubkey,
    live_release: [u8; 32],
    activation_cache: Pubkey,
    trading_programdata: Pubkey,
    closer_keypair: Option<PathBuf>,
    evidence: PathBuf,
    execute: bool,
    acknowledgment: Option<String>,
}

/// Run one owned-loopback seal close.
pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    if arguments.acknowledgment.is_some() {
        return Err(Error::new(format!(
            "--i-mean-devnet belongs to {COMMAND_DEVNET_V1}, not to the owned-loopback arm"
        )));
    }
    let rpc = Rpc::connect(&arguments.rpc_url)?;
    close_v1(rpc, &arguments, "owned-loopback")
}

/// Run one devnet seal close.
///
/// This is the arm the one stranded seal is actually reached through, and the
/// cohort-9 plan review gates it on a FRESH probe at cut time rather than on the
/// dump anybody took earlier -- which is why the probe runs here, against the
/// account this command just read, and not on a document.
pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    let acknowledgment = arguments.acknowledgment.as_deref().ok_or_else(|| {
        Error::new("--i-mean-devnet GENESIS_HASH is required to close against a public cluster")
    })?;
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, Some(acknowledgment))?;
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let rpc = Rpc::connect_cluster(&origin, policy)?;
    let label = origin.label().to_owned();
    close_v1(rpc, &arguments, &label)
}

/// Everything both arms do once an authenticated RPC exists.
fn close_v1(mut rpc: Rpc, arguments: &ArgumentsV1, cluster: &str) -> Result<()> {
    let Some(account) = rpc.account(arguments.seal)? else {
        // Racing is expected and harmless; being late is not a failure.
        println!("seal                 {}", arguments.seal);
        println!("state                already closed; nothing to do");
        write_evidence(&arguments.evidence, arguments, None, None, cluster)?;
        return Ok(());
    };

    // The same value the offline gate builds, judged by the same function.
    let dump = SealAccountDumpV1::from_observed_v1(
        arguments.seal.to_bytes(),
        account.owner.to_bytes(),
        account.lamports,
        account.data.clone(),
    );
    let verdict = probe_defunct_seal_v1(
        &dump,
        arguments.trading_program.to_bytes(),
        arguments.live_release,
    );

    println!("== the probe, on the account just read ==");
    println!("seal                 {}", arguments.seal);
    println!("owner                {}", account.owner);
    println!("lamports             {}", account.lamports);
    println!("width                {} bytes", account.data.len());
    println!("owned by program     {}", yes_no(verdict.owner_is_program));
    println!("funded rent persists {}", yes_no(verdict.funded_rent_persists));
    println!("defunct-canonical    {}", yes_no(verdict.defunct.is_ok()));
    println!(
        "bump candidate       {}",
        verdict.bump_candidate.map_or_else(
            || "none reproduces the address".to_owned(),
            |b| b.to_string()
        )
    );
    println!(
        "sealed release live  {}",
        verdict
            .release_is_live
            .map_or_else(|| "unreadable".to_owned(), yes_no_owned)
    );

    if let Some(refusal) = verdict.refusal() {
        return Err(Error::new(format!(
            "this seal is not one the ZeroBump arm would close: {refusal}"
        )));
    }
    let candidate = verdict
        .bump_candidate
        .ok_or_else(|| Error::new("the probe passed without a bump candidate"))?;
    if candidate == CAPABILITY_SEAL_CLOSE_NO_BUMP_CANDIDATE_V1 {
        return Err(Error::new(
            "candidate zero selects the ORDINARY close arm, not the defunct one; a seal whose \
             address is reproduced by bump zero is not a seal this command may close",
        ));
    }
    let key = verdict
        .key
        .ok_or_else(|| Error::new("the probe passed without reading the seal's coordinates"))?;
    // The Registry the route compares account 2 against is the one the seal's
    // own body names, not one this command chooses.
    let registry = Pubkey::new_from_array(key.registry_program());

    let instruction = close_instruction(arguments, registry, candidate);
    println!("== the close ==");
    println!("registry             {registry}");
    println!("closer keeps         {} lamports", account.lamports);
    println!("bump candidate       {candidate}");
    println!("accounts             {}", instruction.accounts.len());
    println!(
        "signers              {}",
        instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_signer)
            .count()
    );

    if !arguments.execute {
        write_evidence(
            &arguments.evidence,
            arguments,
            Some((candidate, account.lamports, registry)),
            None,
            cluster,
        )?;
        println!("dry run; no key was opened and nothing was sent");
        return Ok(());
    }

    let path = arguments
        .closer_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --closer-keypair"))?;
    let closer = Keypair::new_from_array(read_keypair_file(path, "seal closer")?);
    println!("closer               {}", closer.pubkey());

    // The closer is the beneficiary AND a required signer, so the frame's
    // signer slot is filled with the key that is about to be paid rather than
    // with a fee payer who merely happens to be present.
    let mut instruction = instruction;
    instruction
        .accounts
        .get_mut(1)
        .ok_or_else(|| Error::new("the close frame lost its closer coordinate"))?
        .pubkey = closer.pubkey();

    let before = rpc
        .account(closer.pubkey())?
        .map_or(0, |account| account.lamports);
    let evidence = rpc.send(
        "capability seal close",
        std::slice::from_ref(&instruction),
        &closer,
    )?;
    if let Some(error) = evidence.error.as_ref() {
        return Err(Error::new(format!("the close refused on chain: {error}")));
    }
    println!("signature            {}", evidence.signature);
    println!("slot                 {}", evidence.slot);

    // The chain is asked whether the seal is gone, rather than the send being
    // taken as proof. A close that landed and left the seal readable would be
    // the one failure this must not report as success.
    let standing = rpc.account(arguments.seal)?;
    if standing.is_some_and(|value| value.lamports != 0 || !value.data.is_empty()) {
        return Err(Error::new(format!(
            "the close landed but the seal {} is still readable",
            arguments.seal
        )));
    }
    println!("seal after           gone (read back from chain)");
    let after = rpc
        .account(closer.pubkey())?
        .map_or(0, |account| account.lamports);
    println!("closer before        {before}");
    println!("closer after         {after} (fee deducted from the liberated rent)");

    write_evidence(
        &arguments.evidence,
        arguments,
        Some((candidate, account.lamports, registry)),
        Some((&evidence, closer.pubkey(), before, after)),
        cluster,
    )?;
    Ok(())
}

/// The exact seven-account frame, in the order the route reads it.
///
/// The closer slot is filled with a placeholder on a dry run, because a dry run
/// opens no key: the frame's SHAPE is what a preflight can show, and the
/// beneficiary is substituted only once a key exists to sign with.
fn close_instruction(arguments: &ArgumentsV1, registry: Pubkey, bump_candidate: u8) -> Instruction {
    let accounts = vec![
        AccountMeta::new(arguments.seal, false),
        AccountMeta::new(Pubkey::default(), true),
        AccountMeta::new_readonly(registry, false),
        AccountMeta::new_readonly(arguments.activation_cache, false),
        AccountMeta::new_readonly(arguments.trading_program, false),
        AccountMeta::new_readonly(arguments.trading_programdata, false),
        AccountMeta::new_readonly(solana_sdk_ids::sysvar::rent::ID, false),
    ];
    debug_assert_eq!(accounts.len(), FRAME_ACCOUNTS_V1);
    Instruction {
        program_id: arguments.trading_program,
        accounts,
        data: CapabilitySealCloseRequestV1::new(bump_candidate)
            .to_bytes()
            .to_vec(),
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn yes_no_owned(value: bool) -> String {
    yes_no(value).to_owned()
}

/// Write what this close was, whether or not it was sent.
fn write_evidence(
    path: &Path,
    arguments: &ArgumentsV1,
    probed: Option<(u8, u64, Pubkey)>,
    landed: Option<(&crate::model::TransactionEvidence, Pubkey, u64, u64)>,
    cluster: &str,
) -> Result<()> {
    let document = json!({
        "schema": "dclutch-capability-seal-close-evidence-v1",
        "cluster": cluster,
        "seal": arguments.seal.to_string(),
        "tradingProgram": arguments.trading_program.to_string(),
        "sealWidth": CAPABILITY_SEAL_BYTES_V1,
        "alreadyClosed": probed.is_none(),
        "probe": probed.map(|(candidate, lamports, registry)| json!({
            "bumpCandidate": candidate,
            "liberatedLamports": lamports,
            "registryProgram": registry.to_string(),
            "arm": "defunct",
        })),
        "landed": landed.map(|(evidence, closer, before, after)| json!({
            "signature": evidence.signature,
            "slot": evidence.slot,
            "closer": closer.to_string(),
            "closerLamportsBefore": before,
            "closerLamportsAfter": after,
            "computeUnitsConsumed": evidence.compute_units_consumed,
            "feeLamports": evidence.fee_lamports,
        })),
    });
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&document)?),
    )?;
    println!("evidence             {}", path.display());
    Ok(())
}

fn parse(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut seal = None;
    let mut trading_program = None;
    let mut live_release = None;
    let mut activation_cache = None;
    let mut trading_programdata = None;
    let mut closer_keypair = None;
    let mut evidence = None;
    let mut execute = false;
    let mut acknowledgment = None;
    let mut cursor = arguments.into_iter();
    while let Some(flag) = cursor.next() {
        let mut value = || {
            cursor
                .next()
                .ok_or_else(|| Error::new(format!("{flag} requires a value")))
        };
        match flag.as_str() {
            "--rpc-url" => rpc_url = Some(value()?),
            "--seal" => seal = Some(pubkey_flag(&value()?, "--seal")?),
            "--trading-program" => {
                trading_program = Some(pubkey_flag(&value()?, "--trading-program")?);
            }
            "--activation-cache" => {
                activation_cache = Some(pubkey_flag(&value()?, "--activation-cache")?);
            }
            "--trading-programdata" => {
                trading_programdata = Some(pubkey_flag(&value()?, "--trading-programdata")?);
            }
            "--live-release" => live_release = Some(hex32_flag(&value()?, "--live-release")?),
            "--closer-keypair" => closer_keypair = Some(PathBuf::from(value()?)),
            "--evidence" => evidence = Some(PathBuf::from(value()?)),
            "--i-mean-devnet" => acknowledgment = Some(value()?),
            "--execute" => execute = true,
            other => return Err(Error::new(format!("unknown flag: {other}"))),
        }
    }
    Ok(ArgumentsV1 {
        rpc_url: rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?,
        seal: seal.ok_or_else(|| Error::new("--seal is required"))?,
        trading_program: trading_program
            .ok_or_else(|| Error::new("--trading-program is required"))?,
        live_release: live_release.ok_or_else(|| Error::new("--live-release is required"))?,
        activation_cache: activation_cache
            .ok_or_else(|| Error::new("--activation-cache is required"))?,
        trading_programdata: trading_programdata
            .ok_or_else(|| Error::new("--trading-programdata is required"))?,
        closer_keypair,
        evidence: evidence.ok_or_else(|| Error::new("--evidence is required"))?,
        execute,
        acknowledgment,
    })
}

fn pubkey_flag(value: &str, flag: &str) -> Result<Pubkey> {
    value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{flag}: {error}")))
}

fn hex32_flag(value: &str, flag: &str) -> Result<[u8; 32]> {
    crate::plan::hex32(value).map_err(|error| Error::new(format!("{flag}: {error:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Vec<String> {
        let mut v = vec![
            "--rpc-url".into(),
            "http://127.0.0.1:8899".into(),
            "--seal".into(),
            "11111111111111111111111111111111".into(),
            "--trading-program".into(),
            "11111111111111111111111111111111".into(),
            "--live-release".into(),
            "00".repeat(32),
            "--activation-cache".into(),
            "11111111111111111111111111111111".into(),
            "--trading-programdata".into(),
            "11111111111111111111111111111111".into(),
            "--evidence".into(),
            "/nonexistent/evidence.json".into(),
        ];
        v.extend(extra.iter().map(|s| (*s).to_string()));
        v
    }

    /// Each arm refuses the other's cluster contract BEFORE it opens a socket,
    /// a file, or a key.
    #[test]
    fn neither_arm_accepts_the_other_cluster_contract() {
        let refusal = run_owned_loopback_v1(args(&["--i-mean-devnet", "SomeGenesisHash"]))
            .expect_err("the loopback arm must refuse a devnet acknowledgment");
        assert!(
            format!("{refusal:?}").contains("--i-mean-devnet belongs to"),
            "unexpected refusal: {refusal:?}",
        );

        let refusal = run_devnet_v1(args(&[]))
            .expect_err("the devnet arm must refuse to run without an acknowledgment");
        assert!(
            format!("{refusal:?}").contains("--i-mean-devnet"),
            "unexpected refusal: {refusal:?}",
        );
    }

    /// The frame is the width the route declares, the closer is its only
    /// signer, and the seal is writable.
    ///
    /// The route refuses any other shape as `CloseSealFrame`, and it refuses a
    /// signing seal outright, so these are the properties a preflight can prove
    /// without a chain.
    #[test]
    fn the_frame_is_seven_accounts_with_exactly_one_signer() {
        let arguments = parse(args(&[])).expect("well formed arguments");
        let instruction = close_instruction(&arguments, Pubkey::new_from_array([9; 32]), 254);
        assert_eq!(instruction.accounts.len(), FRAME_ACCOUNTS_V1);
        assert_eq!(
            instruction
                .accounts
                .iter()
                .filter(|meta| meta.is_signer)
                .count(),
            1,
        );
        // Closer-keeps: the beneficiary is the signer, at index one.
        assert!(instruction.accounts[1].is_signer);
        assert!(instruction.accounts[1].is_writable);
        // The seal is written and must never sign.
        assert!(instruction.accounts[0].is_writable);
        assert!(!instruction.accounts[0].is_signer);
        // Everything else is read-only.
        assert!(
            instruction.accounts[2..]
                .iter()
                .all(|meta| !meta.is_signer && !meta.is_writable),
        );
    }

    /// The request is the exact sixteen-byte wire, and the mined candidate is
    /// the byte that selects the defunct arm.
    #[test]
    fn the_request_carries_the_mined_candidate_and_round_trips() {
        let arguments = parse(args(&[])).expect("well formed arguments");
        let instruction = close_instruction(&arguments, Pubkey::new_from_array([9; 32]), 254);
        let decoded =
            CapabilitySealCloseRequestV1::decode(&instruction.data).expect("canonical request");
        assert_eq!(decoded.bump_candidate(), 254);
        assert_ne!(
            decoded.bump_candidate(),
            CAPABILITY_SEAL_CLOSE_NO_BUMP_CANDIDATE_V1,
            "a nonzero candidate is what selects the defunct arm over the ordinary one",
        );
    }

    /// Every coordinate this command cannot invent is required.
    #[test]
    fn the_coordinates_are_all_required() {
        for missing in [
            "--seal",
            "--trading-program",
            "--live-release",
            "--activation-cache",
            "--trading-programdata",
            "--evidence",
        ] {
            let filtered = drop_flag(args(&[]), missing);
            let refusal = parse(filtered).expect_err("a missing coordinate must refuse");
            assert!(
                format!("{refusal:?}").contains(missing),
                "{missing} was not named in its own refusal: {refusal:?}",
            );
        }
    }

    fn drop_flag(arguments: Vec<String>, flag: &str) -> Vec<String> {
        let mut output = Vec::new();
        let mut cursor = arguments.into_iter();
        while let Some(value) = cursor.next() {
            if value == flag {
                let _ = cursor.next();
                continue;
            }
            output.push(value);
        }
        output
    }
}
