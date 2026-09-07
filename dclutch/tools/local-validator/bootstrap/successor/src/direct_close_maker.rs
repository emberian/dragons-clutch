//! The permissionless Direct maker-replay close, driven against a live cluster.
//!
//! This is the caller side of `DCLTDMC1`. Until it existed the route had a
//! program, a codec, and a bank-driven program test, and no way at all to reach
//! a live chain: the only builders in the tree were inside a program test's
//! fixture and inside the operator crate that nothing called. A route that can
//! only be reached from a bank is not a route a stranger can crank, and
//! "permissionless" is a claim about strangers.
//!
//! # What this reads and what it refuses to invent
//!
//! The wire carries a COORDINATE -- market, maker, generation -- and nothing
//! economic. So neither does this driver. The rent beneficiary, the historical
//! principal, the donation slice and the resulting maker-root count are all
//! read off authenticated chain bytes by
//! [`plan_direct_close_maker_v1`], which is the same function the cut's
//! sequencer would call and which re-derives every account coordinate from the
//! Market's own state before it agrees to build anything.
//!
//! The donation is no longer one destination. Decision 0024 split the observed
//! balance three ways -- the recorded owner takes the principal EXACTLY, the
//! closer takes a carve out of the donation, and every remaining lamport is
//! credited to the upkeep vault -- and the carve's share and its lamport
//! ceiling are read out of the Custody-owned governed parameters record this
//! frame carries at coordinate 22. That record is not passed in either: this
//! driver derives its address under the Custody program the Market's own
//! release set names, and the plan stage authenticates it before a single
//! number comes out of it.
//!
//! This driver's whole job is to turn a cluster into that function's input and
//! its answer into something an operator can read. It decides nothing.
//!
//! # Why the refusals matter more than the submission
//!
//! Two of this route's refusals are reachable states of a real market rather
//! than hostile input: a replay that still owes its Direct fee (`0x4011`) and a
//! replay with registered live intents (`0x4012`). Both are met at PLAN time
//! here -- before a key is opened, a transaction is built, or a fee is spent --
//! and both are reported by name with the remedy attached. A cut day that
//! learns "settle the fee first" from a preflight has lost nothing; one that
//! learns it from a failed devnet transaction has spent a slot and a signature
//! to be told the same thing.
//!
//! # The two arms
//!
//! Same close, same plan, same refusals: the ONLY difference is how the RPC
//! origin is established. The loopback arm takes a credential-free explicit-port
//! loopback URL; the devnet arm takes a keyed endpoint plus the
//! `--i-mean-devnet GENESIS` acknowledgment every other devnet writer in this
//! binary requires, and `Rpc::connect_cluster` authenticates health and the
//! observed genesis hash before a single account is read. The plan builder is
//! told which cluster it is looking at and checks the claim itself, so a
//! loopback run pointed at devnet by mistake refuses inside the builder even if
//! the transport somehow allowed it.

use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

use dclutch_core_contract::ContentId;
use dclutch_custody::upkeep_vault_v1::UpkeepVaultSeedsV1;
use dclutch_market::CoreState;
use dclutch_market::capability_manifest::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1,
};
use dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2;
use dclutch_market::capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
use dclutch_market::protocol_parameters::ProtocolParametersRecordSeedsV1;
use dclutch_operator::direct_close_maker_v1::{
    DirectCloseMakerClusterV1, DirectCloseMakerPlanErrorV1, DirectCloseMakerPlanV1,
    DirectCloseMakerSnapshotV1, DirectCloseMakerSubmitV1, plan_direct_close_maker_v1,
};
use dclutch_registry::record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_registry::release_set::{
    CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1, ExecutionRoleV1,
};
use dclutch_registry::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_trading::{
    close_maker_bundle_v1::{
        direct_close_maker_account_profile_schema_v1, direct_close_maker_descriptor_schema_v1,
        direct_close_maker_effect_schema_v1,
    },
    close_maker_v1::{
        DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1, DIRECT_CLOSE_MAKER_CALLER_AUTHORITY_ACCOUNT_V1,
        DIRECT_CLOSE_MAKER_CLOSER_ACCOUNT_V1, DIRECT_CLOSE_MAKER_CUSTODY_PROGRAM_ACCOUNT_V1,
        DIRECT_CLOSE_MAKER_PROTOCOL_PARAMETERS_ACCOUNT_V1,
        DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1, DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1,
        DIRECT_CLOSE_MAKER_UPKEEP_VAULT_ACCOUNT_V1, DirectCloseMakerRequestV1,
    },
    program_set_v4::build_direct_inline_ordinary_lifecycle_program_set_v1,
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DirectCoordinatesV1, MakerReplayRootV1,
        MakerReplaySeedsV1,
    },
};

use crate::campaign::{
    parse_campaign_terminal_evidence_with_expected_cluster_v1, read_keypair_file,
};
use crate::cluster::{ClusterOriginV1, ExpectedClusterV1};
use crate::direct_trade::{
    authenticate_devnet_terminal_evidence_v1, authenticate_owned_loopback_terminal_evidence_v1,
};
use crate::model::MarketRunInput;
use crate::rpc::{Rpc, WritePolicyV1, parse_json_without_duplicate_keys_v1};
use crate::terminal_lifecycle::{authenticate_plan_source, finalized_snapshot};
use crate::{Error, Result};

/// The owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-direct-close-maker-v1";

/// The devnet command name.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-direct-close-maker-v1";

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-direct-close-maker-v1 --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON --market-input ABSOLUTE_JSON --campaign-evidence ABSOLUTE_JSON --direct-evidence ABSOLUTE_JSON --market MARKET --maker MAKER --evidence ABSOLUTE_NEW_JSON [--entry-index N] [--maker-replay ADDRESS] [--closer ADDRESS] [--execute --fee-payer-keypair ABSOLUTE_JSON]\n\
     dclutch-local-successor-bootstrap devnet-direct-close-maker-v1 --rpc-url URL --i-mean-devnet GENESIS_HASH --plan ABSOLUTE_JSON --market-input ABSOLUTE_JSON --campaign-evidence ABSOLUTE_JSON --direct-evidence ABSOLUTE_JSON --market MARKET --maker MAKER --evidence ABSOLUTE_NEW_JSON [--entry-index N] [--maker-replay ADDRESS] [--closer ADDRESS] [--execute --fee-payer-keypair ABSOLUTE_JSON]\n\
     \nCloses one Direct maker replay inside Retiring: wall 22's missing decrement, driven against a live cluster. It is permissionless -- no party to the market signs it, and the payer may be a stranger. Nothing economic is passed in: the beneficiary, the historical rent principal and the donation slice are read off the replay's own authenticated bytes, and the closer's SHARE of that donation and its lamport ceiling are read off the Custody-owned governed parameters record this frame carries, which is not passed in either. So a submission cannot move a lamport the market and the governed record did not already fix. The balance splits three ways: the recorded rent owner takes the principal exactly, the closer takes the carve, and every remaining lamport of the donation is credited to the upkeep vault. Without --execute this is a DRY RUN that opens no key and sends nothing, so it cannot know who the closer would be: name one with --closer. With --execute the closer IS the fee payer this tool opens, which is the one frame coordinate a payer may occupy, because it is the one that signs anyway. Either way this reports the exact instruction, the exact three-way split, and the exact poststate the close would produce. A replay that still owes its fee, or still has live intents, refuses here by name rather than on chain."
}

/// Parsed command line.
#[derive(Debug)]
struct ArgumentsV1 {
    rpc_url: String,
    market: Pubkey,
    maker: Pubkey,
    plan: PathBuf,
    market_input: PathBuf,
    campaign_evidence: PathBuf,
    direct_evidence: PathBuf,
    entry_index: u16,
    maker_replay: Option<Pubkey>,
    closer: Option<Pubkey>,
    fee_payer_keypair: Option<PathBuf>,
    evidence: PathBuf,
    execute: bool,
    acknowledgment: Option<String>,
}

/// Run one owned-loopback close.
pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    if arguments.acknowledgment.is_some() {
        return Err(Error::new(format!(
            "--i-mean-devnet belongs to {COMMAND_DEVNET_V1}, not to the owned-loopback arm"
        )));
    }
    let rpc = Rpc::connect(&arguments.rpc_url)?;
    close_v1(
        rpc,
        &arguments,
        DirectCloseMakerClusterV1::OwnedLoopback,
        "owned-loopback",
    )
}

/// Run one devnet close.
///
/// The route is permissionless by design, so the payer here is a stranger to
/// the market and signs nothing but the transaction fee.
pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    let acknowledgment = arguments.acknowledgment.as_deref().ok_or_else(|| {
        Error::new("--i-mean-devnet GENESIS_HASH is required to close against a public cluster")
    })?;
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, Some(acknowledgment))?;
    // ReadsOnly on a dry run is not decoration: it is what makes "opens no key
    // and sends nothing" a property of the transport rather than a promise this
    // function makes about itself.
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let rpc = Rpc::connect_cluster(&origin, policy)?;
    let label = origin.label().to_owned();
    close_v1(rpc, &arguments, DirectCloseMakerClusterV1::Devnet, &label)
}

/// Everything both arms do once an authenticated RPC exists.
fn close_v1(
    mut rpc: Rpc,
    arguments: &ArgumentsV1,
    cluster: DirectCloseMakerClusterV1,
    label: &str,
) -> Result<()> {
    let genesis = observed_genesis(&mut rpc)?;
    let source = authenticate_direct_close_source(&mut rpc, arguments, cluster)?;
    let coordinates = derive_coordinates(&mut rpc, arguments)?;
    authenticate_close_identity(
        &source,
        coordinates.market,
        coordinates.generation,
        coordinates.root,
        arguments.maker,
        coordinates.maker_replay,
    )?;

    // Read the replay first. A replay that is already gone is the ordinary
    // outcome of losing a permissionless race. The immutable Direct history
    // was authenticated first, so this recovery cannot silently cross a root
    // or generation merely because the replay account is now absent.
    let replay_account = rpc.account(coordinates.maker_replay)?;
    let Some(replay_data) = replay_account.as_ref().map(|account| account.data.clone()) else {
        println!("market               {}", arguments.market);
        println!("maker                {}", arguments.maker);
        println!("maker replay         {}", coordinates.maker_replay);
        println!("state                already closed; nothing to do");
        write_evidence(
            &arguments.evidence,
            None,
            &coordinates,
            &source,
            None,
            label,
        )?;
        return Ok(());
    };
    let replay = MakerReplayRootV1::decode(&replay_data).map_err(|error| {
        Error::new(format!(
            "maker replay {}: {error:?}",
            coordinates.maker_replay
        ))
    })?;
    let rent_owner = Pubkey::new_from_array(replay.rent_owner());

    // The closer is a frame member now, so it has to exist before anything is
    // gathered. Under `--execute` it is the payer this tool opens, which is the
    // one frame coordinate a payer may occupy: it signs the transaction anyway,
    // and coordinate 24 is the one coordinate the route asks a signature of. A
    // dry run opens no key, so it has none to name and takes `--closer`.
    let payer = match arguments.execute {
        true => {
            let path = arguments
                .fee_payer_keypair
                .as_deref()
                .ok_or_else(|| Error::new("--execute requires --fee-payer-keypair"))?;
            Some(Keypair::new_from_array(read_keypair_file(
                path,
                "close maker payer",
            )?))
        }
        false => None,
    };
    let closer = resolve_closer(arguments, payer.as_ref())?;
    if let Some(payer) = payer.as_ref() {
        refuse_payer_in_frame(
            &frame_keys(
                &coordinates,
                rent_owner,
                closer,
                coordinates.first_pass_caller_authority,
            ),
            payer.pubkey(),
        )?;
    }

    // TWO PASSES AT MOST, and the common path is ONE. Coordinate 26's last seed
    // is the digest of an upkeep request carrying the donation remainder and
    // the digest of this close's own receipt, so it is derivable only from a
    // completed plan. `InvalidCallerAuthority` carries the address the plan
    // wanted, which is how a builder becomes right without reimplementing
    // `assemble_plan`. When the remainder is zero -- every cohort to date,
    // because every measured donation slice has been zero -- the plan requires
    // nothing of that coordinate and the first pass stands.
    let first = gather(
        &mut rpc,
        arguments,
        &coordinates,
        rent_owner,
        closer,
        coordinates.first_pass_caller_authority,
        cluster,
        genesis,
    )?;
    let plan = match plan_direct_close_maker_v1(&first) {
        Ok(plan) => plan,
        Err(DirectCloseMakerPlanErrorV1::InvalidCallerAuthority(expected)) => {
            let second = gather(
                &mut rpc,
                arguments,
                &coordinates,
                rent_owner,
                closer,
                expected,
                cluster,
                genesis,
            )?;
            // A second disagreement is a real refusal, not a coordinate this
            // driver still has to learn: the plan already named the authority
            // once and was given it.
            plan_direct_close_maker_v1(&second).map_err(describe_refusal)?
        }
        Err(error) => return Err(describe_refusal(error)),
    };
    let report = match plan {
        DirectCloseMakerPlanV1::Complete(complete) => {
            println!("market               {}", complete.market);
            println!("maker replay         {}", complete.maker_replay);
            println!("state                already closed; nothing to do");
            write_evidence(
                &arguments.evidence,
                None,
                &coordinates,
                &source,
                None,
                label,
            )?;
            return Ok(());
        }
        DirectCloseMakerPlanV1::Submit(report) => report,
    };
    report_plan(&coordinates, &report);

    if !arguments.execute {
        write_evidence(
            &arguments.evidence,
            Some(&report),
            &coordinates,
            &source,
            None,
            label,
        )?;
        println!("dry run; no key was opened and nothing was sent");
        return Ok(());
    }

    let payer = payer.ok_or_else(|| Error::new("--execute requires --fee-payer-keypair"))?;
    println!("payer                {}", payer.pubkey());

    let evidence = rpc.send(
        "direct close maker",
        std::slice::from_ref(&report.instruction),
        &payer,
    )?;
    if let Some(error) = evidence.error.as_ref() {
        return Err(Error::new(format!("the close refused on chain: {error}")));
    }
    println!("signature            {}", evidence.signature);
    println!("slot                 {}", evidence.slot);
    println!(
        "compute units        {}",
        evidence
            .compute_units_consumed
            .map_or_else(|| "unreported".to_string(), |units| units.to_string())
    );

    // The chain is asked whether the replay is gone and whether the beneficiary
    // was credited, rather than the send being taken as proof of either. A
    // close that landed and left the replay standing would be the one failure
    // this driver must not report as success.
    let standing = rpc.account(coordinates.maker_replay)?;
    if standing.is_some_and(|account| account.lamports != 0 || !account.data.is_empty()) {
        return Err(Error::new(format!(
            "the close landed but the replay {} still stands",
            coordinates.maker_replay
        )));
    }
    println!("replay after         gone (read back from chain)");
    let credited = rpc
        .account(rent_owner)?
        .map_or(0, |account| account.lamports);
    if credited != report.expected_rent_owner_lamports {
        return Err(Error::new(format!(
            "the close landed but {rent_owner} holds {credited}, not the projected {}",
            report.expected_rent_owner_lamports
        )));
    }
    println!("beneficiary after    {credited} (read back from chain)");
    let housed = rpc
        .account(coordinates.upkeep_vault)?
        .map_or(0, |account| account.lamports);
    if housed != report.expected_upkeep_vault_lamports {
        return Err(Error::new(format!(
            "the close landed but the upkeep vault {} holds {housed}, not the projected {}",
            coordinates.upkeep_vault, report.expected_upkeep_vault_lamports
        )));
    }
    println!("upkeep vault after   {housed} (read back from chain)");
    // The closer's balance is deliberately NOT read back: under --execute the
    // closer is the fee payer, so its balance carries the transaction fee as
    // well as the carve, and the plan projects only the carve.

    write_evidence(
        &arguments.evidence,
        Some(&report),
        &coordinates,
        &source,
        Some(&evidence),
        label,
    )?;
    Ok(())
}

/// Turn a plan-time refusal into the sentence an operator can act on.
///
/// The two named ones carry their remedy, because both are reachable states of
/// a working market rather than defects: fee settlement is deliberately
/// phase-free, so settle-then-close is always available inside Retiring, and
/// older registered intents are permissionlessly closable once `cancel-through`
/// passes them.
fn describe_refusal(error: DirectCloseMakerPlanErrorV1) -> Error {
    match error {
        DirectCloseMakerPlanErrorV1::FeeOutstanding => Error::new(
            "this maker replay still owes its Direct fee, so the close refuses: the replay is the \
             SOLE record of that receivable and closing it would erase the debt with no residue. \
             Settle it first -- fee settlement is permissionless and phase-free, so \
             direct-fee-settlement-v1 works inside Retiring -- then close. On chain this is \
             CloseMakerFeeOutstanding (0x4011).",
        ),
        DirectCloseMakerPlanErrorV1::LiveIntents => Error::new(
            "this maker replay still has registered live intents, so the close refuses. Close \
             them first; older ones are permissionlessly closable once cancel-through passes \
             them. On chain this is CloseMakerLiveIntents (0x4012).",
        ),
        DirectCloseMakerPlanErrorV1::InvalidRootState => Error::new(
            "the Direct root is not Retiring, or its open-maker count is already drained. A close \
             runs INSIDE Retiring: begin-retiring must land first.",
        ),
        DirectCloseMakerPlanErrorV1::ClusterRefused => Error::new(
            "the observed genesis hash is not the cluster this arm closes against. Nothing was \
             read past the handshake.",
        ),
        other => Error::new(format!("the close plan refused: {other:?}")),
    }
}

/// Every account coordinate one close names, derived from the Market's own
/// state and the release the market plan witnesses.
struct CoordinatesV1 {
    market: Pubkey,
    generation: u64,
    root: Pubkey,
    registry: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
    trading_program: Pubkey,
    trading_programdata: Pubkey,
    activation_cache: Pubkey,
    manifest_raw: Pubkey,
    program_set: RecordPairV1,
    config: RecordPairV1,
    descriptor: RecordPairV1,
    account_profile: RecordPairV1,
    effect: RecordPairV1,
    rent_sysvar: Pubkey,
    maker_replay: Pubkey,
    /// The Custody program THIS Market's release set names.
    ///
    /// Read out of the activation cache beside Core and Trading rather than
    /// taken from a flag, because both Custody coordinates below are PDAs
    /// UNDER it: a wrong program here is a wrong answer about every economic
    /// number in the plan, and the plan stage refuses one that is not the
    /// role's ([`DirectCloseMakerPlanErrorV1::InvalidCustodyProgram`]).
    custody_program: Pubkey,
    /// The Custody-owned governed parameters record; the carve's share and its
    /// lamport ceiling are read here.
    protocol_parameters: Pubkey,
    /// The Custody-owned upkeep vault the donation remainder is credited to.
    upkeep_vault: Pubkey,
    /// The caller authority coordinate 26 carries on the FIRST pass.
    ///
    /// The real one is seeded by the digest of this close's own RECEIPT, which
    /// no gather can know, so the first pass names the authority seeded by the
    /// digest of its own REQUEST instead: same family, same derivation, and a
    /// digest no upkeep request for this close can ever have. A plan that needs
    /// the real one refuses with it attached and the second pass carries it; a
    /// plan with no donation remainder asks nothing of this coordinate at all.
    first_pass_caller_authority: Pubkey,
    ordinary_witness: dclutch_trading::ordinary_bundle_v4::DirectInlineOrdinaryHotBundleV4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectCloseSourceV1 {
    market: Pubkey,
    generation: u64,
    direct_root: Pubkey,
    maker: Pubkey,
    maker_replay: Pubkey,
    direct_evidence_sha256: [u8; 32],
}

fn authenticate_direct_close_source(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    cluster: DirectCloseMakerClusterV1,
) -> Result<DirectCloseSourceV1> {
    let expected_cluster = match cluster {
        DirectCloseMakerClusterV1::OwnedLoopback => ExpectedClusterV1::OwnedLoopback,
        DirectCloseMakerClusterV1::Devnet => ExpectedClusterV1::Devnet,
    };
    let plan = read_source(&arguments.plan, "successor plan")?;
    let market_input = read_source(&arguments.market_input, "Market input")?;
    let campaign_bytes = read_source(&arguments.campaign_evidence, "campaign evidence")?;
    let direct_bytes = read_source(&arguments.direct_evidence, "Direct finalized evidence")?;
    let campaign = parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_bytes,
        expected_cluster,
    )?;
    authenticate_plan_source(&plan, &campaign.plan_sha256)?;
    let plan_sha256 = sha256(&plan);
    let market_input_sha256 = sha256(&market_input);
    if market_input_sha256 != campaign.market_sha256 {
        return Err(Error::new(
            "Direct CloseMaker Market input changed from founding evidence",
        ));
    }
    let market = campaign
        .accounts
        .get("founding_market")
        .ok_or_else(|| Error::new("campaign evidence omitted founding_market"))?
        .address
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("campaign founding Market: {error}")))?;
    if market != arguments.market {
        return Err(Error::new(
            "Direct CloseMaker campaign evidence names another Market",
        ));
    }
    let terminal = match expected_cluster {
        ExpectedClusterV1::OwnedLoopback => authenticate_owned_loopback_terminal_evidence_v1(
            rpc,
            &arguments.direct_evidence,
            market,
            &plan_sha256,
            &market_input_sha256,
        )?,
        ExpectedClusterV1::Devnet => authenticate_devnet_terminal_evidence_v1(
            rpc,
            &arguments.direct_evidence,
            market,
            &plan_sha256,
            &market_input_sha256,
        )?,
    };
    if terminal.direct.market != market {
        return Err(Error::new(
            "Direct CloseMaker history and founding evidence name different Markets",
        ));
    }
    let generation = authenticated_direct_generation(&direct_bytes)?;
    let candidates = terminal
        .maker_replays
        .iter()
        .filter(|row| row.maker == arguments.maker)
        .collect::<Vec<_>>();
    let child = match candidates.as_slice() {
        [child] => *child,
        _ => {
            return Err(Error::new(
                "Direct CloseMaker maker has no unique replay in authenticated Direct history",
            ));
        }
    };
    if arguments
        .maker_replay
        .is_some_and(|named| named != child.replay)
    {
        return Err(Error::new(
            "--maker-replay differs from the authenticated Direct maker child",
        ));
    }
    if read_source(&arguments.direct_evidence, "Direct finalized evidence")? != direct_bytes {
        return Err(Error::new(
            "Direct finalized evidence changed while CloseMaker authenticated its history",
        ));
    }
    Ok(DirectCloseSourceV1 {
        market,
        generation,
        direct_root: terminal.direct_root,
        maker: child.maker,
        maker_replay: child.replay,
        direct_evidence_sha256: Sha256::digest(&direct_bytes).into(),
    })
}

fn authenticated_direct_generation(direct_evidence: &[u8]) -> Result<u64> {
    let evidence = parse_json_without_duplicate_keys_v1(direct_evidence)?;
    let encoded = evidence
        .get("publicManifestBase64")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::new("Direct evidence omitted its public manifest"))?;
    let public = BASE64
        .decode(encoded)
        .map_err(|error| Error::new(format!("Direct public manifest base64: {error}")))?;
    if BASE64.encode(&public) != encoded {
        return Err(Error::new("Direct public manifest base64 is noncanonical"));
    }
    let public = parse_json_without_duplicate_keys_v1(&public)?;
    public
        .get("context")
        .and_then(Value::as_object)
        .and_then(|context| context.get("generation"))
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("Direct public manifest omitted exact generation"))
}

fn authenticate_close_identity(
    source: &DirectCloseSourceV1,
    market: Pubkey,
    generation: u64,
    direct_root: Pubkey,
    maker: Pubkey,
    maker_replay: Pubkey,
) -> Result<()> {
    if source.market != market
        || source.generation != generation
        || source.direct_root != direct_root
        || source.maker != maker
        || source.maker_replay != maker_replay
    {
        return Err(Error::new(
            "CloseMaker coordinates differ from authenticated Direct Market, generation, root, maker, or replay",
        ));
    }
    Ok(())
}

fn read_source(path: &Path, label: &str) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path)
        .map_err(|error| Error::new(format!("{label} {}: {error}", path.display())))?;
    if bytes.is_empty() {
        return Err(Error::new(format!("{label} is empty")));
    }
    Ok(bytes)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[derive(Clone, Copy)]
struct RecordPairV1 {
    raw: Pubkey,
    staging: Pubkey,
}

/// One record address from the contract-owned seed material.
///
/// The seed TUPLE is not spelled in this driver. `dclutch-registry::record` owns
/// both domains and exports the constructors that place them, so a second
/// spelling here would be a second source of truth for an address the chain
/// derives its own way (`DOMAIN_RAW_RESTATEMENT`).
fn record_address(seeds: RecordPdaSeedsV1, registry: Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        &registry,
    )
    .0
}

fn record_key(schema: [u8; 32], digest: [u8; 32]) -> Result<RecordKeyV1> {
    Ok(RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|error| Error::new(format!("record schema identity: {error:?}")))?,
        ContentDigest::new(digest)
            .map_err(|error| Error::new(format!("record content digest: {error:?}")))?,
    ))
}

fn record_raw(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> Result<Pubkey> {
    Ok(record_address(
        record_key(schema, digest)?.raw_record_pda_seeds(),
        registry,
    ))
}

fn record_pair(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> Result<RecordPairV1> {
    let key = record_key(schema, digest)?;
    Ok(RecordPairV1 {
        raw: record_address(key.raw_record_pda_seeds(), registry),
        staging: record_address(key.staging_cursor_pda_seeds(), registry),
    })
}

/// Derive the whole frame from the Market, the manifest it selects, and the
/// release the market plan witnesses.
///
/// Nothing here is believed by the plan builder: it re-derives every one of
/// these coordinates from the same chain state and refuses if any differs. This
/// exists so an operator names a market and a maker rather than a whole frame
/// of addresses. Two of them are not derivable from the Market alone: the
/// closer is a party rather than a coordinate, and the caller authority is
/// seeded from a plan that has not run yet, so both arrive from elsewhere.
fn derive_coordinates(rpc: &mut Rpc, arguments: &ArgumentsV1) -> Result<CoordinatesV1> {
    let market_account = rpc.required_account(arguments.market, "Core Market")?;
    let market = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market {}: {error:?}", arguments.market)))?;
    let core_program = market_account.owner;
    let registry = Pubkey::new_from_array(market.identity.registry_program.to_bytes());
    let generation = market.identity.generation;
    let release_set = market.identity.selected_release_set.to_bytes();

    // The activation cache is the Market's own statement of which programs its
    // release selected, so the Core and Trading identities come from the chain
    // rather than from a flag.
    let activation_cache =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &registry).0;
    let cache = rpc.required_account(activation_cache, "Registry activation cache")?;
    let activated = ActivatedExecutionReleaseSetViewV1::decode(&cache.data)
        .map_err(|error| Error::new(format!("activation cache: {error:?}")))?;
    let role = |role: ExecutionRoleV1, label: &str| -> Result<(Pubkey, Pubkey)> {
        let selected = activated
            .role(role)
            .map_err(|error| Error::new(format!("{label} role: {error:?}")))?;
        let release = selected.release();
        Ok((
            Pubkey::new_from_array(release.program().to_bytes()),
            Pubkey::new_from_array(release.programdata()),
        ))
    };
    let (core_selected, core_programdata) = role(ExecutionRoleV1::Core, "Core")?;
    let (trading_program, trading_programdata) = role(ExecutionRoleV1::Trading, "Trading")?;
    // The same authority the Core and Trading identities come from, for the
    // same reason: the release set is the Market's own statement of which
    // programs it selected. Custody's ProgramData is not a frame member, so
    // only the program identity is taken.
    let (custody_program, _) = role(ExecutionRoleV1::Custody, "Custody")?;
    if core_selected != core_program {
        return Err(Error::new(format!(
            "the Market at {} is owned by {core_program}, but its release selects Core {core_selected}",
            arguments.market
        )));
    }

    let manifest_digest = market.identity.capability_manifest.to_bytes();
    let manifest_raw = record_raw(
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_digest,
    )?;
    let manifest_account = rpc.required_account(manifest_raw, "capability manifest")?;
    let manifest = CapabilityManifestV1::decode(&manifest_account.data)
        .map_err(|error| Error::new(format!("capability manifest: {error:?}")))?;
    let entry = manifest.entry(arguments.entry_index).map_err(|error| {
        Error::new(format!(
            "manifest entry {}: {error:?}",
            arguments.entry_index
        ))
    })?;

    // The release is REGENERATED from the ordinary witness the market plan
    // carries, exactly as the plan builder regenerates it. That is what makes
    // the close artifacts derived rather than named: a substituted document
    // produces a different ProgramSet identity and the builder refuses.
    let ordinary_witness = ordinary_witness(&arguments.market_input)?;
    let release = build_direct_inline_ordinary_lifecycle_program_set_v1(
        ordinary_witness,
        entry.capacity_profile_id().to_bytes(),
    )
    .map_err(|error| {
        Error::new(format!(
            "regenerate the Direct lifecycle release: {error:?}"
        ))
    })?;
    if release.program_set_id != entry.release_id().to_bytes() {
        return Err(Error::new(
            "the market plan's ordinary witness does not regenerate the release this Market \
             selected; the plan is for another market or another release",
        ));
    }

    let selection = CapabilityExecutionSelectionV1::new(
        arguments.entry_index,
        ContentId::new(manifest_digest)
            .map_err(|error| Error::new(format!("manifest identity: {error:?}")))?,
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
    )
    .map_err(|error| Error::new(format!("execution selection: {error:?}")))?;
    // The record bumps are NOT root seeds -- `CapabilityRootSeedsV1` reads only
    // the market, generation, manifest, entry index, kind, release and config --
    // so any well-formed placeholder derives the same address. The real header
    // is read back off the root account and re-authenticated by the plan
    // builder, which is the only authority on it.
    let header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set)
            .map_err(|error| Error::new(format!("release set: {error:?}")))?,
        arguments.market.to_bytes(),
        generation,
        selection,
        SelectedRecordBumpsV1::new(255, 255, 255, 255),
    )
    .map_err(|error| Error::new(format!("root header projection: {error:?}")))?;
    let root = Pubkey::find_program_address(&header.seeds().as_slices(), &trading_program).0;

    let digest = |bytes: &[u8]| solana_program::hash::hash(bytes).to_bytes();
    let maker_replay = match arguments.maker_replay {
        Some(named) => named,
        None => {
            let coordinates = DirectCoordinatesV1::new(arguments.market.to_bytes(), generation)
                .map_err(|error| Error::new(format!("replay coordinates: {error:?}")))?;
            let seeds = MakerReplaySeedsV1::new(coordinates, arguments.maker.to_bytes())
                .map_err(|error| Error::new(format!("replay seeds: {error:?}")))?;
            Pubkey::find_program_address(&seeds.as_slices(), &trading_program).0
        }
    };

    // Both Custody coordinates are one per DEPLOYMENT rather than one per
    // market: their seed tuples are a domain and nothing else, so naming the
    // Custody program is naming them. The seeds are borrowed from the contracts
    // that own them rather than respelled here.
    let protocol_parameters = Pubkey::find_program_address(
        &ProtocolParametersRecordSeedsV1.as_slices(),
        &custody_program,
    )
    .0;
    let upkeep_vault =
        Pubkey::find_program_address(&UpkeepVaultSeedsV1.as_slices(), &custody_program).0;
    let request_body = DirectCloseMakerRequestV1 {
        market: arguments.market.to_bytes(),
        maker: arguments.maker.to_bytes(),
        generation,
    }
    .to_bytes()
    .map_err(|error| Error::new(format!("canonical close request: {error:?}")))?;
    let first_pass_caller_authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            release_set,
            arguments.market.to_bytes(),
            ExecutionRoleV1::Trading,
            maker_replay.to_bytes(),
            digest(&request_body),
        )
        .map_err(|error| Error::new(format!("first-pass caller authority: {error:?}")))?
        .as_slices(),
        &trading_program,
    )
    .0;

    Ok(CoordinatesV1 {
        market: arguments.market,
        generation,
        root,
        registry,
        core_program,
        core_programdata,
        trading_program,
        trading_programdata,
        activation_cache,
        manifest_raw,
        program_set: record_pair(
            registry,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            entry.release_id().to_bytes(),
        )?,
        config: record_pair(
            registry,
            DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
            entry.config_id().to_bytes(),
        )?,
        descriptor: record_pair(
            registry,
            direct_close_maker_descriptor_schema_v1(),
            digest(&release.close_maker.descriptor),
        )?,
        account_profile: record_pair(
            registry,
            direct_close_maker_account_profile_schema_v1(),
            digest(&release.close_maker.account_profile),
        )?,
        effect: record_pair(
            registry,
            direct_close_maker_effect_schema_v1(),
            digest(&release.close_maker.effect),
        )?,
        rent_sysvar: solana_sdk_ids::sysvar::rent::ID,
        maker_replay,
        custody_program,
        protocol_parameters,
        upkeep_vault,
        first_pass_caller_authority,
        ordinary_witness,
    })
}

/// The exact ordered frame this close names, coordinate by coordinate.
///
/// One author for the key order, so `gather` and the fee-payer census cannot
/// disagree about which coordinate is which. The width is the codec's own
/// constant, so a frame that grows cannot leave this function a coordinate
/// short and still compile.
fn frame_keys(
    coordinates: &CoordinatesV1,
    rent_owner: Pubkey,
    closer: Pubkey,
    caller_authority: Pubkey,
) -> [Pubkey; DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1] {
    [
        coordinates.root,
        coordinates.market,
        coordinates.manifest_raw,
        coordinates.program_set.raw,
        coordinates.program_set.staging,
        coordinates.descriptor.raw,
        coordinates.descriptor.staging,
        coordinates.config.raw,
        coordinates.config.staging,
        coordinates.account_profile.raw,
        coordinates.account_profile.staging,
        coordinates.effect.raw,
        coordinates.effect.staging,
        coordinates.activation_cache,
        coordinates.core_program,
        coordinates.core_programdata,
        coordinates.trading_program,
        coordinates.trading_programdata,
        coordinates.registry,
        coordinates.rent_sysvar,
        coordinates.maker_replay,
        rent_owner,
        coordinates.protocol_parameters,
        coordinates.upkeep_vault,
        closer,
        coordinates.custody_program,
        caller_authority,
    ]
}

/// Whose the carve is, under each arm.
///
/// A dry run opens no key, so it cannot know a closer and is told one. Under
/// `--execute` the payer IS the closer, and a `--closer` that disagrees with
/// the key actually being opened is refused rather than silently ignored: the
/// carve would land somewhere other than where the operator was told.
fn resolve_closer(arguments: &ArgumentsV1, payer: Option<&Keypair>) -> Result<Pubkey> {
    match payer {
        Some(payer) => {
            let payer = payer.pubkey();
            if arguments.closer.is_some_and(|named| named != payer) {
                return Err(Error::new(format!(
                    "--closer names another account, but --execute pays and signs from \
                     {payer}, which is the closer this close would credit. Drop --closer, or \
                     open the key you meant to pay the carve to."
                )));
            }
            Ok(payer)
        }
        None => arguments.closer.ok_or_else(|| {
            Error::new(
                "--closer ADDRESS is required on a dry run: the close frame now carries a \
                 closer at coordinate 24, it is paid the carve out of the donation, and a run \
                 that opens no key cannot know whose it would be. Under --execute the fee payer \
                 is the closer and --closer is not needed.",
            )
        }),
    }
}

/// Read the whole close frame at one finalized observation.
#[allow(clippy::too_many_arguments)]
fn gather(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    coordinates: &CoordinatesV1,
    rent_owner: Pubkey,
    closer: Pubkey,
    caller_authority: Pubkey,
    cluster: DirectCloseMakerClusterV1,
    genesis_hash: [u8; 32],
) -> Result<DirectCloseMakerSnapshotV1> {
    let keys = frame_keys(coordinates, rent_owner, closer, caller_authority);
    if keys.len() != DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1 {
        // The width is stated by the codec constant rather than spelled in
        // English, so a frame that grows again cannot leave a sentence behind
        // saying how wide it used to be.
        return Err(Error::new(format!(
            "the close frame is not {DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1} accounts"
        )));
    }
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let at = |index: usize| -> Result<dclutch_operator::ObservedAccount> {
        let key = keys[index];
        // The snapshot's refusals now come from the extracted payout crate;
        // carry them into this binary's error type unchanged.
        snapshot.account(key).cloned().map_err(Into::into)
    };
    Ok(DirectCloseMakerSnapshotV1 {
        cluster,
        genesis_hash,
        ordinary_release_witness: coordinates.ordinary_witness,
        root: at(0)?,
        market: at(1)?,
        capability_manifest: at(2)?,
        program_set: at(3)?,
        program_set_staging: at(4)?,
        descriptor: at(5)?,
        descriptor_staging: at(6)?,
        config: at(7)?,
        config_staging: at(8)?,
        account_profile: at(9)?,
        account_profile_staging: at(10)?,
        effect: at(11)?,
        effect_staging: at(12)?,
        activation_cache: at(13)?,
        core_program: at(14)?,
        core_programdata: at(15)?,
        trading_program: at(16)?,
        trading_programdata: at(17)?,
        registry_program: at(18)?,
        rent_sysvar: at(19)?,
        maker: arguments.maker,
        maker_replay: at(DIRECT_CLOSE_MAKER_REPLAY_ACCOUNT_V1)?,
        rent_owner: at(DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1)?,
        protocol_parameters: at(DIRECT_CLOSE_MAKER_PROTOCOL_PARAMETERS_ACCOUNT_V1)?,
        upkeep_vault: at(DIRECT_CLOSE_MAKER_UPKEEP_VAULT_ACCOUNT_V1)?,
        closer: at(DIRECT_CLOSE_MAKER_CLOSER_ACCOUNT_V1)?,
        custody_program: at(DIRECT_CLOSE_MAKER_CUSTODY_PROGRAM_ACCOUNT_V1)?,
        caller_authority: at(DIRECT_CLOSE_MAKER_CALLER_AUTHORITY_ACCOUNT_V1)?,
    })
}

/// Refuse a fee payer this close's own frame names anywhere but as the closer.
///
/// The route admits EXACTLY ONE signer -- the closer at coordinate 24 -- and
/// pins exact writability on every coordinate, and BOTH are transaction-level
/// properties rather than per-instruction ones: a fee payer signs the
/// transaction and is written for the fee whatever `AccountMeta` it was given,
/// so an `AccountInfo` the route reads back reports `is_signer` and
/// `is_writable` true for it. That is why paying from the closer is the one
/// collision that WORKS -- coordinate 24 signs and is written anyway, which is
/// exactly what a payer does -- and why paying from any other coordinate
/// refuses on chain as `CloseMakerCloser` or `CloseMakerFrame`, with nothing in
/// the message to say why.
///
/// The rent owner is the collision an operator will actually reach for: a maker
/// closing their own replay and receiving their own rent is the obvious way to
/// do it, and it is the one way that cannot work. So it refuses here, naming the
/// coordinate, rather than letting the cut read a frame refusal off a failed
/// transaction.
fn refuse_payer_in_frame(
    keys: &[Pubkey; DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1],
    payer: Pubkey,
) -> Result<()> {
    match keys.iter().position(|key| *key == payer) {
        None | Some(DIRECT_CLOSE_MAKER_CLOSER_ACCOUNT_V1) => Ok(()),
        Some(index) => Err(Error::new(format!(
            "the fee payer {payer} is coordinate {index} of this close's own frame. The route \
             admits one signer, the closer at coordinate {DIRECT_CLOSE_MAKER_CLOSER_ACCOUNT_V1}, \
             and a fee payer signs whatever meta it carries, so this would refuse on chain as \
             CloseMakerCloser or CloseMakerFrame. Pay from an account this close does not name -- \
             it is permissionless, so any funded stranger will do -- or pay from the closer, \
             which is the coordinate a signature belongs to."
        ))),
    }
}

fn report_plan(coordinates: &CoordinatesV1, report: &DirectCloseMakerSubmitV1) {
    let receipt = report.expected_receipt;
    println!("== the close, read off chain ==");
    println!("market               {}", coordinates.market);
    println!("generation           {}", coordinates.generation);
    println!("Direct root          {}", coordinates.root);
    println!(
        "maker                {}",
        Pubkey::new_from_array(receipt.maker)
    );
    println!("maker replay         {}", coordinates.maker_replay);
    println!(
        "rent owner           {}",
        Pubkey::new_from_array(receipt.rent_owner)
    );
    println!("rent principal       {}", report.rent_principal);
    println!("donation             {}", report.unclassified_donation);
    println!(
        "closer               {}",
        Pubkey::new_from_array(receipt.closer)
    );
    println!("upkeep vault         {}", coordinates.upkeep_vault);
    // The three destinations, each beside the number it receives. `total_credit`
    // is the OWNER's credit and it is the principal exactly -- never a lamport
    // of the donation -- so it is printed under the owner's name and not as a
    // total of anything.
    println!("-> owner  (principal){:>12}", report.total_credit);
    println!("-> closer (carve)    {:>12}", report.closer_reward);
    println!("-> vault  (remainder){:>12}", report.upkeep_credit);
    println!(
        "beneficiary after    {}",
        report.expected_rent_owner_lamports
    );
    println!("closer after         {}", report.expected_closer_lamports);
    println!(
        "upkeep vault after   {}",
        report.expected_upkeep_vault_lamports
    );
    println!(
        "open maker roots     {} -> {}",
        report.expected_remaining_open_maker_roots.saturating_add(1),
        report.expected_remaining_open_maker_roots
    );
    println!("accounts             {}", report.instruction.accounts.len());
    println!(
        "signers              {}",
        report
            .instruction
            .accounts
            .iter()
            .filter(|meta| meta.is_signer)
            .count()
    );
}

fn observed_genesis(rpc: &mut Rpc) -> Result<[u8; 32]> {
    let genesis = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| Error::new("getGenesisHash result was not a string"))?
        .to_owned();
    let parsed = genesis
        .parse::<solana_sdk::hash::Hash>()
        .map_err(|error| Error::new(format!("getGenesisHash: {error}")))?;
    Ok(parsed.to_bytes())
}

fn ordinary_witness(
    path: &Path,
) -> Result<dclutch_trading::ordinary_bundle_v4::DirectInlineOrdinaryHotBundleV4> {
    let bytes =
        std::fs::read(path).map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
    let input: MarketRunInput = serde_json::from_slice(&bytes)
        .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
    let direct = input
        .direct_capability
        .as_ref()
        .ok_or_else(|| Error::new("the market plan omits its Direct capability payload"))?;
    crate::direct_market::direct_ordinary_bundle_v1(direct)
}

/// Write what this close was, whether or not it was sent.
fn write_evidence(
    path: &Path,
    report: Option<&DirectCloseMakerSubmitV1>,
    coordinates: &CoordinatesV1,
    source: &DirectCloseSourceV1,
    landed: Option<&crate::model::TransactionEvidence>,
    cluster: &str,
) -> Result<()> {
    let document = json!({
        "schema": "dclutch-direct-close-maker-evidence-v1",
        "cluster": cluster,
        "market": coordinates.market.to_string(),
        "generation": coordinates.generation,
        "directRoot": coordinates.root.to_string(),
        "directEvidenceSha256": crate::plan::hex(&source.direct_evidence_sha256),
        "makerReplay": coordinates.maker_replay.to_string(),
        "plan": report.map(|report| json!({
            "maker": Pubkey::new_from_array(report.expected_receipt.maker).to_string(),
            "rentOwner": Pubkey::new_from_array(report.expected_receipt.rent_owner).to_string(),
            "closer": Pubkey::new_from_array(report.expected_receipt.closer).to_string(),
            "upkeepVault": coordinates.upkeep_vault.to_string(),
            "protocolParameters": coordinates.protocol_parameters.to_string(),
            "custodyProgram": coordinates.custody_program.to_string(),
            "rentPrincipal": report.rent_principal,
            "unclassifiedDonation": report.unclassified_donation,
            "closerReward": report.closer_reward,
            "upkeepCredit": report.upkeep_credit,
            "totalCredit": report.total_credit,
            "beneficiaryLamportsAfter": report.expected_rent_owner_lamports,
            "closerLamportsAfter": report.expected_closer_lamports,
            "upkeepVaultLamportsAfter": report.expected_upkeep_vault_lamports,
            "remainingOpenMakerRoots": report.expected_remaining_open_maker_roots,
            "requestDigest": crate::plan::hex(&report.request_digest),
            "expectedPostRootDigest": crate::plan::hex(&report.expected_post_root_digest),
            "expectedReceipt": crate::plan::hex(&report.expected_receipt_body),
        })),
        "alreadyClosed": report.is_none(),
        "landed": landed.map(|evidence| json!({
            "signature": evidence.signature,
            "slot": evidence.slot,
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
    let mut market = None;
    let mut maker = None;
    let mut plan = None;
    let mut market_input = None;
    let mut campaign_evidence = None;
    let mut direct_evidence = None;
    let mut entry_index = 0_u16;
    let mut maker_replay = None;
    let mut closer = None;
    let mut fee_payer_keypair = None;
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
            "--market" => {
                market = Some(
                    value()?
                        .parse::<Pubkey>()
                        .map_err(|error| Error::new(format!("--market: {error}")))?,
                );
            }
            "--maker" => {
                maker = Some(
                    value()?
                        .parse::<Pubkey>()
                        .map_err(|error| Error::new(format!("--maker: {error}")))?,
                );
            }
            "--maker-replay" => {
                maker_replay = Some(
                    value()?
                        .parse::<Pubkey>()
                        .map_err(|error| Error::new(format!("--maker-replay: {error}")))?,
                );
            }
            "--closer" => {
                closer = Some(
                    value()?
                        .parse::<Pubkey>()
                        .map_err(|error| Error::new(format!("--closer: {error}")))?,
                );
            }
            "--plan" => plan = Some(PathBuf::from(value()?)),
            "--market-input" => market_input = Some(PathBuf::from(value()?)),
            "--campaign-evidence" => campaign_evidence = Some(PathBuf::from(value()?)),
            "--direct-evidence" => direct_evidence = Some(PathBuf::from(value()?)),
            "--entry-index" => {
                entry_index = value()?
                    .parse::<u16>()
                    .map_err(|error| Error::new(format!("--entry-index: {error}")))?;
            }
            "--fee-payer-keypair" => fee_payer_keypair = Some(PathBuf::from(value()?)),
            "--evidence" => evidence = Some(PathBuf::from(value()?)),
            "--i-mean-devnet" => acknowledgment = Some(value()?),
            "--execute" => execute = true,
            other => return Err(Error::new(format!("unknown flag: {other}"))),
        }
    }
    Ok(ArgumentsV1 {
        rpc_url: rpc_url.ok_or_else(|| Error::new("--rpc-url is required"))?,
        market: market.ok_or_else(|| Error::new("--market is required"))?,
        maker: maker.ok_or_else(|| Error::new("--maker is required"))?,
        plan: plan.ok_or_else(|| Error::new("--plan is required"))?,
        market_input: market_input.ok_or_else(|| Error::new("--market-input is required"))?,
        campaign_evidence: campaign_evidence
            .ok_or_else(|| Error::new("--campaign-evidence is required"))?,
        direct_evidence: direct_evidence
            .ok_or_else(|| Error::new("--direct-evidence is required"))?,
        entry_index,
        maker_replay,
        closer,
        fee_payer_keypair,
        evidence: evidence.ok_or_else(|| Error::new("--evidence is required"))?,
        execute,
        acknowledgment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(extra: &[&str]) -> Vec<String> {
        let mut v = vec![
            "--rpc-url".into(),
            "http://127.0.0.1:8899".into(),
            "--market".into(),
            "11111111111111111111111111111111".into(),
            "--maker".into(),
            "11111111111111111111111111111111".into(),
            "--plan".into(),
            "/nonexistent/successor-plan.json".into(),
            "--market-input".into(),
            "/nonexistent/plan.json".into(),
            "--campaign-evidence".into(),
            "/nonexistent/campaign.json".into(),
            "--direct-evidence".into(),
            "/nonexistent/direct.json".into(),
            "--evidence".into(),
            "/nonexistent/evidence.json".into(),
        ];
        v.extend(extra.iter().map(|s| (*s).to_string()));
        v
    }

    /// Each arm refuses the other's cluster contract BEFORE it opens a socket,
    /// a file, or a key.
    ///
    /// The two arms differ in exactly one thing -- how the RPC origin is
    /// established -- so the way they could go wrong is a caller reaching a
    /// public cluster through the arm that never authenticates a genesis hash,
    /// or reaching loopback through the arm that demands one. Both refusals
    /// happen during argument parsing, which is why this test needs no chain.
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

    /// The required coordinates are required, and the optional ones are
    /// optional. A close that silently defaulted a market or a maker would
    /// address somebody else's replay.
    #[test]
    fn the_market_and_maker_are_required_and_the_entry_index_defaults() {
        let parsed = parse(args(&[])).expect("well formed arguments");
        assert_eq!(parsed.entry_index, 0);
        assert!(parsed.maker_replay.is_none());
        assert!(!parsed.execute);

        for missing in [
            "--market",
            "--maker",
            "--plan",
            "--market-input",
            "--campaign-evidence",
            "--direct-evidence",
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

    /// Both named refusals carry their remedy, because both are reachable
    /// states of a working market rather than defects.
    #[test]
    fn the_two_named_refusals_tell_an_operator_what_to_do_next() {
        let fee = describe_refusal(DirectCloseMakerPlanErrorV1::FeeOutstanding);
        let fee = format!("{fee:?}");
        assert!(fee.contains("0x4011"), "{fee}");
        assert!(fee.contains("settlement"), "{fee}");

        let live = describe_refusal(DirectCloseMakerPlanErrorV1::LiveIntents);
        let live = format!("{live:?}");
        assert!(live.contains("0x4012"), "{live}");
        assert!(live.contains("cancel-through"), "{live}");
    }

    /// A fee payer the close already names is refused before anything is sent,
    /// and the closer -- the one coordinate that signs anyway -- is not.
    ///
    /// This is the seam the static audit pointed at: the route's signer census
    /// and writability pins are transaction-level, so the one obvious way to
    /// run this close -- pay from the wallet that receives the rent -- is the
    /// one way that cannot work. It must refuse here, naming the coordinate.
    /// The frame's ONE admitted signer is the other half of the same fact: a
    /// payer that is the closer carries exactly the privileges coordinate 24
    /// already asks for, so refusing it would refuse the arm this tool takes
    /// under `--execute`.
    #[test]
    fn a_fee_payer_the_frame_names_is_refused_unless_it_is_the_closer() {
        let mut keys = [Pubkey::default(); DIRECT_CLOSE_MAKER_ACCOUNT_COUNT_V1];
        for (index, key) in keys.iter_mut().enumerate() {
            let seed = u8::try_from(index).expect("frame index fits a byte");
            *key = Pubkey::new_from_array([seed + 1; 32]);
        }
        let stranger = Pubkey::new_from_array([0xfe; 32]);
        let closer = keys[DIRECT_CLOSE_MAKER_CLOSER_ACCOUNT_V1];
        let rent_owner = keys[DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1];

        refuse_payer_in_frame(&keys, stranger).expect("a stranger may always pay");
        refuse_payer_in_frame(&keys, closer)
            .expect("the closer signs and is written anyway, so it may also pay");

        let refusal = refuse_payer_in_frame(&keys, rent_owner)
            .expect_err("the beneficiary must not pay for its own close");
        let refusal = format!("{refusal:?}");
        assert!(
            refusal.contains(&format!(
                "coordinate {DIRECT_CLOSE_MAKER_RENT_OWNER_ACCOUNT_V1}"
            )),
            "{refusal}"
        );
        assert!(refusal.contains("CloseMakerCloser"), "{refusal}");
    }

    /// The carve has a destination under both arms, and the arms cannot
    /// disagree about it.
    ///
    /// A dry run opens no key, so it must be told one; `--execute` opens the
    /// payer and that key IS the closer. A `--closer` naming a third account
    /// under `--execute` would promise the carve to somewhere the transaction
    /// cannot send it, so it refuses rather than being ignored.
    #[test]
    fn the_closer_is_named_on_a_dry_run_and_is_the_payer_under_execute() {
        let named = Pubkey::new_from_array([9; 32]);
        let dry = parse(args(&["--closer", &named.to_string()])).expect("well formed arguments");
        assert_eq!(resolve_closer(&dry, None).expect("a named closer"), named);

        let refusal = resolve_closer(&parse(args(&[])).expect("well formed arguments"), None)
            .expect_err("a dry run with no closer must refuse");
        assert!(format!("{refusal:?}").contains("--closer"), "{refusal:?}");

        let payer = Keypair::new();
        assert_eq!(
            resolve_closer(
                &parse(args(&[])).expect("well formed arguments"),
                Some(&payer)
            )
            .expect("the payer is the closer"),
            payer.pubkey(),
        );
        let refusal = resolve_closer(&dry, Some(&payer))
            .expect_err("a --closer that is not the opened payer must refuse");
        assert!(
            format!("{refusal:?}").contains("--closer names another"),
            "{refusal:?}"
        );
    }

    #[test]
    fn close_and_already_closed_recovery_bind_exact_direct_identity() {
        let source = DirectCloseSourceV1 {
            market: Pubkey::new_unique(),
            generation: 9,
            direct_root: Pubkey::new_unique(),
            maker: Pubkey::new_unique(),
            maker_replay: Pubkey::new_unique(),
            direct_evidence_sha256: [7; 32],
        };
        authenticate_close_identity(
            &source,
            source.market,
            source.generation,
            source.direct_root,
            source.maker,
            source.maker_replay,
        )
        .expect("exact Direct child may close or recover as already closed");

        let substitutions = [
            (
                Pubkey::new_unique(),
                source.generation,
                source.direct_root,
                source.maker,
                source.maker_replay,
            ),
            (
                source.market,
                source.generation + 1,
                source.direct_root,
                source.maker,
                source.maker_replay,
            ),
            (
                source.market,
                source.generation,
                Pubkey::new_unique(),
                source.maker,
                source.maker_replay,
            ),
            (
                source.market,
                source.generation,
                source.direct_root,
                Pubkey::new_unique(),
                source.maker_replay,
            ),
            (
                source.market,
                source.generation,
                source.direct_root,
                source.maker,
                Pubkey::new_unique(),
            ),
        ];
        for (market, generation, root, maker, replay) in substitutions {
            assert!(
                authenticate_close_identity(&source, market, generation, root, maker, replay)
                    .is_err(),
                "a substituted Direct identity must refuse before the replay existence check",
            );
        }
    }

    #[test]
    fn direct_generation_is_read_only_from_canonical_embedded_manifest() {
        let public = serde_json::to_vec(&json!({"context": {"generation": 17}})).unwrap();
        let evidence = serde_json::to_vec(&json!({
            "publicManifestBase64": BASE64.encode(&public),
        }))
        .unwrap();
        assert_eq!(authenticated_direct_generation(&evidence).unwrap(), 17);

        let noncanonical = br#"{"publicManifestBase64":"e30"}"#;
        assert!(authenticated_direct_generation(noncanonical).is_err());
        let missing = serde_json::to_vec(&json!({
            "publicManifestBase64": BASE64.encode(b"{}"),
        }))
        .unwrap();
        assert!(authenticated_direct_generation(&missing).is_err());
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
