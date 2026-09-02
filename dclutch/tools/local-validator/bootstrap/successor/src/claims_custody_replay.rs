//! First-use creation of a Market's Claims-role Custody replay, driven against
//! a validator this runner owns.
//!
//! # Why this exists
//!
//! `programs/dclutch-claims-sbf/src/custody_replay_v1.rs` is a dedicated
//! first-use creation route — *"Only the Claims program can produce a
//! Claims-role caller authority, so only the Claims program can create the
//! Claims-role replay."* Until this module, nothing under `tools/` built its
//! instruction: the only two builders in the whole tree were program tests, so
//! the route could be reached from a bank and from nowhere else.
//!
//! That gap is not cosmetic. Terminal payout — redemption — decodes the
//! Claims-role replay and deliberately does not create it: creation is never a
//! side effect of a payout (`terminal_settlement_v3`). So every wallet holding
//! a position in a resolved market was behind a route with no caller, and
//! `EVIDENCE_REFRESH_V1` §7.14.4 recorded exactly that: the replay *"does not
//! exist and nothing under `tools/` drives the route that creates it."*
//!
//! # What this reads and what it refuses to invent
//!
//! The wire carries a Market coordinate and nothing else — 48 bytes, of which
//! 32 are the Market. So neither does this driver. Every economic and
//! namespacing fact comes off the Claims aggregate the derivation admits:
//! release set, Realm, generation, and the custody context. The rent refund is
//! Core's own `rent_beneficiary`, never the payer's choice, and the rent is the
//! Rent sysvar's minimum balance for the replay's exact width.
//!
//! # The one agreement that has to hold exactly
//!
//! The Claims caller authority's fifth seed is the digest of the forwarded
//! Custody request bytes, so a driver that rebuilt that request separately
//! would address a PDA nothing can sign the instant the two drifted by a byte.
//! This module therefore calls the program's own `expected_request_v1` — the
//! single author the program itself uses — rather than restating it. That is
//! §7.14's ruling on `caller_authority`, which reproduced a failed packet's
//! static key byte for byte by calling the builder instead of reimplementing
//! it.
//!
//! # On the custody context
//!
//! Note what this driver does NOT take: a custody context. The aggregate's own
//! persisted `custody_context` is the authority (Decision 0008 §1 — *"the
//! aggregate is the sole persisted owner of this Market's Custody namespace,
//! and no route may re-guess it"*), and it is the chain-persisted projected-hoard
//! digest, not the founding campaign's pre-image. A driver that took the
//! context from campaign evidence would have addressed an empty universe; see
//! `EVIDENCE_REFRESH_V1` §7.14.4 and the refresh's
//! `chain_persisted_custody_context`.

use std::path::{Path, PathBuf};

use serde_json::json;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
};
use solana_sdk_ids::{system_program, sysvar};

use dclutch_claims_sbf::custody_replay_v1::expected_request_v1;
use dclutch_claims_svm::{
    custody_replay_v1::ClaimsCustodyReplayRequestV1,
    liability_basis_state_v2::{LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2},
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{
    CUSTODY_REPLAY_BYTES_V1, CallerRoleV1, CustodyReplaySeedsV1, CustodyReplayV1,
};
use dclutch_market_core_codec::CoreState;
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::hash::hash;

use crate::campaign::{
    parse_campaign_terminal_evidence_with_expected_cluster_v1, read_keypair_file,
};
use crate::cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1};
use crate::model::{SuccessorPlan, TransactionEvidence};
use crate::plan::pubkey;
use crate::rpc::{Rpc, WritePolicyV1};
use crate::terminal_lifecycle::routed_record;
use crate::{Error, Result};

/// The owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-claims-custody-replay-v1";
/// The public arm.
///
/// The route was written against a validator this runner owns, and the payout
/// it unblocks is not: a devnet market resolved and admitted to Core still has
/// no Claims-role replay, and terminal payout decodes one and never creates it.
/// So the loopback-only driver was the producer gap standing between a Terminal
/// Market and its first redemption. Nothing about the instruction differs
/// between the two clusters -- the cluster decides which origin is admitted,
/// which campaign report is consumable, and which label the evidence carries.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-claims-custody-replay-v1";

const fn command(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => COMMAND_DEVNET_V1,
        ExpectedClusterV1::OwnedLoopback => COMMAND_V1,
    }
}

/// The exact account count this route demands, restated from the program so a
/// frame that drifts fails here rather than after a cluster round trip.
const FRAME_ACCOUNTS_V1: usize = 15;

pub(crate) fn devnet_usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-claims-custody-replay-v1 --rpc-url URL --i-mean-devnet DEVNET_GENESIS --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY --fee-payer PUBKEY --output ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON]\n\
     \nThe public arm of the same first-use creation. It consumes an executed devnet campaign report, refuses every non-devnet origin, and writes its evidence under the devnet label. A Terminal Market cannot pay a wallet until this account exists."
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-claims-custody-replay-v1 --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY --fee-payer PUBKEY --output ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON]\n\
     \nFirst-use creation of a Market's Claims-role Custody replay -- the account terminal payout decodes and never creates, and the only caller of the DCLCCR01 route outside a program test. Nothing economic is passed in: the release set, Realm, generation and custody context come off the Claims aggregate, the rent refund off Core's own rent_beneficiary, and the rent off the Rent sysvar. Preflight opens no key. Execute sends one transaction and then reads the replay back to prove Custody created it at revision 1."
}

/// Parsed command line.
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    /// The payer's ADDRESS, required even on a preflight.
    ///
    /// This is not a convenience. `expected_request_v1` hashes the payer into
    /// the parent request digest, which seeds the caller authority, so the
    /// frame cannot be projected at all without knowing who pays. A preflight
    /// that guessed would report a PDA the execute run would not use.
    fee_payer: Pubkey,
    fee_payer_keypair: Option<PathBuf>,
    output: PathBuf,
    execute: bool,
}

/// Everything one creation is, before any key exists.
struct PlanV1 {
    instruction: Instruction,
    market: Pubkey,
    aggregate: Pubkey,
    replay: Pubkey,
    caller_authority: Pubkey,
    rent_refund: Pubkey,
    rent_lamports: u64,
    release_set: [u8; 32],
    custody_context: [u8; 32],
    generation: u64,
    request_digest: [u8; 32],
}

pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::OwnedLoopback)
}

pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::Devnet)
}

fn run(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<()> {
    let arguments = parse(arguments, expected)?;
    expected.authenticate(&arguments.origin)?;
    let mut rpc = Rpc::connect_cluster(
        &arguments.origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    let plan = plan(&mut rpc, &arguments, expected)?;
    report(&plan);

    if !arguments.execute {
        write_evidence(&arguments.output, &plan, expected, None)?;
        println!("preflight only; no key was opened and nothing was sent");
        return Ok(());
    }

    let path = arguments
        .fee_payer_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --fee-payer-keypair"))?;
    let payer = Keypair::new_from_array(read_keypair_file(path, "Claims replay creation payer")?);
    if payer.pubkey() != arguments.fee_payer {
        return Err(Error::new(format!(
            "the plan was built for payer {} and --fee-payer-keypair holds {}; the payer is one \
             of the parent request digest's coordinates, so it cannot be substituted after planning",
            arguments.fee_payer,
            payer.pubkey()
        )));
    }

    let evidence = rpc.send(
        "Claims custody replay creation",
        &[plan.instruction.clone()],
        &payer,
    )?;
    if let Some(error) = evidence.error.as_ref() {
        return Err(Error::new(format!(
            "the Claims replay creation refused on chain: {error}"
        )));
    }
    println!("signature            {}", evidence.signature);
    println!("slot                 {}", evidence.slot);
    println!(
        "compute units        {}",
        evidence
            .compute_units_consumed
            .map_or_else(|| "unreported".to_string(), |units| units.to_string())
    );

    // The chain is asked whether the replay exists, rather than the send being
    // taken as proof that it does. A creation that landed and left the account
    // unwritten is the one failure this driver must not report as success.
    let created = rpc.required_account(plan.replay, "created Claims-role Custody replay")?;
    let decoded = CustodyReplayV1::decode(&created.data)
        .map_err(|error| Error::new(format!("created Claims replay: {error:?}")))?;
    if decoded.next_revision != 1
        || decoded.caller_role != CallerRoleV1::Claims
        || decoded.market != plan.market.to_bytes()
        || decoded.context != plan.custody_context
    {
        return Err(Error::new(format!(
            "the creation landed but the replay reads role {:?} next_revision {} at context {}, \
             not a Claims cursor over {} at {}",
            decoded.caller_role,
            decoded.next_revision,
            hex_lower(&decoded.context),
            plan.market,
            hex_lower(&plan.custody_context)
        )));
    }
    println!(
        "replay after         {} next_revision 1, Claims role (read back from chain)",
        plan.replay
    );

    write_evidence(&arguments.output, &plan, expected, Some(&evidence))?;
    Ok(())
}

/// Build the one instruction this route will ever send.
fn plan(rpc: &mut Rpc, arguments: &ArgumentsV1, expected: ExpectedClusterV1) -> Result<PlanV1> {
    let plan_bytes = std::fs::read(&arguments.plan)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let evidence_bytes = std::fs::read(&arguments.evidence)?;
    let evidence =
        parse_campaign_terminal_evidence_with_expected_cluster_v1(&evidence_bytes, expected)?;

    let claims = pubkey(&plan.claims.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let activation = pubkey(&plan.activation)?;
    let market = arguments.market;

    // The aggregate is addressed by derivation, never taken from a flag, so a
    // substituted aggregate is refused by the same seed the program uses.
    let aggregate_key =
        Pubkey::find_program_address(&[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()], &claims).0;
    let aggregate_account = rpc.required_account(aggregate_key, "Claims aggregate")?;
    if aggregate_account.owner != claims {
        return Err(Error::new(format!(
            "the Claims aggregate at {aggregate_key} is owned by {}, not the plan's Claims program {claims}",
            aggregate_account.owner
        )));
    }
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    if aggregate.logical_market != market.to_bytes() {
        return Err(Error::new(
            "the Claims aggregate names another logical market",
        ));
    }

    // Core's own rent beneficiary. A transaction sponsor is never allowed to
    // redirect protocol rent, and the program re-derives this same value.
    let market_account = rpc.required_account(market, "Core Market")?;
    if market_account.owner != core {
        return Err(Error::new("the Market is not owned by the plan's Core"));
    }
    let state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    let rent_refund = Pubkey::new_from_array(state.rent_beneficiary.to_bytes());

    let rent_account = rpc.required_account(sysvar::rent::ID, "Rent sysvar")?;
    let rent: solana_sdk::rent::Rent = bincode::deserialize(&rent_account.data)
        .map_err(|error| Error::new(format!("Rent sysvar: {error}")))?;
    let rent_lamports = rent.minimum_balance(CUSTODY_REPLAY_BYTES_V1);

    // The program's own author, called rather than restated. The caller
    // authority's fifth seed is this request's digest, so any second
    // implementation that drifted by a byte would address a PDA nothing signs.
    let request = expected_request_v1(
        aggregate,
        claims.to_bytes(),
        arguments.fee_payer.to_bytes(),
        rent_refund.to_bytes(),
        rent_lamports,
    )
    .map_err(|error| Error::new(format!("Claims replay Custody request: {error:?}")))?;
    let request_bytes = request
        .to_bytes()
        .map_err(|error| Error::new(format!("Claims replay request encoding: {error:?}")))?;
    let request_digest = hash(&request_bytes).to_bytes();

    // Both PDAs come from the request the program will rebuild, so the driver
    // and the program cannot disagree about where the replay lives or who may
    // sign for it.
    let caller_seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(request.release_set)
            .map_err(|error| Error::new(format!("Claims caller release set: {error:?}")))?,
        request.market,
        ExecutionRoleV1::Claims,
        request.context,
        request_digest,
    )
    .map_err(|error| Error::new(format!("Claims caller authority seeds: {error:?}")))?;
    let caller_authority = Pubkey::find_program_address(&caller_seeds.as_slices(), &claims).0;
    let replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::from_request(request).as_slices(),
        &custody,
    )
    .0;

    // First use means first use: an occupied address is not this route's to
    // create, and saying so here is cheaper than a cluster round trip.
    if let Some(existing) = rpc.account(replay)? {
        return Err(Error::new(format!(
            "the Claims-role replay {replay} already exists ({} bytes, owner {}); this route is \
             first-use creation and has nothing to do",
            existing.data.len(),
            existing.owner
        )));
    }

    let realm = routed_record(
        &evidence,
        "realm_record",
        registry,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let claims_programdata = pubkey(&plan.claims.programdata_id)?;

    // Indices 0..12 are Custody's InitializeReplay frame verbatim; 13 and 14
    // are the Custody program this route invokes and the aggregate that owns
    // the namespace. The order is the program's own index constants.
    let accounts = vec![
        AccountMeta::new_readonly(caller_authority, false),
        AccountMeta::new_readonly(market, false),
        AccountMeta::new_readonly(activation, false),
        AccountMeta::new_readonly(registry, false),
        AccountMeta::new_readonly(claims, false),
        AccountMeta::new_readonly(claims_programdata, false),
        AccountMeta::new_readonly(realm.raw, false),
        AccountMeta::new_readonly(realm.staging, false),
        AccountMeta::new(replay, false),
        AccountMeta::new(arguments.fee_payer, true),
        AccountMeta::new_readonly(system_program::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new(rent_refund, false),
        AccountMeta::new_readonly(custody, false),
        AccountMeta::new_readonly(aggregate_key, false),
    ];
    if accounts.len() != FRAME_ACCOUNTS_V1 {
        return Err(Error::new(format!(
            "Claims replay frame is {} accounts, not the route's exact {FRAME_ACCOUNTS_V1}",
            accounts.len()
        )));
    }

    Ok(PlanV1 {
        instruction: Instruction {
            program_id: claims,
            accounts,
            data: ClaimsCustodyReplayRequestV1::new(market.to_bytes())
                .map_err(|error| Error::new(format!("Claims replay wire: {error:?}")))?
                .to_bytes()
                .to_vec(),
        },
        market,
        aggregate: aggregate_key,
        replay,
        caller_authority,
        rent_refund,
        rent_lamports,
        release_set: request.release_set,
        custody_context: request.context,
        generation: request.semantic.generation,
        request_digest,
    })
}

fn report(plan: &PlanV1) {
    println!("market               {}", plan.market);
    println!("claims aggregate     {}", plan.aggregate);
    println!("custody context      {}", hex_lower(&plan.custody_context));
    println!("release set          {}", hex_lower(&plan.release_set));
    println!("generation           {}", plan.generation);
    println!("replay to create     {}", plan.replay);
    println!("caller authority     {}", plan.caller_authority);
    println!("rent refund          {}", plan.rent_refund);
    println!("rent lamports        {}", plan.rent_lamports);
    println!("request digest       {}", hex_lower(&plan.request_digest));
    println!("frame accounts       {}", plan.instruction.accounts.len());
}

fn usage_for(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => devnet_usage(),
        ExpectedClusterV1::OwnedLoopback => usage(),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_evidence(
    path: &Path,
    plan: &PlanV1,
    expected: ExpectedClusterV1,
    landed: Option<&TransactionEvidence>,
) -> Result<()> {
    let document = json!({
        "schema": "dclutch-claims-custody-replay-evidence-v1",
        "cluster": expected.evidence_label(),
        "market": plan.market.to_string(),
        "claimsAggregate": plan.aggregate.to_string(),
        "custodyContext": hex_lower(&plan.custody_context),
        "releaseSet": hex_lower(&plan.release_set),
        "generation": plan.generation,
        "replay": plan.replay.to_string(),
        "callerAuthority": plan.caller_authority.to_string(),
        "rentRefund": plan.rent_refund.to_string(),
        "rentLamports": plan.rent_lamports,
        "requestDigest": hex_lower(&plan.request_digest),
        "frameAccounts": plan.instruction.accounts.len(),
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
    Ok(())
}

fn parse(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut evidence = None;
    let mut market = None;
    let mut fee_payer = None;
    let mut fee_payer_keypair = None;
    let mut output = None;
    let mut execute = false;
    let mut cursor = arguments.into_iter();
    while let Some(flag) = cursor.next() {
        if flag == "--execute" {
            if execute {
                return Err(Error::new("--execute was given twice"));
            }
            execute = true;
            continue;
        }
        let value = cursor.next().ok_or_else(|| {
            Error::new(format!(
                "{flag} needs a value; usage: {}",
                usage_for(expected)
            ))
        })?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            // Accepted on the public arm only. On the loopback arm the flag
            // falls through to the unknown-argument refusal below, which names
            // the command that does take it.
            DEVNET_ACKNOWLEDGMENT_FLAG if expected == ExpectedClusterV1::Devnet => {
                &mut acknowledgment
            }
            "--plan" => &mut plan,
            "--evidence" => &mut evidence,
            "--market" => &mut market,
            "--fee-payer" => &mut fee_payer,
            "--fee-payer-keypair" => &mut fee_payer_keypair,
            "--output" => &mut output,
            other => {
                return Err(Error::new(format!(
                    "unknown {} argument: {other}",
                    command(expected)
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{flag} was given twice")));
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| {
            Error::new(format!(
                "{name} is required; usage: {}",
                usage_for(expected)
            ))
        })
    };
    let rpc_url = required(rpc_url, "--rpc-url")?;
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: PathBuf::from(required(plan, "--plan")?),
        evidence: PathBuf::from(required(evidence, "--evidence")?),
        market: required(market, "--market")?
            .parse()
            .map_err(|error| Error::new(format!("--market: {error}")))?,
        fee_payer: required(fee_payer, "--fee-payer")?
            .parse()
            .map_err(|error| Error::new(format!("--fee-payer: {error}")))?,
        fee_payer_keypair: fee_payer_keypair.map(PathBuf::from),
        output: PathBuf::from(required(output, "--output")?),
        execute,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(extra: &[&str]) -> Vec<String> {
        let mut out = vec![
            "--rpc-url".to_owned(),
            "http://127.0.0.1:21400".to_owned(),
            "--plan".to_owned(),
            "/abs/plan.json".to_owned(),
            "--evidence".to_owned(),
            "/abs/evidence.json".to_owned(),
            "--market".to_owned(),
            "11111111111111111111111111111112".to_owned(),
            "--fee-payer".to_owned(),
            "11111111111111111111111111111113".to_owned(),
            "--output".to_owned(),
            "/abs/out.json".to_owned(),
        ];
        out.extend(extra.iter().map(|value| (*value).to_owned()));
        out
    }

    /// The acknowledgment flag exists on exactly one arm, and the refusal on
    /// the other names the command that takes it rather than reporting an
    /// anonymous unknown argument.
    #[test]
    fn the_devnet_acknowledgment_belongs_to_the_public_arm_alone() {
        // `ArgumentsV1` deliberately has no `Debug` -- it holds a key path --
        // so the refusal is taken by pattern rather than by `expect_err`.
        let Err(refusal) = parse(
            arguments(&[DEVNET_ACKNOWLEDGMENT_FLAG, "SomeGenesisHash"]),
            ExpectedClusterV1::OwnedLoopback,
        ) else {
            panic!("the loopback arm must not take a devnet acknowledgment");
        };
        assert!(
            refusal.0.contains(COMMAND_V1) && refusal.0.contains(DEVNET_ACKNOWLEDGMENT_FLAG),
            "expected the loopback arm to name itself and the flag, got: {refusal}"
        );
        // And the loopback arm still parses without it, so the refusal above is
        // about the flag and not about the rest of the command line.
        assert!(
            parse(arguments(&[]), ExpectedClusterV1::OwnedLoopback).is_ok(),
            "the loopback arm must parse its own command line"
        );
    }

    /// A loopback URL is refused by the public arm at ORIGIN PARSING, which is
    /// earlier than the cluster check and earlier than any key or read.
    ///
    /// The refusal is `ClusterOriginV1::parse`'s, and it is the honest one: an
    /// acknowledgment given for a loopback socket means one of the two is a
    /// typo and nothing here can tell which.
    #[test]
    fn the_public_arm_refuses_a_loopback_origin_before_any_key_or_read() {
        let Err(refusal) = parse(
            arguments(&[DEVNET_ACKNOWLEDGMENT_FLAG, "SomeGenesisHash"]),
            ExpectedClusterV1::Devnet,
        ) else {
            panic!("a loopback socket must not be acknowledged as devnet");
        };
        assert!(
            refusal.0.contains("was given for the loopback origin"),
            "expected the origin parser's loopback refusal, got: {refusal}"
        );
    }
}
