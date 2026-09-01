//! The permissionless Direct fee-settlement transaction, driven against a
//! validator this runner owns.
//!
//! This is the caller side of `docs/design/FEE_SECOND_TRANSACTION_V1.md`'s
//! second transaction. Until it existed, the `DCLTDFS1` route had a program, a
//! codec, and seven program-test arms, and no way at all to reach a live chain:
//! the only builder in the tree was inside the program test's fixture. A route
//! that can only be reached from a bank is not a route a stranger can crank,
//! and "permissionless" is a claim about strangers.
//!
//! # What this reads and what it refuses to invent
//!
//! The wire carries a COORDINATE and three bump hints, and nothing economic --
//! no amount, no destination, no revision. So neither does this driver. The
//! obligation comes off the debtor's maker replay, the creditor off the config
//! the Direct root selects, and the allowance off the live token account. The
//! caller supplies the market, the maker, and where to find the frame.
//!
//! # Why the frame comes from the trade's own public manifest
//!
//! Nineteen coordinates have to be named, and eleven of them are the same
//! release-pinned identities the fill already froze into its public manifest --
//! a file this tree writes, hashes, and re-authenticates elsewhere. Rederiving
//! them here would be a second implementation of the Direct route closure whose
//! only test would be that it agreed with the first one. So the manifest
//! supplies the ADDRESSES and the chain supplies every VALUE, and each address
//! that can be checked against chain state is checked: the maker replay against
//! its own PDA derivation, the token accounts against their owners and mint,
//! and the caller authority against the projection this driver and the program
//! both build with `project_direct_fee_request_v1`.
//!
//! That last one is the load-bearing agreement. The Custody caller authority's
//! sixth seed is the digest of the projected request bytes, so a driver that
//! reproduced the projection separately would address a PDA nothing signs the
//! moment the two drifted by a byte. Calling the same function is not a
//! convenience here; it is the only way the address can be right.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
};

use dclutch_core_contract::ContentId;
use dclutch_custody_contract::CustodyReplayV1;
use dclutch_direct_codec::fee_settlement_v1::{
    DirectFeeProjectionV1, DirectFeeSettlementRequestV1, project_direct_fee_request_v1,
};
use dclutch_direct_codec::successor::MakerReplayRootV1;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_token_svm::TokenAccount;

use crate::campaign::read_keypair_file;
use crate::cluster::ClusterOriginV1;
use crate::model::TransactionEvidence;
use crate::rpc::{Rpc, WritePolicyV1};
use crate::{Error, Result};

/// The owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-direct-fee-settlement-v1";

/// The devnet command name.
///
/// Same settlement, same plan, same refusals: the ONLY difference is how the
/// RPC origin is established. The loopback arm takes a credential-free
/// explicit-port loopback URL; this one takes a keyed cluster endpoint plus the
/// `--i-mean-devnet GENESIS` acknowledgment every other devnet writer in this
/// binary requires, and `Rpc::connect_cluster` authenticates the observed
/// genesis hash against it before a single account is read.
///
/// Nothing economic differs, because nothing economic is passed in on either
/// arm: the obligation is read off the debtor's maker replay, the creditor off
/// the config the Direct root selects, and the allowance off the live token
/// account.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-direct-fee-settlement-v1";

/// Exact top-level frame width the Trading route declares.
const FRAME_ACCOUNTS_V1: usize = 19;

/// The fourteen coordinates Custody's `Transfer` frame owns, then the five
/// Trading adds. Each entry is the manifest path its address is read from, or
/// `None` for the caller authority, which is derived rather than named.
///
/// Writability is stated once, here, in the same order the route's `frame_role`
/// states it, so a reader can put the two tables side by side.
const FRAME_V1: [(Option<&str>, bool); FRAME_ACCOUNTS_V1] = [
    (None, false),                                   //  0 caller authority (derived)
    (Some("route.fixed.market"), false),             //  1 Core Market
    (Some("route.fixed.activationCache"), false),    //  2 activation cache
    (Some("route.fixed.registryProgram"), false),    //  3 Registry program
    (Some("route.fixed.tradingProgram"), false),     //  4 Trading program
    (Some("route.fixed.tradingProgramdata"), false), //  5 Trading ProgramData
    (Some("route.custody.realm.raw"), false),        //  6 Realm record
    (Some("route.custody.realm.staging"), false),    //  7 Realm staging
    (Some("route.custody.replay"), true),            //  8 Custody replay
    (Some("route.custody.mint"), false),             //  9 collateral Mint
    (Some("route.custody.buyerToken"), true),        // 10 fee source (the debtor's)
    (Some("route.custody.feeToken"), true),          // 11 fee destination
    (Some("route.custody.custodyAuthority"), false), // 12 Custody transfer authority
    (Some("route.custody.tokenProgram"), false),     // 13 token program
    (Some("route.custody.custodyProgram"), false),   // 14 Custody program
    (Some("route.buyerMaker"), true),                // 15 debtor's maker replay
    (Some("route.fixed.root"), false),               // 16 Direct root
    (Some("route.fixed.config.raw"), false),         // 17 config record
    (Some("route.fixed.config.staging"), false),     // 18 config staging
];

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-direct-fee-settlement-v1 --rpc-url http://127.0.0.1:PORT --public-manifest ABSOLUTE_JSON --maker DEBTOR_PUBKEY --evidence ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON]\n\
     dclutch-local-successor-bootstrap devnet-direct-fee-settlement-v1 --rpc-url URL --i-mean-devnet GENESIS_HASH --public-manifest ABSOLUTE_JSON --maker DEBTOR_PUBKEY --evidence ABSOLUTE_JSON [--execute --fee-payer-keypair ABSOLUTE_JSON]\n\
     \nThe second transaction of the fee-bearing Direct pair, and the only caller of the DCLTDFS1 route outside a program test. It is permissionless: no party to the trade signs it, and the payer may be a stranger. Nothing economic is passed in -- the obligation is read off the debtor's maker replay, the creditor off the config the Direct root selects, and the allowance off the live token account, so a submission cannot move an atom the fill did not already fix. Preflight opens no key. Execute sends one transaction and then reads the maker replay back to prove fee_owed reached zero."
}

/// Parsed command line.
struct ArgumentsV1 {
    rpc_url: String,
    public_manifest: PathBuf,
    maker: Pubkey,
    fee_payer_keypair: Option<PathBuf>,
    evidence: PathBuf,
    execute: bool,
    acknowledgment: Option<String>,
}

/// Run one owned-loopback fee settlement.
pub(crate) fn run_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    if arguments.acknowledgment.is_some() {
        return Err(Error::new(format!(
            "--i-mean-devnet belongs to {COMMAND_DEVNET_V1}, not to the owned-loopback arm"
        )));
    }
    let rpc = Rpc::connect(&arguments.rpc_url)?;
    settle_v1(rpc, &arguments, "owned-loopback")
}

/// Run one devnet fee settlement.
///
/// The route is permissionless by design, so the payer here is a stranger to
/// the trade and signs nothing but the transaction fee.
pub(crate) fn run_devnet_v1(arguments: Vec<String>) -> Result<()> {
    let arguments = parse(arguments)?;
    let acknowledgment = arguments.acknowledgment.as_deref().ok_or_else(|| {
        Error::new("--i-mean-devnet GENESIS_HASH is required to settle against a public cluster")
    })?;
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, Some(acknowledgment))?;
    // ReadsOnly on a preflight is not decoration: it is what makes "preflight
    // opens no key and sends nothing" a property of the transport rather than
    // a promise made by this function.
    let policy = if arguments.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    // Health and genesis are authenticated inside `connect_cluster`, before any
    // account is read.
    let rpc = Rpc::connect_cluster(&origin, policy)?;
    let label = origin.label().to_owned();
    settle_v1(rpc, &arguments, &label)
}

/// Everything both arms do once an authenticated RPC exists.
fn settle_v1(mut rpc: Rpc, arguments: &ArgumentsV1, cluster: &str) -> Result<()> {
    let manifest = read_manifest(&arguments.public_manifest)?;

    let plan = plan(&mut rpc, &manifest, arguments.maker)?;
    report(&plan);

    if !arguments.execute {
        write_evidence(&arguments.evidence, &plan, None, None, cluster)?;
        println!("preflight only; no key was opened and nothing was sent");
        return Ok(());
    }

    let path = arguments
        .fee_payer_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --fee-payer-keypair"))?;
    let payer = Keypair::new_from_array(read_keypair_file(path, "fee settlement payer")?);
    println!("payer                {}", payer.pubkey());

    let evidence = rpc.send("direct fee settlement", &[plan.instruction.clone()], &payer)?;
    if let Some(error) = evidence.error.as_ref() {
        return Err(Error::new(format!(
            "the settlement refused on chain: {error}"
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

    // The chain is asked whether the obligation is gone, rather than the send
    // being taken as proof that it is. A settlement that landed and left
    // `fee_owed` standing would be the one failure this driver must not report
    // as success.
    let cleared = read_maker_replay(&mut rpc, plan.maker_root)?;
    if cleared.fee_owed() != 0 {
        return Err(Error::new(format!(
            "the settlement landed but {} still owes {}",
            plan.maker_root,
            cleared.fee_owed()
        )));
    }
    println!("fee_owed after       0 (read back from chain)");

    write_evidence(
        &arguments.evidence,
        &plan,
        Some(payer.pubkey()),
        Some(&evidence),
        cluster,
    )?;
    Ok(())
}

/// Everything one settlement is, before any key exists.
struct PlanV1 {
    instruction: Instruction,
    market: Pubkey,
    maker: Pubkey,
    maker_root: Pubkey,
    generation: u64,
    fee_owed: u64,
    source: Pubkey,
    destination: Pubkey,
    destination_owner: Pubkey,
    caller_authority: Pubkey,
    caller_authority_bump: u8,
    expected_revision: u64,
    allowance: u64,
}

/// Read the obligation off chain and build the exact instruction that clears it.
fn plan(rpc: &mut Rpc, manifest: &Value, maker: Pubkey) -> Result<PlanV1> {
    let mut frame = [Pubkey::default(); FRAME_ACCOUNTS_V1];
    for (index, (path, _)) in FRAME_V1.iter().enumerate() {
        if let Some(path) = path {
            frame[index] = manifest_pubkey(manifest, path)?;
        }
    }

    let maker_root = frame[15];
    let root = read_maker_replay(rpc, maker_root)?;
    if root.maker() != maker.to_bytes() {
        return Err(Error::new(format!(
            "{maker_root} is the replay of another maker, not {maker}"
        )));
    }
    let fee_owed = root.fee_owed();
    if fee_owed == 0 {
        return Err(Error::new(format!(
            "{maker} owes nothing on {maker_root}; the route would refuse as FeeNotOwed"
        )));
    }

    let replay = {
        let account = require_account(rpc, frame[8], "Custody replay")?;
        CustodyReplayV1::decode(&account)
            .map_err(|error| Error::new(format!("Custody replay: {error:?}")))?
    };

    let source = read_token(rpc, frame[10], "fee source")?;
    let destination = read_token(rpc, frame[11], "fee destination")?;
    // The route pins the source to the DEBTOR and the destination to the
    // configured recipient by OWNER. Both are checked here so a mismatch is a
    // sentence rather than an on-chain `FeeSource`/`FeeDestination` code.
    if source.owner != maker.to_bytes() {
        return Err(Error::new(format!(
            "the fee source {} is owned by {}, not by the debtor {maker}",
            frame[10],
            Pubkey::new_from_array(source.owner)
        )));
    }
    if source.mint != destination.mint {
        return Err(Error::new(
            "the fee source and destination hold different mints",
        ));
    }

    let projected = project_direct_fee_request_v1(DirectFeeProjectionV1 {
        replay,
        fee_owed,
        source: frame[10].to_bytes(),
        source_owner: source.owner,
        destination: frame[11].to_bytes(),
        destination_owner: destination.owner,
        mint: frame[9].to_bytes(),
        token_program: frame[13].to_bytes(),
        custody_authority: frame[12].to_bytes(),
    })
    .map_err(|error| Error::new(format!("fee request projection: {error:?}")))?;
    let projected_bytes = projected
        .encode()
        .map_err(|error| Error::new(format!("encode projected fee request: {error:?}")))?;

    // The same five seeds the program reassembles, over the digest of the same
    // bytes. Nothing about this address is a guess the chain will tolerate.
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(replay.release_set)
            .map_err(|error| Error::new(format!("release set: {error:?}")))?,
        replay.market,
        ExecutionRoleV1::Trading,
        replay.context,
        solana_program::hash::hash(&projected_bytes).to_bytes(),
    )
    .map_err(|error| Error::new(format!("caller authority seeds: {error:?}")))?;
    let (caller_authority, caller_authority_bump) =
        Pubkey::find_program_address(&seeds.as_slices(), &frame[4]);
    frame[0] = caller_authority;

    let wire = DirectFeeSettlementRequestV1 {
        market: root.market(),
        maker: maker.to_bytes(),
        generation: root.generation(),
        caller_authority_bump,
        // Custody derives its own two; zero is this wire's "search for it".
        custody_replay_bump: 0,
        custody_transfer_bump: 0,
    }
    .to_bytes()
    .map_err(|error| Error::new(format!("settlement wire: {error:?}")))?;

    let accounts = FRAME_V1
        .iter()
        .enumerate()
        .map(|(index, (_, writable))| {
            if *writable {
                AccountMeta::new(frame[index], false)
            } else {
                AccountMeta::new_readonly(frame[index], false)
            }
        })
        .collect::<Vec<_>>();
    if accounts.len() != FRAME_ACCOUNTS_V1 {
        return Err(Error::new("the settlement frame is not nineteen accounts"));
    }

    Ok(PlanV1 {
        instruction: Instruction {
            program_id: frame[4],
            accounts,
            data: wire.to_vec(),
        },
        market: Pubkey::new_from_array(root.market()),
        maker,
        maker_root,
        generation: root.generation(),
        fee_owed,
        source: frame[10],
        destination: frame[11],
        destination_owner: Pubkey::new_from_array(destination.owner),
        caller_authority,
        caller_authority_bump,
        expected_revision: replay.next_revision,
        allowance: source.delegated_amount,
    })
}

fn report(plan: &PlanV1) {
    println!("== the obligation, read off chain ==");
    println!("market               {}", plan.market);
    println!("generation           {}", plan.generation);
    println!("debtor               {}", plan.maker);
    println!("maker replay         {}", plan.maker_root);
    println!("fee_owed             {}", plan.fee_owed);
    println!("fee source           {}", plan.source);
    println!("standing allowance   {}", plan.allowance);
    println!("fee destination      {}", plan.destination);
    println!("destination owner    {}", plan.destination_owner);
    println!("caller authority     {}", plan.caller_authority);
    println!("authority bump       {}", plan.caller_authority_bump);
    println!(
        "custody revision     {} -> {}",
        plan.expected_revision,
        plan.expected_revision + 1
    );
}

/// Read one Trading-owned Direct maker replay.
fn read_maker_replay(rpc: &mut Rpc, address: Pubkey) -> Result<MakerReplayRootV1> {
    let data = require_account(rpc, address, "maker replay")?;
    MakerReplayRootV1::decode(&data)
        .map_err(|error| Error::new(format!("maker replay {address}: {error:?}")))
}

fn read_token(rpc: &mut Rpc, address: Pubkey, label: &str) -> Result<TokenAccount> {
    let data = require_account(rpc, address, label)?;
    TokenAccount::parse(&data).map_err(|error| Error::new(format!("{label} {address}: {error:?}")))
}

fn require_account(rpc: &mut Rpc, address: Pubkey, label: &str) -> Result<Vec<u8>> {
    rpc.account(address)?
        .map(|account| account.data)
        .ok_or_else(|| Error::new(format!("{label} {address} does not exist")))
}

/// Write what this settlement was, whether or not it was sent.
fn write_evidence(
    path: &Path,
    plan: &PlanV1,
    fee_payer: Option<Pubkey>,
    landed: Option<&TransactionEvidence>,
    cluster: &str,
) -> Result<()> {
    let document = json!({
        // One schema across both arms, with the cluster named as a FIELD. The
        // previous name asserted owned-loopback provenance in the schema
        // itself, which a devnet settlement would have carried while being
        // untrue. Nothing consumed it.
        "schema": "dclutch-direct-fee-settlement-evidence-v1",
        "cluster": cluster,
        "market": plan.market.to_string(),
        "generation": plan.generation,
        "maker": plan.maker.to_string(),
        "makerReplay": plan.maker_root.to_string(),
        "feeOwed": plan.fee_owed,
        "feeSource": plan.source.to_string(),
        "feeDestination": plan.destination.to_string(),
        "feeDestinationOwner": plan.destination_owner.to_string(),
        "standingAllowance": plan.allowance,
        // Fixed-width across preflight and execution. A null payer means no
        // key was opened and no transaction exists; an executed receipt must
        // name the exact payer whose key signed the landed transaction.
        "feePayer": fee_payer.map(|payer| payer.to_string()),
        "callerAuthority": plan.caller_authority.to_string(),
        "callerAuthorityBump": plan.caller_authority_bump,
        "custodyExpectedRevision": plan.expected_revision,
        "custodyResultingRevision": plan.expected_revision + 1,
        "landed": landed.map(|evidence| json!({
            "signature": evidence.signature,
            "slot": evidence.slot,
            "computeUnitsConsumed": evidence.compute_units_consumed,
            "feeLamports": evidence.fee_lamports,
        })),
    });
    authenticate_fee_payer_evidence_v1(&document, fee_payer, landed)?;
    std::fs::write(
        path,
        format!("{}\n", serde_json::to_string_pretty(&document)?),
    )?;
    println!("evidence             {}", path.display());
    Ok(())
}

/// Bind the fixed evidence payer to the exact transaction tuple returned by
/// the RPC semantic owner. This validator deliberately takes the payer as a
/// `Pubkey`, already derived from the opened signer, rather than accepting a
/// filename or a caller-authored string.
fn authenticate_fee_payer_evidence_v1(
    document: &Value,
    expected_fee_payer: Option<Pubkey>,
    expected_landed: Option<&TransactionEvidence>,
) -> Result<()> {
    let recorded_payer = document
        .get("feePayer")
        .ok_or_else(|| Error::new("fee-settlement evidence omitted its fixed feePayer field"))?;
    let recorded_landed = document
        .get("landed")
        .ok_or_else(|| Error::new("fee-settlement evidence omitted its fixed landed field"))?;
    match (expected_fee_payer, expected_landed) {
        (None, None) => {
            if !recorded_payer.is_null() || !recorded_landed.is_null() {
                return Err(Error::new(
                    "fee-settlement preflight evidence named a payer or transaction",
                ));
            }
        }
        (Some(payer), Some(transaction)) => {
            if transaction.error.is_some()
                || recorded_payer.as_str() != Some(payer.to_string().as_str())
                || recorded_landed.get("signature").and_then(Value::as_str)
                    != Some(transaction.signature.as_str())
                || recorded_landed.get("slot").and_then(Value::as_u64) != Some(transaction.slot)
                || recorded_landed
                    .get("computeUnitsConsumed")
                    .and_then(Value::as_u64)
                    != transaction.compute_units_consumed
                || recorded_landed.get("feeLamports").and_then(Value::as_u64)
                    != transaction.fee_lamports
            {
                return Err(Error::new(
                    "fee-settlement evidence payer or landed transaction tuple changed",
                ));
            }
        }
        (Some(_), None) => {
            return Err(Error::new(
                "fee-settlement execute evidence omitted its landed transaction",
            ));
        }
        (None, Some(_)) => {
            return Err(Error::new(
                "fee-settlement landed evidence omitted its exact transaction payer",
            ));
        }
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<Value> {
    let bytes =
        std::fs::read(path).map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
    crate::rpc::parse_json_without_duplicate_keys_v1(&bytes)
}

/// Read one dotted path out of the public manifest as a pubkey.
fn manifest_pubkey(manifest: &Value, path: &str) -> Result<Pubkey> {
    let mut cursor = manifest;
    for segment in path.split('.') {
        cursor = cursor
            .get(segment)
            .ok_or_else(|| Error::new(format!("the public manifest has no {path}")))?;
    }
    cursor
        .as_str()
        .ok_or_else(|| Error::new(format!("the public manifest's {path} is not a string")))?
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{path}: {error}")))
}

fn parse(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut public_manifest = None;
    let mut maker = None;
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
            "--public-manifest" => public_manifest = Some(PathBuf::from(value()?)),
            "--maker" => {
                maker = Some(
                    value()?
                        .parse::<Pubkey>()
                        .map_err(|error| Error::new(format!("--maker: {error}")))?,
                );
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
        public_manifest: public_manifest
            .ok_or_else(|| Error::new("--public-manifest is required"))?,
        maker: maker.ok_or_else(|| Error::new("--maker is required"))?,
        fee_payer_keypair,
        evidence: evidence.ok_or_else(|| Error::new("--evidence is required"))?,
        execute,
        acknowledgment,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transaction() -> TransactionEvidence {
        TransactionEvidence {
            label: "direct fee settlement".into(),
            signature: "fixture-signature".into(),
            slot: 17,
            transaction_metadata_available: true,
            fee_lamports: Some(5_000),
            fee_only_balance_change: None,
            compute_units_consumed: Some(1_234),
            error: None,
            logs: Vec::new(),
        }
    }

    fn executed_document(payer: Pubkey, transaction: &TransactionEvidence) -> Value {
        json!({
            "feePayer": payer.to_string(),
            "landed": {
                "signature": transaction.signature,
                "slot": transaction.slot,
                "computeUnitsConsumed": transaction.compute_units_consumed,
                "feeLamports": transaction.fee_lamports,
            }
        })
    }

    fn args(extra: &[&str]) -> Vec<String> {
        let mut v = vec![
            "--rpc-url".into(),
            "http://127.0.0.1:8899".into(),
            "--public-manifest".into(),
            "/nonexistent/manifest.json".into(),
            "--maker".into(),
            "11111111111111111111111111111111".into(),
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

    /// The frame table and the route's own account count must not drift.
    #[test]
    fn the_frame_table_is_the_width_the_route_declares() {
        assert_eq!(FRAME_V1.len(), FRAME_ACCOUNTS_V1);
        assert_eq!(FRAME_ACCOUNTS_V1, 19);
    }

    /// Exactly four coordinates are written, and they are the four the route
    /// names `Written`: the Custody replay, both token accounts, and the
    /// debtor's maker replay.
    #[test]
    fn exactly_the_four_written_coordinates_are_writable() {
        let written = FRAME_V1
            .iter()
            .enumerate()
            .filter(|(_, (_, writable))| *writable)
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(written, vec![8, 10, 11, 15]);
    }

    /// Only the caller authority is unnamed; every other coordinate is read
    /// from the manifest rather than guessed.
    #[test]
    fn only_the_caller_authority_is_derived() {
        let derived = FRAME_V1
            .iter()
            .enumerate()
            .filter(|(_, (path, _))| path.is_none())
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(derived, vec![0]);
    }

    #[test]
    fn fee_evidence_binds_the_exact_opened_payer() {
        let payer = Pubkey::new_from_array([7; 32]);
        let transaction = transaction();
        let document = executed_document(payer, &transaction);
        authenticate_fee_payer_evidence_v1(&document, Some(payer), Some(&transaction))
            .expect("the exact payer/transaction tuple is admitted");

        let substituted = executed_document(Pubkey::new_from_array([8; 32]), &transaction);
        assert!(
            authenticate_fee_payer_evidence_v1(&substituted, Some(payer), Some(&transaction))
                .is_err()
        );
    }

    #[test]
    fn execute_evidence_cannot_carry_a_null_payer_or_transaction() {
        let payer = Pubkey::new_from_array([7; 32]);
        let transaction = transaction();
        let null_execute = json!({"feePayer": null, "landed": null});
        assert!(
            authenticate_fee_payer_evidence_v1(&null_execute, Some(payer), Some(&transaction))
                .is_err()
        );
    }

    #[test]
    fn preflight_evidence_cannot_claim_a_payer() {
        let claimed = json!({
            "feePayer": Pubkey::new_from_array([7; 32]).to_string(),
            "landed": null,
        });
        assert!(authenticate_fee_payer_evidence_v1(&claimed, None, None).is_err());
    }

    #[test]
    fn fee_evidence_refuses_a_substituted_transaction_tuple() {
        let payer = Pubkey::new_from_array([7; 32]);
        let transaction = transaction();
        let mut substituted = executed_document(payer, &transaction);
        substituted["landed"]["signature"] = json!("another-signature");
        assert!(
            authenticate_fee_payer_evidence_v1(&substituted, Some(payer), Some(&transaction))
                .is_err()
        );
    }

    /// The three programs and the two token accounts sit where Custody's own
    /// `Transfer` FrameSpec puts them, so a reordering there is caught here
    /// rather than as an unaddressable PDA at send time.
    #[test]
    fn the_custody_transfer_coordinates_are_where_the_frame_spec_puts_them() {
        use dclutch_custody_contract::{
            CustodyFrameRoleV1, CustodyFrameSpecV1, OperationV1, TRANSFER_ACCOUNT_COUNT_V1,
        };
        let spec = CustodyFrameSpecV1::new(OperationV1::Transfer);
        assert_eq!(u16::from(TRANSFER_ACCOUNT_COUNT_V1), 14);
        let role = |index: u16| spec.account(index).expect("coordinate").role();
        assert_eq!(role(0), CustodyFrameRoleV1::CallerAuthority);
        assert_eq!(role(8), CustodyFrameRoleV1::Replay);
        assert_eq!(role(9), CustodyFrameRoleV1::Mint);
        assert_eq!(role(10), CustodyFrameRoleV1::TransferSource);
        assert_eq!(role(11), CustodyFrameRoleV1::TransferDestination);
        assert_eq!(role(12), CustodyFrameRoleV1::CustodyAuthority);
        assert_eq!(role(13), CustodyFrameRoleV1::TokenProgram);
        // Every coordinate Custody declares writable this driver also marks
        // writable; the reverse is deliberately free (the route pins
        // writability one way only, so the pair stays batchable).
        for index in 0..u16::from(TRANSFER_ACCOUNT_COUNT_V1) {
            if spec
                .account(index)
                .expect("coordinate")
                .privileges()
                .writable()
            {
                assert!(
                    FRAME_V1[usize::from(index)].1,
                    "coordinate {index} is writable in the FrameSpec and readonly here"
                );
            }
        }
    }
}
