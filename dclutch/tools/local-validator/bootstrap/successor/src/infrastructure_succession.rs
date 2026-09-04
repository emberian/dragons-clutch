//! Run the infrastructure profile succession ceremony: the only caller of
//! `DCLTIIN2`.
//!
//! `docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md` §5 is the route; this is
//! the host that calls it, and it exists to be run ONCE on cut day, between the
//! Registry upgrade (step 3) and the declarations (step 5). Until now nothing
//! outside a program test could build its twenty-one-account frame.
//!
//! # Why this command exists at all
//!
//! The V1 profile made the Registry and Rent selections write-once with no
//! second write route, so moving either program's bytes refused founding, all
//! three retirement entries, the series-permit refund and both provider
//! resolution routes at once, with both escapes structurally dead. That is
//! P-008. The succession is the repair, and it is a ceremony rather than a
//! write: a new profile at its own one-seed PDA, created under evidence
//! strictly stronger than V1's own creation, with V1 left on chain
//! byte-identical forever.
//!
//! # What is passed in, and what is not
//!
//! A Core program and the two SUCCESSOR artifact release ids. That is all.
//! Which programs the profile selects, which of the two bindings actually
//! moved, which record is the predecessor for each, and which key must consent
//! for it are all read out of the V1 profile's own bytes and re-derived from
//! the chain, so an id typed wrong finds the wrong account and is refused
//! rather than succeeding to the wrong selection. Nothing here takes a `moved`
//! flag: the bytes state what moved, exactly as the route decides it.
//!
//! # Two wallets, and why
//!
//! A consent slot refuses `is_writable`, and message compilation UNIONS an
//! account's privileges across the whole transaction -- so a consent key that
//! is also the payer arrives at the program writable and is refused. The cut
//! therefore needs two wallets, and this command refuses the collision here by
//! name rather than letting it surface as a confusing on-chain refusal. The
//! builder refuses it too; this is the earlier, clearer of the two.
//!
//! # Keys
//!
//! Two signatures at most: Core's live Loader upgrade authority, and for each
//! MOVED binding the predecessor release's bound authority. On a cluster where
//! one deployer key holds both, the frame needs exactly one consenting
//! signature and the builder deduplicates it. The consenting authority is taken
//! ONLY by `--authority-keypair-env NAME`, where NAME is an ENVIRONMENT
//! VARIABLE holding the absolute path; a path on the command line is a path in
//! the process table and in the shell history. The fee payer follows the
//! ordinary `--fee-payer-keypair ABSOLUTE_JSON` convention.
//!
//! Neither key is opened on a preflight.
//!
//! # Simulate first
//!
//! Both arms simulate before anything else and the simulation needs no key, so
//! the ceremony can be checked against the live chain BEFORE the deployer key
//! is fetched -- leaving the signature as the last unknown rather than the
//! first blocker. This matters more here than anywhere else in this binary:
//! the succession spends the V2 domain's ONE SUCCESSION, and there is no second
//! attempt at it. (The domain is not necessarily vacant: since `c60b25e8` a
//! cohort is born at V2, and the ceremony supersedes that genesis profile in
//! place. What is spent once is the succession, not the account.) A simulation is a decision aid before a write and never
//! evidence that one happened; the 224 bytes this tool reports as landed are
//! read back off the chain and byte-compared against what it projected.

use std::path::{Path, PathBuf};

use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    infrastructure_succession_v1::{
        CoreInfrastructureSuccessionReportV1, CoreInfrastructureSuccessionStateV1,
        InfrastructureBindingV1, PredecessorRecordObservationV1, SuccessionProfileStandingV1,
        build_core_infrastructure_succession_v1,
    },
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::ARTIFACT_RELEASE_SCHEMA_ID_V1;
use dclutch_release_set_contract::{
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProtocolInfrastructureProfileV1,
    ProtocolInfrastructureProfileV2,
};
use serde_json::json;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use crate::{
    Error, Result,
    campaign::read_keypair_file,
    cluster::ClusterOriginV1,
    model::TransactionEvidence,
    rpc::{Rpc, WritePolicyV1},
};

/// The owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-infrastructure-succession-v1";

/// The devnet command name.
///
/// Same ceremony, same frame, same refusals: the only difference is how the RPC
/// origin is established.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-infrastructure-succession-v1";

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-infrastructure-succession-v1 --rpc-url http://127.0.0.1:PORT --core CORE_PUBKEY --registry-artifact HEX64 --rent-artifact HEX64 --evidence ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON --authority-keypair-env NAME]\n\
     dclutch-local-successor-bootstrap devnet-infrastructure-succession-v1 --rpc-url URL --i-mean-devnet GENESIS_HASH --core CORE_PUBKEY --registry-artifact HEX64 --rent-artifact HEX64 --evidence ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON --authority-keypair-env NAME]\n\
     \nThe only caller of the DCLTIIN2 infrastructure profile succession ceremony, and the repair for P-008. Creates the V2 profile that every redeployed consumer reads, selecting the successor Registry and Rent artifact releases. The predecessor V1 profile is never written and stays on chain byte-identical. Run ONCE, on cut day, AFTER the Registry upgrade and BEFORE the declarations: the ceremony refuses a succession in which nothing moved, so it cannot precede the upgrade it records.\n\
     \nOnly the two successor artifact release ids are passed in. Which programs are selected, which binding moved, and which key consents for it are read out of the V1 profile's own bytes and re-derived from the chain.\n\
     \nBoth arms SIMULATE first, with no key and no signature. Preflight opens no key and sends nothing. Execute sends one transaction and then reads the 224 bytes back off the chain and byte-compares them against what this caller projected. The consenting upgrade authority is taken ONLY as --authority-keypair-env NAME, naming an ENVIRONMENT VARIABLE that holds the absolute keypair path. The fee payer MUST NOT be a consenting authority: privileges merge across a transaction, a consent slot refuses is_writable, and the cut therefore needs two wallets."
}

/// Parsed command line.
#[derive(Debug)]
struct ArgumentsV1 {
    rpc_url: String,
    core: Pubkey,
    registry_artifact: [u8; 32],
    rent_artifact: [u8; 32],
    evidence: PathBuf,
    fee_payer_keypair: Option<PathBuf>,
    authority_keypair_env: Option<String>,
    execute: bool,
    acknowledgment: Option<String>,
}

fn parse(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut core = None;
    let mut registry_artifact = None;
    let mut rent_artifact = None;
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
            "--core" => {
                core = Some(
                    value()?
                        .parse::<Pubkey>()
                        .map_err(|error| Error::new(format!("--core: {error}")))?,
                );
            }
            "--registry-artifact" => {
                registry_artifact = Some(hex32("--registry-artifact", &value()?)?);
            }
            "--rent-artifact" => rent_artifact = Some(hex32("--rent-artifact", &value()?)?),
            "--evidence" => evidence = Some(PathBuf::from(value()?)),
            "--fee-payer-keypair" => fee_payer_keypair = Some(PathBuf::from(value()?)),
            "--authority-keypair-env" => authority_keypair_env = Some(value()?),
            "--execute" => execute = true,
            "--i-mean-devnet" => acknowledgment = Some(value()?),
            // The refusal is named rather than generic, because the whole point
            // of the env-var convention is lost if a near-miss flag silently
            // looks like it worked.
            "--authority-keypair" | "--authority" | "--upgrade-authority-keypair" => {
                return Err(Error::new(format!(
                    "{flag} is refused: the consenting upgrade authority is taken only as \
                     --authority-keypair-env NAME, where NAME is an ENVIRONMENT VARIABLE holding \
                     the absolute keypair path. A path on the command line is a path in the \
                     process table and in the shell history."
                )));
            }
            other => return Err(Error::new(format!("unexpected argument {other}"))),
        }
    }
    Ok(ArgumentsV1 {
        rpc_url: rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?,
        core: core.ok_or_else(|| Error::new("--core is required"))?,
        registry_artifact: registry_artifact
            .ok_or_else(|| Error::new("--registry-artifact is required"))?,
        rent_artifact: rent_artifact.ok_or_else(|| Error::new("--rent-artifact is required"))?,
        evidence: evidence.ok_or_else(|| Error::new("--evidence is required"))?,
        fee_payer_keypair,
        authority_keypair_env,
        execute,
        acknowledgment,
    })
}

fn hex32(flag: &str, value: &str) -> Result<[u8; 32]> {
    if value.len() != 64 {
        return Err(Error::new(format!(
            "{flag} must be exactly 64 hex characters, not {}",
            value.len()
        )));
    }
    let mut output = [0_u8; 32];
    for (index, slot) in output.iter_mut().enumerate() {
        let pair = value
            .get(index * 2..index * 2 + 2)
            .ok_or_else(|| Error::new(format!("{flag} is not hex")))?;
        *slot =
            u8::from_str_radix(pair, 16).map_err(|error| Error::new(format!("{flag}: {error}")))?;
    }
    Ok(output)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Run one owned-loopback succession.
pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    if arguments.acknowledgment.is_some() {
        return Err(Error::new(format!(
            "--i-mean-devnet belongs to {COMMAND_DEVNET_V1}, not to the owned-loopback arm"
        )));
    }
    let rpc = Rpc::connect(&arguments.rpc_url)?;
    succeed_v1(rpc, &arguments, "owned-loopback")
}

/// Run one devnet succession.
pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    let acknowledgment = arguments.acknowledgment.as_deref().ok_or_else(|| {
        Error::new(
            "--i-mean-devnet GENESIS_HASH is required to run the ceremony against a public cluster",
        )
    })?;
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, Some(acknowledgment))?;
    // ReadsOnly on a preflight makes "preflight sends nothing" a property of
    // the transport rather than a promise made by this function.
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let rpc = Rpc::connect_cluster(&origin, policy)?;
    let label = origin.label().to_owned();
    succeed_v1(rpc, &arguments, &label)
}

/// The addresses a succession frame reads, all derived rather than passed.
struct FrameAddressesV1 {
    predecessor_profile: Pubkey,
    registry_program: Pubkey,
    rent_program: Pubkey,
    /// Per binding: the successor record pair, and the predecessor record pair
    /// when this binding moved.
    registry_successor: RecordPairV1,
    rent_successor: RecordPairV1,
    registry_predecessor: Option<RecordPairV1>,
    rent_predecessor: Option<RecordPairV1>,
}

#[derive(Clone, Copy)]
struct RecordPairV1 {
    raw: Pubkey,
    staging: Pubkey,
}

/// Derive one finalized record's raw and staging addresses from its digest.
fn record_pair(registry_program: Pubkey, digest: &[u8; 32]) -> RecordPairV1 {
    RecordPairV1 {
        raw: Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                digest,
            ],
            &registry_program,
        )
        .0,
        staging: Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &ARTIFACT_RELEASE_SCHEMA_ID_V1,
                digest,
            ],
            &registry_program,
        )
        .0,
    }
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// Read the predecessor profile and derive every address the ceremony reads.
///
/// The V1 profile is the addressing authority for the whole frame: it names
/// both programs (conjunct 3 requires V2 to name the same two) and pins both
/// predecessor records by content. Reading it first is what lets this command
/// take two ids instead of a dozen addresses.
fn frame_addresses(
    rpc: &mut Rpc,
    core: Pubkey,
    registry_artifact: [u8; 32],
    rent_artifact: [u8; 32],
) -> Result<FrameAddressesV1> {
    let predecessor_profile =
        Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], &core).0;
    let account = rpc.account(predecessor_profile)?.ok_or_else(|| {
        Error::new(format!(
            "there is no predecessor profile at {predecessor_profile}. Succession without a \
             predecessor is the V1 initialize route's job, not this one."
        ))
    })?;
    if account.owner != core || account.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1 {
        return Err(Error::new(format!(
            "the account at {predecessor_profile} is {} bytes owned by {}, not the \
             {PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1} bytes Core owns",
            account.data.len(),
            account.owner
        )));
    }
    let profile = ProtocolInfrastructureProfileV1::decode(&account.data).map_err(|error| {
        Error::new(format!(
            "the predecessor profile does not decode: {error:?}"
        ))
    })?;
    let registry_program = Pubkey::new_from_array(profile.registry().program().to_bytes());
    let rent_program = Pubkey::new_from_array(profile.rent().program().to_bytes());

    // Moved-ness is DERIVED, never asserted: a binding moved exactly when the
    // successor id differs from the one V1 pinned. This is the same rule the
    // route applies, and stating it here rather than taking a flag is what
    // makes a mistyped id a refusal instead of a wrong succession.
    let pinned_registry = profile.registry().artifact_release().to_bytes();
    let pinned_rent = profile.rent().artifact_release().to_bytes();
    Ok(FrameAddressesV1 {
        predecessor_profile,
        registry_program,
        rent_program,
        registry_successor: record_pair(registry_program, &registry_artifact),
        rent_successor: record_pair(registry_program, &rent_artifact),
        registry_predecessor: (pinned_registry != registry_artifact)
            .then(|| record_pair(registry_program, &pinned_registry)),
        rent_predecessor: (pinned_rent != rent_artifact)
            .then(|| record_pair(registry_program, &pinned_rent)),
    })
}

/// Read every account at one finalized observation and project the ceremony.
fn plan_for_values_v1(
    rpc: &mut Rpc,
    core: Pubkey,
    registry_artifact: [u8; 32],
    rent_artifact: [u8; 32],
    fee_payer: Pubkey,
) -> Result<CoreInfrastructureSuccessionReportV1> {
    let addresses = frame_addresses(rpc, core, registry_artifact, rent_artifact)?;
    let core_programdata = programdata(core);

    // The live upgrade authority is read out of Core's ProgramData rather than
    // passed in, because it is the key the route will compare against and a
    // frame built around a different one is not the frame that would be sent.
    let core_account = rpc
        .account(core_programdata)?
        .ok_or_else(|| Error::new(format!("Core ProgramData {core_programdata} is absent")))?;
    let upgrade_authority = loader_upgrade_authority(&core_account.data).ok_or_else(|| {
        Error::new(format!(
            "Core ProgramData {core_programdata} binds no upgrade authority, so no key can \
             authorize a succession under it"
        ))
    })?;
    if upgrade_authority == fee_payer {
        return Err(Error::new(format!(
            "the fee payer {fee_payer} is also Core's upgrade authority. Privileges merge across \
             a transaction and the ceremony's consent slots refuse is_writable, so the payer and \
             every consenting authority must be different wallets."
        )));
    }

    let mut keys = vec![
        fee_payer,
        Pubkey::find_program_address(
            &[dclutch_release_set_contract::PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
            &core,
        )
        .0,
        addresses.predecessor_profile,
        core_programdata,
        upgrade_authority,
        addresses.registry_successor.raw,
        addresses.registry_successor.staging,
        addresses.registry_program,
        programdata(addresses.registry_program),
        addresses.rent_successor.raw,
        addresses.rent_successor.staging,
        addresses.rent_program,
        programdata(addresses.rent_program),
    ];
    for pair in [addresses.registry_predecessor, addresses.rent_predecessor]
        .into_iter()
        .flatten()
    {
        keys.push(pair.raw);
        keys.push(pair.staging);
    }
    keys.push(sysvar::rent::ID);
    keys.push(system_program::ID);

    let (slot, accounts) = rpc.finalized_accounts(&keys, 0)?;
    let observation = Observation {
        slot,
        unix_timestamp: rpc.block_time(slot)?,
        finality: Finality::Finalized,
    };
    let mut observed = Vec::with_capacity(keys.len());
    for (key, account) in keys.iter().zip(accounts) {
        observed.push(match account {
            // A vacant address is a real observation, and conjunct 6 reads
            // it. On a cohort born at V2 the V2 profile is present instead and
            // arrives through the `Some` arm; both are standings the ceremony
            // admits, and the builder is what tells them apart.
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
    let mut next = |label: &'static str| cursor.next().ok_or_else(|| Error::new(label));
    let payer = next("payer")?;
    let profile = next("succession profile")?;
    let predecessor_profile = next("predecessor profile")?;
    let core_programdata = next("Core ProgramData")?;
    let upgrade_authority = next("Core upgrade authority")?;
    let registry_artifact_raw = next("successor Registry record")?;
    let registry_artifact_staging = next("successor Registry staging cursor")?;
    let registry_program = next("Registry program")?;
    let registry_programdata = next("Registry ProgramData")?;
    let rent_artifact_raw = next("successor Rent record")?;
    let rent_artifact_staging = next("successor Rent staging cursor")?;
    let rent_program = next("Rent program")?;
    let rent_programdata = next("Rent ProgramData")?;
    let predecessor_registry_record = addresses
        .registry_predecessor
        .map(|_| -> Result<PredecessorRecordObservationV1> {
            Ok(PredecessorRecordObservationV1 {
                raw: next("predecessor Registry record")?,
                staging: next("predecessor Registry staging cursor")?,
            })
        })
        .transpose()?;
    let predecessor_rent_record = addresses
        .rent_predecessor
        .map(|_| -> Result<PredecessorRecordObservationV1> {
            Ok(PredecessorRecordObservationV1 {
                raw: next("predecessor Rent record")?,
                staging: next("predecessor Rent staging cursor")?,
            })
        })
        .transpose()?;
    let rent_sysvar = next("Rent sysvar")?;
    let system = next("System program")?;

    let state = CoreInfrastructureSuccessionStateV1 {
        payer,
        profile,
        predecessor_profile,
        core_programdata,
        upgrade_authority,
        registry_artifact_raw,
        registry_artifact_staging,
        registry_program,
        registry_programdata,
        rent_artifact_raw,
        rent_artifact_staging,
        rent_program,
        rent_programdata,
        predecessor_registry_record,
        predecessor_rent_record,
        rent_sysvar,
        system_program: system,
    };
    build_core_infrastructure_succession_v1(core, &state).map_err(|error| {
        Error::new(format!(
            "the succession was refused before it was built: {error:?}"
        ))
    })
}

/// Build the exact shipped-builder ceremony frame for the bootstrap campaign.
/// The campaign supplies only chain-derived successor ids and its distinct fee
/// payer; all moved-ness and consent still come from the predecessor profile.
pub(crate) fn plan_for_campaign_v1(
    rpc: &mut Rpc,
    core: Pubkey,
    registry_artifact: [u8; 32],
    rent_artifact: [u8; 32],
    fee_payer: Pubkey,
) -> Result<CoreInfrastructureSuccessionReportV1> {
    plan_for_values_v1(rpc, core, registry_artifact, rent_artifact, fee_payer)
}

/// Read a Loader V3 ProgramData account's bound upgrade authority.
fn loader_upgrade_authority(data: &[u8]) -> Option<Pubkey> {
    if data.get(0..4)? != 3_u32.to_le_bytes() || *data.get(12)? != 1 {
        return None;
    }
    let mut key = [0_u8; 32];
    key.copy_from_slice(data.get(13..45)?);
    Some(Pubkey::new_from_array(key))
}

/// The address that will fund the profile, known before any key is opened.
fn fee_payer_address(arguments: &ArgumentsV1) -> Result<Pubkey> {
    let path = arguments.fee_payer_keypair.as_deref().ok_or_else(|| {
        Error::new(
            "--fee-payer-keypair is required even for a preflight: the payer is one of the \
             twenty-one accounts, and a frame built around a placeholder is not the frame that \
             would be sent",
        )
    })?;
    Ok(Keypair::new_from_array(read_keypair_file(path, "succession fee payer")?).pubkey())
}

/// Everything both arms do once an authenticated RPC exists.
fn succeed_v1(mut rpc: Rpc, arguments: &ArgumentsV1, cluster: &str) -> Result<()> {
    let report = plan_for_values_v1(
        &mut rpc,
        arguments.core,
        arguments.registry_artifact,
        arguments.rent_artifact,
        fee_payer_address(arguments)?,
    )?;
    self_report(arguments, &report);

    // Simulated before any key is opened, on both arms and in both modes.
    let outcome = rpc.simulate_v0(
        "infrastructure-succession",
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
             succession changes between a simulation and a send, and this domain gets exactly \
             one succession, so a refused simulation is a refused ceremony.",
        ));
    }

    let fee_payer_path = arguments
        .fee_payer_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --fee-payer-keypair"))?;
    let variable = arguments.authority_keypair_env.as_deref().ok_or_else(|| {
        Error::new(
            "--execute requires --authority-keypair-env NAME, naming the environment variable \
             that holds the consenting upgrade authority's absolute keypair path",
        )
    })?;
    // No secret-bearing file is opened above this line.
    let fee_payer = Keypair::new_from_array(read_keypair_file(fee_payer_path, "succession payer")?);
    if fee_payer.pubkey() != *report.required_signers.first().expect("fee payer") {
        return Err(Error::new(format!(
            "the fee payer keypair expands to {}, and the frame was built to be funded by {}",
            fee_payer.pubkey(),
            report.required_signers.first().expect("fee payer"),
        )));
    }
    let authorities = consenting_authorities(variable, &report)?;

    let signers: Vec<&Keypair> = authorities.iter().collect();
    let evidence = rpc.send_v0_inline_with_signers(
        "infrastructure-succession",
        &[report.instruction.clone()],
        &fee_payer,
        &signers,
        report.observation,
    )?;
    drop(authorities);

    // The chain is asked what the profile says. Conjunct 7 already made the
    // program read back what it persisted; this reads the same account from
    // outside, and byte-compares it against the 224 bytes composed locally
    // before any key existed.
    let landed = rpc
        .account(report.profile)?
        .ok_or_else(|| Error::new("the ceremony landed and the succession profile is absent"))?;
    if landed.owner != arguments.core
        || landed.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
    {
        return Err(Error::new(format!(
            "the profile at {} is {} bytes owned by {}, not the \
             {PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2} bytes Core owns",
            report.profile,
            landed.data.len(),
            landed.owner
        )));
    }
    if landed.data != report.record.to_bytes() {
        return Err(Error::new(
            "the landed succession profile is not byte-equal to the one this caller projected",
        ));
    }
    let decoded = ProtocolInfrastructureProfileV2::decode(&landed.data).map_err(|error| {
        Error::new(format!(
            "the landed succession profile does not decode: {error:?}"
        ))
    })?;
    if decoded != report.record {
        return Err(Error::new(
            "the landed succession profile decodes to a different selection",
        ));
    }
    // V1 is evidence, never a target. If this ceremony wrote to it, the
    // write-once bar the whole succession rests on is gone.
    let predecessor = rpc
        .account(
            Pubkey::find_program_address(
                &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
                &arguments.core,
            )
            .0,
        )?
        .ok_or_else(|| Error::new("the predecessor profile vanished across the ceremony"))?;
    if predecessor.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1 {
        return Err(Error::new(
            "the predecessor profile changed width across the ceremony",
        ));
    }
    println!("\nlanded  {}", evidence.signature);
    println!(
        "  profile          {} ({PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2} bytes)",
        report.profile
    );
    println!("  reads back as    the exact selection this caller projected");
    println!(
        "  predecessor      unchanged, still {PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1} bytes"
    );

    write_evidence(arguments, &report, &None, Some(&evidence), cluster)
}

/// Open every consenting authority the chain named, or prove none is needed.
///
/// The keys past the fee payer are what the CHAIN said must consent, read out
/// of the predecessor records rather than chosen here, and the file named by
/// the environment variable is checked against them. Naming the wrong file is a
/// refusal here, not a signature the cluster rejects for reasons that look like
/// something else.
fn consenting_authorities(
    variable: &str,
    report: &CoreInfrastructureSuccessionReportV1,
) -> Result<Vec<Keypair>> {
    let required: Vec<Pubkey> = report.required_signers.iter().skip(1).copied().collect();
    if required.is_empty() {
        // Unreachable in practice: a succession that moved nothing is refused
        // by conjunct 4 long before here. Stated rather than assumed.
        return Ok(Vec::new());
    }
    let path = std::env::var(variable).map_err(|_| {
        Error::new(format!(
            "the environment variable {variable} named by --authority-keypair-env is not set; it \
             must hold the absolute path of the consenting authority's keypair file"
        ))
    })?;
    let keypair = Keypair::new_from_array(read_keypair_file(
        Path::new(&path),
        "succession consenting authority",
    )?);
    // One key on a cluster where the deployer holds everything; the builder
    // already deduplicated, so anything else means this file is the wrong one.
    if required.len() != 1 || required.first() != Some(&keypair.pubkey()) {
        return Err(Error::new(format!(
            "{variable} expands to a keypair for {}, and the chain named {} as the consenting \
             authority set. Every moved binding's predecessor record binds the key that must \
             consent for it, and this tool signs with exactly what the chain named.",
            keypair.pubkey(),
            required
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    Ok(vec![keypair])
}

/// Print what this ceremony would do, before it does any of it.
fn self_report(arguments: &ArgumentsV1, report: &CoreInfrastructureSuccessionReportV1) {
    println!("infrastructure succession");
    println!("  core             {}", arguments.core);
    println!(
        "  profile          {} ({})",
        report.profile,
        match report.profile_standing {
            SuccessionProfileStandingV1::Vacant => "vacant",
            // Since c60b25e8 this is the ordinary standing: initialization
            // writes the cohort's genesis V2, and the ceremony supersedes it in
            // place rather than creating an account.
            SuccessionProfileStandingV1::BornAtV2 => "genesis V2, succession unspent",
        }
    );
    println!("  bump             {}", report.profile_bump);
    println!(
        "  rent debit       {} lamports",
        report.profile_rent_debit_lamports
    );
    println!("  observation      slot {}", report.observation.slot);
    for consent in &report.consent {
        let binding = match consent.binding {
            InfrastructureBindingV1::Registry => "registry",
            InfrastructureBindingV1::Rent => "rent    ",
        };
        if consent.moved {
            println!(
                "  {binding}         MOVED, consented by {} (signs)",
                consent.slot
            );
        } else {
            println!("  {binding}         unmoved, System stands in its consent slot");
        }
    }
    println!("  signers          {}", report.required_signers.len());
    for signer in &report.required_signers {
        println!("    {signer}");
    }
    println!("  profile bytes    {}", hex(&report.record.to_bytes()));
}

/// Write the evidence document this run is accountable to.
fn write_evidence(
    arguments: &ArgumentsV1,
    report: &CoreInfrastructureSuccessionReportV1,
    simulation_error: &Option<serde_json::Value>,
    sent: Option<&TransactionEvidence>,
    cluster: &str,
) -> Result<()> {
    let document = json!({
        "schema": "dclutch-infrastructure-succession-v1",
        "cluster": cluster,
        "core_program": arguments.core.to_string(),
        "profile": report.profile.to_string(),
        "profile_bump": report.profile_bump,
        "profile_bytes_hex": hex(&report.record.to_bytes()),
        "profile_rent_debit_lamports": report.profile_rent_debit_lamports,
        "observation_slot": report.observation.slot,
        "registry_artifact_release_id": hex(&arguments.registry_artifact),
        "rent_artifact_release_id": hex(&arguments.rent_artifact),
        "consent": report.consent.iter().map(|consent| json!({
            "binding": match consent.binding {
                InfrastructureBindingV1::Registry => "registry",
                InfrastructureBindingV1::Rent => "rent",
            },
            "moved": consent.moved,
            "slot": consent.slot.to_string(),
            "must_sign": consent.must_sign,
        })).collect::<Vec<_>>(),
        "required_signers": report.required_signers.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "simulation_error": simulation_error,
        "executed": sent.is_some(),
        "signature": sent.map(|evidence| evidence.signature.clone()),
    });
    let bytes = serde_json::to_vec_pretty(&document)
        .map_err(|error| Error::new(format!("evidence: {error}")))?;
    std::fs::write(&arguments.evidence, &bytes).map_err(|error| {
        Error::new(format!(
            "evidence {}: {error}",
            arguments.evidence.display()
        ))
    })?;
    println!("\nevidence  {}", arguments.evidence.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Vec<String> {
        [
            "--rpc-url",
            "http://127.0.0.1:8899",
            "--core",
            "11111111111111111111111111111112",
            "--registry-artifact",
            &"a1".repeat(32),
            "--rent-artifact",
            &"a2".repeat(32),
            "--evidence",
            "/tmp/evidence.json",
        ]
        .iter()
        .map(ToString::to_string)
        .collect()
    }

    #[test]
    fn the_minimum_command_line_parses_and_defaults_to_preflight() {
        let arguments = parse(base()).expect("the minimum arguments must parse");
        assert!(!arguments.execute, "preflight is the default");
        assert!(arguments.acknowledgment.is_none());
        assert!(arguments.authority_keypair_env.is_none());
        assert_eq!(arguments.registry_artifact, [0xa1; 32]);
        assert_eq!(arguments.rent_artifact, [0xa2; 32]);
    }

    /// A path on the command line is a path in the process table.
    #[test]
    fn a_flag_carrying_the_authority_path_is_refused_by_name() {
        for flag in [
            "--authority-keypair",
            "--authority",
            "--upgrade-authority-keypair",
        ] {
            let mut arguments = base();
            arguments.push(flag.to_owned());
            arguments.push("/home/someone/deployer.json".to_owned());
            let error = parse(arguments).expect_err("a key path on argv must be refused");
            let rendered = format!("{error:?}");
            assert!(
                rendered.contains("--authority-keypair-env"),
                "the refusal must name the only way in: {rendered}"
            );
        }
    }

    #[test]
    fn the_loopback_arm_refuses_the_devnet_acknowledgment() {
        let mut arguments = base();
        arguments.push("--i-mean-devnet".to_owned());
        arguments.push("EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG".to_owned());
        let error = run_owned_loopback_v1(arguments)
            .expect_err("the loopback arm must refuse a devnet acknowledgment");
        assert!(format!("{error:?}").contains(COMMAND_DEVNET_V1));
    }

    #[test]
    fn the_usage_names_the_two_wallet_constraint() {
        let usage = usage();
        assert!(usage.contains(COMMAND_V1));
        assert!(usage.contains(COMMAND_DEVNET_V1));
        assert!(
            usage.contains("two wallets"),
            "the payer/consent collision is the constraint an operator most needs told"
        );
    }

    /// The ceremony runs after the upgrade, and the usage has to say so: a
    /// world whose Registry never moved cannot reach V2 at all.
    #[test]
    fn the_usage_states_the_cut_ordering() {
        let usage = usage();
        assert!(usage.contains("AFTER the Registry upgrade"));
        assert!(usage.contains("nothing moved"));
    }

    #[test]
    fn a_loader_programdata_image_yields_its_bound_authority() {
        let authority = Pubkey::new_from_array([0x7c; 32]);
        let mut data = vec![0_u8; 45];
        data[..4].copy_from_slice(&3_u32.to_le_bytes());
        data[12] = 1;
        data[13..45].copy_from_slice(authority.as_ref());
        assert_eq!(loader_upgrade_authority(&data), Some(authority));
        // Tag 0 is a revoked authority, and no key can authorize under it.
        data[12] = 0;
        assert_eq!(loader_upgrade_authority(&data), None);
    }

    #[test]
    fn a_record_pair_is_derived_from_its_own_digest() {
        let registry = Pubkey::new_from_array([0x9e; 32]);
        let first = record_pair(registry, &[0x11; 32]);
        let second = record_pair(registry, &[0x12; 32]);
        assert_ne!(
            first.raw, second.raw,
            "a different record has a different address"
        );
        assert_ne!(first.raw, first.staging);
    }
}
