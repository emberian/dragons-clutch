//! Declare one release set's successor: the only caller of `DCLRLND1`.
//!
//! The route has been live in the deployed Registry since cohort-8 with all
//! eight conjuncts dispatched, and until now nothing in this repository could
//! build its instruction. This is that caller, and it exists to be run seven
//! times on cut day.
//!
//! # What is passed in, and what is not
//!
//! Two release-set ids and a Registry program. That is all, and it is already
//! more than the route itself takes -- the wire is argument-free, and the ids
//! are here only to say WHICH accounts to fetch. Everything else, including
//! which key must sign for which role, is read out of the successor's own
//! activation cache by `dclutch_operator::registry::declare_successor_v1` and
//! re-derived from the account bytes, so an id typed wrong finds the wrong
//! account and is refused rather than declaring the wrong hop.
//!
//! # Keys
//!
//! The consenting upgrade authority is the retained Loader deployer: the one
//! key that can strand every market in the protocol. It is taken by
//! `--authority-keypair-env NAME`, where NAME is an ENVIRONMENT VARIABLE
//! holding the absolute path, and every flag that would carry the path itself
//! is refused at parse. A path on the command line is a path in the process
//! table and in the shell history, and this is not a key to learn that about
//! twice. The fee payer, which is a stranger to the hop and signs only for
//! rent, follows the ordinary `--fee-payer-keypair ABSOLUTE_JSON` convention
//! every other writer in this binary uses.
//!
//! Neither key is opened on a preflight.
//!
//! # Simulate first
//!
//! Both arms simulate before they do anything else, and the simulation needs no
//! key at all: signatures are zeroed and `sigVerify` is false, so the runtime
//! executes the frame and reports what the program did while the transaction is
//! never a block entry. That is the whole point on cut day -- the hop can be
//! checked against the live chain BEFORE the deployer key is fetched from
//! wherever it is kept, leaving the signature as the last unknown rather than
//! the first blocker. A simulation is a decision aid before a write and never
//! evidence that one happened; the record this tool reports as landed is read
//! back off the chain.

use std::path::{Path, PathBuf};

use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    registry::declare_successor_v1::{
        RegistryDeclareSuccessorReport, RegistryDeclareSuccessorState,
        build_registry_declare_successor_v1, declare_successor_frame_addresses_v1,
    },
};
use dclutch_registry_contract::{RELEASE_LINEAGE_BYTES_V1, ReleaseLineageV1};
use dclutch_release_set_contract::EXECUTION_ROLE_ORDER_V1;
use serde_json::json;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result,
    campaign::read_keypair_file,
    cluster::ClusterOriginV1,
    model::TransactionEvidence,
    rpc::{Rpc, WritePolicyV1},
};

/// The owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-declare-successor-v1";

/// The devnet command name.
///
/// Same declaration, same frame, same refusals: the only difference is how the
/// RPC origin is established. The loopback arm takes a credential-free
/// explicit-port loopback URL; this one takes a keyed cluster endpoint plus the
/// `--i-mean-devnet GENESIS` acknowledgment every other devnet writer in this
/// binary requires, and `Rpc::connect_cluster` authenticates the observed
/// genesis hash against it before a single account is read.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-declare-successor-v1";

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-declare-successor-v1 --rpc-url http://127.0.0.1:PORT --registry REGISTRY_PUBKEY --predecessor HEX64 --successor HEX64 --evidence ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON --authority-keypair-env NAME]\n\
     dclutch-local-successor-bootstrap devnet-declare-successor-v1 --rpc-url URL --i-mean-devnet GENESIS_HASH --registry REGISTRY_PUBKEY --predecessor HEX64 --successor HEX64 --evidence ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON --authority-keypair-env NAME]\n\
     \nThe only caller of the DCLRLND1 successor-declaration route. Declares that one release set is superseded by another, creating the 248-byte lineage record a market's history is followed through. Nothing about the hop is passed in: both endpoints, which roles moved, and which key must consent for each are read out of the two activation caches, and each cache address is re-derived from the id its own bytes carry.\n\
     \nBoth arms SIMULATE first, with no key and no signature, so the hop can be checked against the live chain before the consenting authority is fetched. Preflight opens no key and sends nothing. Execute sends one transaction and then reads the record back off the chain to prove what landed. The consenting upgrade authority is taken ONLY as --authority-keypair-env NAME, where NAME is an ENVIRONMENT VARIABLE holding the absolute path of a Solana CLI keypair file. This is the only way in. Any flag that would carry a key, or the path to one, is refused at parse and named in the refusal -- because a path on the command line is a path in the process table and in the shell history, and this is the key that can strand every market in the protocol."
}

/// Parsed command line.
struct ArgumentsV1 {
    rpc_url: String,
    registry: Pubkey,
    predecessor: [u8; 32],
    successor: [u8; 32],
    evidence: PathBuf,
    fee_payer_keypair: Option<PathBuf>,
    authority_keypair_env: Option<String>,
    execute: bool,
    acknowledgment: Option<String>,
}

fn parse(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut registry = None;
    let mut predecessor = None;
    let mut successor = None;
    let mut evidence = None;
    let mut fee_payer_keypair = None;
    let mut authority_keypair_env = None;
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
            "--registry" => {
                registry = Some(
                    value()?
                        .parse::<Pubkey>()
                        .map_err(|error| Error::new(format!("--registry: {error}")))?,
                );
            }
            "--predecessor" => predecessor = Some(release_set_id(&value()?, "--predecessor")?),
            "--successor" => successor = Some(release_set_id(&value()?, "--successor")?),
            "--evidence" => evidence = Some(PathBuf::from(value()?)),
            "--fee-payer-keypair" => fee_payer_keypair = Some(PathBuf::from(value()?)),
            "--authority-keypair-env" => authority_keypair_env = Some(value()?),
            "--i-mean-devnet" => acknowledgment = Some(value()?),
            "--execute" => execute = true,
            // The consenting authority is the retained Loader deployer. Refused
            // at parse, before the value has travelled any further -- not
            // deprecated in a doc comment, refused.
            "--authority-keypair" | "--authority-keypair-path" | "--secret-key" | "--seed" => {
                return Err(Error::new(format!(
                    "{flag} is refused: pass --authority-keypair-env NAME so the path never \
                     reaches the command line or the process table"
                )));
            }
            other => {
                return Err(Error::new(format!(
                    "unknown {COMMAND_V1} argument: {other}"
                )));
            }
        }
    }
    let predecessor = predecessor.ok_or_else(|| Error::new("--predecessor is required"))?;
    let successor = successor.ok_or_else(|| Error::new("--successor is required"))?;
    // Conjunct 3, refused before a socket is opened. The chain refuses it too;
    // reaching the chain to be told so costs a round trip and a reader's
    // attention for a fact both ids already carry.
    if predecessor == successor {
        return Err(Error::new(
            "--predecessor and --successor name the same release set, and a set is not its own \
             successor",
        ));
    }
    Ok(ArgumentsV1 {
        rpc_url: rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?,
        registry: registry.ok_or_else(|| Error::new("--registry is required"))?,
        predecessor,
        successor,
        evidence: evidence.ok_or_else(|| Error::new("--evidence is required"))?,
        fee_payer_keypair,
        authority_keypair_env,
        execute,
        acknowledgment,
    })
}

/// One 32-byte release-set id, as 64 lowercase hex characters.
///
/// A truncated prefix carries no type: `d202e1f4...` and `559f26e6...` are
/// indistinguishable as strings, and the recovery lane that mapped these hops
/// spent its whole first pass on two values that turned out to be plan digests
/// rather than release-set ids. So the full 64 characters are required, and a
/// short one is a refusal that says which flag was short rather than a
/// derivation that lands somewhere else.
fn release_set_id(value: &str, flag: &str) -> Result<[u8; 32]> {
    if value.len() != 64 {
        return Err(Error::new(format!(
            "{flag} must be a complete 64-character release-set id, not the {} characters given. \
             A truncated hex prefix carries no type: a plan digest and an execution release-set \
             id look identical at eight characters and are different values.",
            value.len()
        )));
    }
    let mut output = [0_u8; 32];
    for (index, slot) in output.iter_mut().enumerate() {
        let pair = value
            .get(index * 2..index * 2 + 2)
            .ok_or_else(|| Error::new(format!("{flag} is not 64 hex characters")))?;
        *slot =
            u8::from_str_radix(pair, 16).map_err(|error| Error::new(format!("{flag}: {error}")))?;
    }
    Ok(output)
}

/// Run one owned-loopback declaration.
pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    if arguments.acknowledgment.is_some() {
        return Err(Error::new(format!(
            "--i-mean-devnet belongs to {COMMAND_DEVNET_V1}, not to the owned-loopback arm"
        )));
    }
    let rpc = Rpc::connect(&arguments.rpc_url)?;
    declare_v1(rpc, &arguments, "owned-loopback")
}

/// Run one devnet declaration.
pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    let acknowledgment = arguments.acknowledgment.as_deref().ok_or_else(|| {
        Error::new("--i-mean-devnet GENESIS_HASH is required to declare against a public cluster")
    })?;
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, Some(acknowledgment))?;
    // ReadsOnly on a preflight is not decoration: it is what makes "preflight
    // opens no key and sends nothing" a property of the transport rather than a
    // promise made by this function. A simulation is admitted under it, because
    // a simulation commits nothing.
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let rpc = Rpc::connect_cluster(&origin, policy)?;
    let label = origin.label().to_owned();
    declare_v1(rpc, &arguments, &label)
}

/// Everything both arms do once an authenticated RPC exists.
fn declare_v1(mut rpc: Rpc, arguments: &ArgumentsV1, cluster: &str) -> Result<()> {
    let report = plan(&mut rpc, arguments)?;
    self_report(arguments, &report);

    // Simulated before any key is opened, on both arms and in both modes.
    let outcome = rpc.simulate_v0(
        "declare-successor",
        &[report.instruction.clone()],
        report
            .required_signers
            .first()
            .copied()
            .ok_or_else(|| Error::new("the frame named no fee payer"))?,
        report.observation,
        &[],
    )?;
    println!("\nsimulation");
    if let Some(units) = outcome.units_consumed {
        println!("  compute units      {units}");
    }
    match &outcome.error {
        None => println!("  accepted           yes (nothing was committed)"),
        Some(error) => {
            println!("  accepted           NO: {error}");
            for line in &outcome.logs {
                println!("    {line}");
            }
        }
    }

    if !arguments.execute {
        write_evidence(arguments, &report, &outcome.error, None, cluster)?;
        println!("\npreflight only; no key was opened and nothing was sent");
        return Ok(());
    }
    if !outcome.accepted() {
        return Err(Error::new(
            "refusing to send: the cluster refused this frame in simulation. Nothing about the \
             hop changes between a simulation and a send, so a refused simulation is a refused \
             declaration.",
        ));
    }

    let fee_payer_path = arguments
        .fee_payer_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --fee-payer-keypair"))?;
    let variable = arguments.authority_keypair_env.as_deref().ok_or_else(|| {
        Error::new(
            "--execute requires --authority-keypair-env NAME, naming the environment \
                    variable that holds the consenting upgrade authority's absolute keypair path",
        )
    })?;
    // No secret-bearing file is opened above this line.
    let fee_payer = Keypair::new_from_array(read_keypair_file(
        fee_payer_path,
        "successor declaration fee payer",
    )?);
    if fee_payer.pubkey() != *report.required_signers.first().expect("fee payer") {
        return Err(Error::new(format!(
            "the fee payer keypair expands to {}, and the frame was built to be funded by {}",
            fee_payer.pubkey(),
            report.required_signers.first().expect("fee payer"),
        )));
    }
    let authority = consenting_authority(variable, &report)?;

    let signers: Vec<&Keypair> = authority.iter().collect();
    let evidence = rpc.send_v0_with_signers(
        "declare-successor",
        &[report.instruction.clone()],
        &fee_payer,
        &signers,
        report.observation,
        &[],
    )?;
    drop(authority);

    // The chain is asked what the record says, rather than the send being taken
    // as proof that it says anything. Conjunct 8 already made the program read
    // back what it persisted; this reads back the same account from outside.
    let landed = rpc
        .account(report.lineage)?
        .ok_or_else(|| Error::new("the declaration landed and the lineage record is absent"))?;
    if landed.owner != arguments.registry || landed.data.len() != RELEASE_LINEAGE_BYTES_V1 {
        return Err(Error::new(format!(
            "the lineage record at {} is {} bytes owned by {}, not the {RELEASE_LINEAGE_BYTES_V1} \
             bytes the Registry owns",
            report.lineage,
            landed.data.len(),
            landed.owner
        )));
    }
    if landed.data != report.record.to_bytes() {
        return Err(Error::new(
            "the landed lineage record is not byte-equal to the one this caller projected",
        ));
    }
    let decoded = ReleaseLineageV1::decode(&landed.data).map_err(|error| {
        Error::new(format!(
            "the landed lineage record does not decode: {error:?}"
        ))
    })?;
    if decoded != report.record {
        return Err(Error::new(
            "the landed lineage record decodes to a different hop",
        ));
    }
    println!("\nlanded  {}", evidence.signature);
    println!(
        "  record           {} ({RELEASE_LINEAGE_BYTES_V1} bytes)",
        report.lineage
    );
    println!("  reads back as    the exact hop this caller projected");

    write_evidence(arguments, &report, &None, Some(&evidence), cluster)
}

/// Open the consenting authority, or prove none is needed.
///
/// A hop that moved no role cannot exist, so `required_signers` past the fee
/// payer is never empty in practice -- but the caller is told what it must hold
/// rather than assuming, and the key it opens is checked against the one the
/// CHAIN named. `--authority-keypair-env` naming the wrong file is a refusal
/// here, not a signature the cluster rejects for reasons that look like
/// something else.
fn consenting_authority(
    variable: &str,
    report: &RegistryDeclareSuccessorReport,
) -> Result<Vec<Keypair>> {
    let required: Vec<Pubkey> = report.required_signers.iter().skip(1).copied().collect();
    if required.is_empty() {
        return Ok(Vec::new());
    }
    if required.len() > 1 {
        return Err(Error::new(format!(
            "this hop needs {} distinct consenting authorities, and one --authority-keypair-env \
             names one file. Every role in every recovered devnet release set binds the same \
             deployer, so a hop needing more than one key is a cluster this caller has not seen \
             and wants a human before it sends.",
            required.len()
        )));
    }
    let path = dclutch_direct_ticket::keypair_path_from_environment_v1(variable)
        .map_err(|error| Error::new(error.to_string()))?;
    let keypair = Keypair::new_from_array(read_keypair_file(
        &path,
        "consenting release upgrade authority",
    )?);
    let expected = *required.first().expect("one required authority");
    if keypair.pubkey() != expected {
        // Naming neither the path nor the identity the file actually holds: the
        // chain named the key that must consent, and the only useful fact is
        // that the file behind the caller's own variable is not it.
        return Err(Error::new(format!(
            "the keypair named by ${variable} is not {expected}, the upgrade authority the \
             successor's activation cache binds for the roles that moved"
        )));
    }
    Ok(vec![keypair])
}

/// Read the six accounts at one finalized observation and project the frame.
fn plan(rpc: &mut Rpc, arguments: &ArgumentsV1) -> Result<RegistryDeclareSuccessorReport> {
    let addresses = declare_successor_frame_addresses_v1(
        arguments.registry,
        &arguments.predecessor,
        &arguments.successor,
    );
    let fee_payer = fee_payer_address(arguments)?;
    let keys = [
        fee_payer,
        addresses.lineage,
        addresses.predecessor_cache,
        addresses.successor_cache,
        system_program::ID,
        sysvar::rent::ID,
    ];
    let (slot, accounts) = rpc.finalized_accounts(&keys, 0)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    let mut observed = Vec::with_capacity(keys.len());
    for (key, account) in keys.iter().zip(accounts) {
        observed.push(match account {
            // A vacant address is a real observation: it is what conjunct 7
            // reads, and the lineage account is expected to be exactly this.
            None => ObservedAccount {
                observation,
                key: *key,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: Vec::new(),
            },
            Some(account) => ObservedAccount {
                observation,
                key: *key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: account.data,
            },
        });
    }
    let mut cursor = observed.into_iter();
    let state = RegistryDeclareSuccessorState {
        payer: cursor.next().ok_or_else(|| Error::new("payer"))?,
        lineage: cursor.next().ok_or_else(|| Error::new("lineage"))?,
        predecessor_cache: cursor
            .next()
            .ok_or_else(|| Error::new("predecessor cache"))?,
        successor_cache: cursor.next().ok_or_else(|| Error::new("successor cache"))?,
        system_program: cursor.next().ok_or_else(|| Error::new("system program"))?,
        rent_sysvar: cursor.next().ok_or_else(|| Error::new("rent sysvar"))?,
    };
    build_registry_declare_successor_v1(arguments.registry, &state).map_err(|error| {
        Error::new(format!(
            "the successor declaration was refused before it was built: {error:?}"
        ))
    })
}

/// The address that will fund the record, known before any key is opened.
///
/// Required even on a preflight, because the frame's payer is one of the eleven
/// accounts and a frame built around a placeholder is not the frame that would
/// be sent.
fn fee_payer_address(arguments: &ArgumentsV1) -> Result<Pubkey> {
    let path = arguments.fee_payer_keypair.as_deref().ok_or_else(|| {
        Error::new(
            "--fee-payer-keypair is required even for a preflight: the payer is one of the \
             eleven accounts, and a frame built around a placeholder is not the frame that \
             would be sent",
        )
    })?;
    payer_pubkey_from_file(path)
}

/// The public half of a keypair file, read without keeping the secret.
fn payer_pubkey_from_file(path: &Path) -> Result<Pubkey> {
    let keypair = Keypair::new_from_array(read_keypair_file(path, "successor declaration payer")?);
    let pubkey = keypair.pubkey();
    drop(keypair);
    Ok(pubkey)
}

/// Everything this caller knows before it decides whether to send.
fn self_report(arguments: &ArgumentsV1, report: &RegistryDeclareSuccessorReport) {
    println!("registry             {}", arguments.registry);
    println!(
        "predecessor          {}",
        hex(report.predecessor.as_bytes())
    );
    println!("successor            {}", hex(report.successor.as_bytes()));
    println!(
        "lineage record       {} (bump {})",
        report.lineage, report.lineage_bump
    );
    println!(
        "rent debit           {} lamports",
        report.lineage_rent_debit_lamports
    );
    println!("roles");
    for role in EXECUTION_ROLE_ORDER_V1 {
        let consent = report
            .consent
            .get(role.role_index())
            .expect("one projection per role");
        if consent.moved {
            println!("  {role:<11?} moved    consents {}", consent.slot);
        } else {
            println!("  {role:<11?} unmoved  system program, not a signer");
        }
    }
    println!("signatures required");
    for (index, signer) in report.required_signers.iter().enumerate() {
        let what = if index == 0 { "payer" } else { "authority" };
        println!("  {what:<9} {signer}");
    }
    println!("would write          {}", hex(&report.record.to_bytes()));
}

fn write_evidence(
    arguments: &ArgumentsV1,
    report: &RegistryDeclareSuccessorReport,
    simulation_error: &Option<serde_json::Value>,
    landed: Option<&TransactionEvidence>,
    cluster: &str,
) -> Result<()> {
    let roles: Vec<serde_json::Value> = EXECUTION_ROLE_ORDER_V1
        .into_iter()
        .map(|role| {
            let consent = report
                .consent
                .get(role.role_index())
                .expect("one projection per role");
            json!({
                "role": format!("{role:?}"),
                "moved": consent.moved,
                "consent_slot": consent.slot.to_string(),
                "must_sign": consent.must_sign,
            })
        })
        .collect();
    let document = json!({
        // One schema across both arms, with the cluster named as a FIELD.
        "schema": "dclutch-release-lineage-declaration-evidence-v1",
        "cluster": cluster,
        "registry": arguments.registry.to_string(),
        "observation_slot": report.observation.slot,
        "predecessor": hex(report.predecessor.as_bytes()),
        "successor": hex(report.successor.as_bytes()),
        "lineage_record": report.lineage.to_string(),
        "lineage_bump": report.lineage_bump,
        "lineage_rent_debit_lamports": report.lineage_rent_debit_lamports,
        "roles": roles,
        "required_signers": report
            .required_signers
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        // The exact bytes the record would hold, whether or not it was sent.
        // The record carries no clock, so this is the same 248 bytes a timely
        // declaration would have written.
        "projected_record": hex(&report.record.to_bytes()),
        "simulation_refusal": simulation_error,
        "landed": landed.map(|evidence| {
            json!({
                "signature": evidence.signature.clone(),
                "slot": evidence.slot,
            })
        }),
    });
    let serialized = serde_json::to_string_pretty(&document)
        .map_err(|error| Error::new(format!("evidence: {error}")))?;
    std::fs::write(&arguments.evidence, format!("{serialized}\n"))
        .map_err(|error| Error::new(format!("write {}: {error}", arguments.evidence.display())))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Vec<String> {
        let mut value = vec![
            "--rpc-url".into(),
            "http://127.0.0.1:8899".into(),
            "--registry".into(),
            "11111111111111111111111111111111".into(),
            "--predecessor".into(),
            "91dcbefd3f8d81b27236aeae535baffcb002210cffad680ba06feb7d7e2f90ae".into(),
            "--successor".into(),
            "559f26e64683bd986d56575518daec2b65bfb66e20845d8615b13ead268378b4".into(),
            "--evidence".into(),
            "/nonexistent/evidence.json".into(),
        ];
        value.extend(extra.iter().map(|entry| (*entry).to_string()));
        value
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

    /// Every flag that would carry the deployer's path is refused at parse.
    #[test]
    fn no_flag_carries_the_consenting_authoritys_path() {
        for flag in [
            "--authority-keypair",
            "--authority-keypair-path",
            "--secret-key",
            "--seed",
        ] {
            let refusal = parse(args(&[flag, "/keys/deployer.json"]))
                // Discarded rather than `#[derive(Debug)]` on `ArgumentsV1`:
                // that struct holds a keypair path, and a type that cannot be
                // printed cannot be printed by accident.
                .map(|_| ())
                .expect_err("a path-bearing key flag must be refused at parse");
            let text = format!("{refusal:?}");
            assert!(
                text.contains("--authority-keypair-env") && text.contains(flag),
                "unexpected refusal for {flag}: {text}",
            );
        }
        let usage = usage();
        for forbidden in ["--authority-keypair ", "--secret-key", "--seed"] {
            assert!(!usage.contains(forbidden), "usage exposed {forbidden}");
        }
    }

    /// A truncated release-set id is refused, and the refusal says why.
    ///
    /// This is the recovery lane's own lesson made mechanical: a plan digest
    /// and an execution release-set id are indistinguishable at eight
    /// characters, and the prose around them called both "the set".
    #[test]
    fn a_truncated_release_set_id_is_refused_rather_than_derived_from() {
        let refusal = parse(args(&["--predecessor", "91dcbefd"]))
            .map(|_| ())
            .expect_err("a short id must be refused");
        let text = format!("{refusal:?}");
        assert!(text.contains("64-character"), "unexpected refusal: {text}");
        assert!(
            text.contains("plan digest"),
            "the refusal must name what a short id can be confused with: {text}"
        );
    }

    #[test]
    fn a_complete_id_parses_to_its_exact_bytes() {
        let parsed = parse(args(&[])).expect("complete ids parse");
        assert_eq!(
            hex(&parsed.predecessor),
            "91dcbefd3f8d81b27236aeae535baffcb002210cffad680ba06feb7d7e2f90ae"
        );
        assert_eq!(
            hex(&parsed.successor),
            "559f26e64683bd986d56575518daec2b65bfb66e20845d8615b13ead268378b4"
        );
        assert!(!parsed.execute, "preflight is the default");
    }

    /// A hop from a set to itself is refused before a socket is opened.
    #[test]
    fn a_set_is_not_its_own_successor() {
        let same = "559f26e64683bd986d56575518daec2b65bfb66e20845d8615b13ead268378b4";
        let refusal = run_owned_loopback_v1(args(&["--predecessor", same, "--successor", same]))
            .expect_err("self-succession must be refused");
        assert!(
            format!("{refusal:?}").contains("not its own"),
            "unexpected refusal: {refusal:?}",
        );
    }
}
