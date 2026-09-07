//! Split and merge as user acts, driven against a live cluster.
//!
//! The caller side of `DCLCNS01`: one Position owner creates or destroys
//! complete sets against the Market's HoardPrincipal vault, through the
//! Claims route that CLAIMS-18 proved could not execute and the LBV2 rewrite
//! made executable. Four verbs, one act: `devnet-split-v1` / `devnet-merge-v1`
//! on the public arm and their `local-private-validator-` twins on the
//! loopback arm. The cluster decides which origin is admitted and which label
//! the evidence carries; nothing about the instruction differs.
//!
//! # What this reads and what it refuses to invent
//!
//! The wire carries the actor's statement of the act; every coordinate in it
//! is DERIVED by `plan_claims_conservation_v1` from bytes this driver reads at
//! finalized -- the aggregate, the Core Market, the linked basis record, the
//! Claims-role replay, the vault and the actor's own token account -- and
//! the operator refuses at plan time, by the route's own names, what the
//! chain would refuse at execution. This driver decides nothing; it turns a
//! cluster into that function's input and the answer into a transaction the
//! owner signs.
//!
//! # The split is two instructions
//!
//! A split debits the actor's account through Custody's delegated wire, so the
//! transaction is `ApproveChecked(collateral_atoms -> Custody authority)` then
//! the conservation instruction, both signed by the owner, who also pays the
//! fee. A merge is the conservation instruction alone.

use std::path::{Path, PathBuf};

use serde_json::json;
use solana_program::hash::hash;
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer};

use dclutch_claims::conservation::ClaimsConservationDirectionV1;
use dclutch_claims::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_claims::protocol_position_v2::{ProtocolPositionSeedsV2, failure_escrow_v1};
use dclutch_custody::token_svm::TokenAccount;
use dclutch_custody::{CallerRoleV1, CustodyReplaySeedsV1};
use dclutch_market::CoreState;
use dclutch_market::realm::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_operator::claims_conservation_v1::{
    ClaimsConservationActV1, ClaimsConservationObservedV1, ClaimsConservationOperatorErrorV1,
    ClaimsConservationPlanV1, ClaimsConservationProgramsV1, plan_claims_conservation_v1,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};

use crate::campaign::read_keypair_file;
use crate::cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1};
use crate::model::{SuccessorPlan, TransactionEvidence};
use crate::plan::pubkey;
use crate::rpc::{Rpc, WritePolicyV1};
use crate::{Error, Result};

/// The loopback split.
pub(crate) const SPLIT_COMMAND_V1: &str = "local-private-validator-split-v1";
/// The public split.
pub(crate) const SPLIT_COMMAND_DEVNET_V1: &str = "devnet-split-v1";
/// The loopback merge.
pub(crate) const MERGE_COMMAND_V1: &str = "local-private-validator-merge-v1";
/// The public merge.
pub(crate) const MERGE_COMMAND_DEVNET_V1: &str = "devnet-merge-v1";

const fn command(
    direction: ClaimsConservationDirectionV1,
    expected: ExpectedClusterV1,
) -> &'static str {
    match (direction, expected) {
        (ClaimsConservationDirectionV1::Split, ExpectedClusterV1::Devnet) => {
            SPLIT_COMMAND_DEVNET_V1
        }
        (ClaimsConservationDirectionV1::Split, ExpectedClusterV1::OwnedLoopback) => {
            SPLIT_COMMAND_V1
        }
        (ClaimsConservationDirectionV1::Merge, ExpectedClusterV1::Devnet) => {
            MERGE_COMMAND_DEVNET_V1
        }
        (ClaimsConservationDirectionV1::Merge, ExpectedClusterV1::OwnedLoopback) => {
            MERGE_COMMAND_V1
        }
    }
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap {local-private-validator-split-v1 | devnet-split-v1 | local-private-validator-merge-v1 | devnet-merge-v1} --rpc-url URL [--i-mean-devnet GENESIS_HASH] --plan ABSOLUTE_JSON --market PUBKEY --owner PUBKEY --collateral-account PUBKEY --linked-basis-record PUBKEY --quantity SETS --evidence ABSOLUTE_NEW_JSON [--execute --owner-keypair ABSOLUTE_JSON]\n\
     \nSplit: the owner deposits quantity * basis_scale collateral atoms into the Market's HoardPrincipal vault and is credited one claim at every ordinary coordinate per set (the failure coordinate, on a refunding Market, is credited to the Market's own escrow). Merge: the reverse. Nothing economic is passed in but the set count: the basis scale, the width, the cursor, the balances and every address are read off the chain and derived by the operator, which refuses at plan time what the route would refuse on chain. Without --execute this is a DRY RUN that opens no key and sends nothing, and still reports the exact instruction pair and the poststate the act would leave."
}

/// Parsed command line.
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    market: Pubkey,
    owner: Pubkey,
    collateral_account: Pubkey,
    linked_basis_record: Pubkey,
    quantity: u64,
    owner_keypair: Option<PathBuf>,
    evidence: PathBuf,
    execute: bool,
}

pub(crate) fn run_split_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run(
        arguments,
        ClaimsConservationDirectionV1::Split,
        ExpectedClusterV1::OwnedLoopback,
    )
}

pub(crate) fn run_split_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run(
        arguments,
        ClaimsConservationDirectionV1::Split,
        ExpectedClusterV1::Devnet,
    )
}

pub(crate) fn run_merge_owned_loopback_v1(arguments: Vec<String>) -> Result<()> {
    run(
        arguments,
        ClaimsConservationDirectionV1::Merge,
        ExpectedClusterV1::OwnedLoopback,
    )
}

pub(crate) fn run_merge_devnet_v1(arguments: Vec<String>) -> Result<()> {
    run(
        arguments,
        ClaimsConservationDirectionV1::Merge,
        ExpectedClusterV1::Devnet,
    )
}

fn run(
    arguments: Vec<String>,
    direction: ClaimsConservationDirectionV1,
    expected: ExpectedClusterV1,
) -> Result<()> {
    let arguments = parse(arguments, direction, expected)?;
    expected.authenticate(&arguments.origin)?;
    let mut rpc = Rpc::connect_cluster(
        &arguments.origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;
    let (plan, before) = plan(&mut rpc, &arguments, direction)?;
    report(direction, &plan, &before);

    if !arguments.execute {
        write_evidence(
            &arguments.evidence,
            direction,
            &plan,
            &before,
            None,
            expected,
            None,
        )?;
        println!("dry run; no key was opened and nothing was sent");
        return Ok(());
    }

    let path = arguments
        .owner_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --owner-keypair"))?;
    let owner = Keypair::new_from_array(read_keypair_file(path, "conservation owner")?);
    if owner.pubkey() != arguments.owner {
        return Err(Error::new(format!(
            "the plan was built for owner {} and --owner-keypair holds {}; the owner signs the \
             request and its own Position, so it cannot be substituted after planning",
            arguments.owner,
            owner.pubkey()
        )));
    }
    let mut instructions = Vec::with_capacity(2);
    if let Some(approve) = plan.approve.clone() {
        instructions.push(approve);
    }
    instructions.push(plan.instruction.clone());
    let label = match direction {
        ClaimsConservationDirectionV1::Split => "Claims split",
        ClaimsConservationDirectionV1::Merge => "Claims merge",
    };
    let evidence = rpc.send(label, &instructions, &owner)?;
    if let Some(error) = evidence.error.as_ref() {
        return Err(Error::new(format!("the {label} refused on chain: {error}")));
    }
    println!("signature            {}", evidence.signature);
    println!("slot                 {}", evidence.slot);
    println!(
        "compute units        {}",
        evidence
            .compute_units_consumed
            .map_or_else(|| "unreported".to_string(), |units| units.to_string())
    );

    // The chain is asked what it holds now, rather than the send being taken
    // as proof. Every figure below is one the request stated; a landed act
    // that left any of them elsewhere is the failure this driver must not
    // report as success.
    let after = read_poststate(&mut rpc, &plan)?;
    if after.vault_atoms != plan.request.post_hoard_amount
        || after.external_atoms != plan.request.post_external_amount
        || after.market_revision != plan.request.expected_market_revision + 1
        || after.position_revision != plan.request.expected_position_revision + 1
        || after.custody_next_revision != plan.request.expected_custody_revision + 1
    {
        return Err(Error::new(format!(
            "the {label} landed but the chain reads vault {} external {} aggregate revision {} \
             Position revision {} replay next_revision {}, not the request's poststate",
            after.vault_atoms,
            after.external_atoms,
            after.market_revision,
            after.position_revision,
            after.custody_next_revision
        )));
    }
    println!(
        "vault after          {} atoms (read back from chain)",
        after.vault_atoms
    );
    println!(
        "position after       {:?} (read back from chain)",
        after.position_balances
    );
    write_evidence(
        &arguments.evidence,
        direction,
        &plan,
        &before,
        Some(&after),
        expected,
        Some(&evidence),
    )?;
    Ok(())
}

/// What the chain holds at one observation, for the report and the evidence.
#[derive(Clone, Debug)]
struct StateV1 {
    market_revision: u64,
    position_revision: u64,
    custody_next_revision: u64,
    vault_atoms: u64,
    external_atoms: u64,
    supplies: Vec<u64>,
    position_balances: Vec<u64>,
    escrow_balances: Option<Vec<u64>>,
}

/// Build the plan from finalized bytes.
fn plan(
    rpc: &mut Rpc,
    arguments: &ArgumentsV1,
    direction: ClaimsConservationDirectionV1,
) -> Result<(ClaimsConservationPlanV1, StateV1)> {
    let plan_bytes = std::fs::read(&arguments.plan)?;
    let successor: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let programs = ClaimsConservationProgramsV1 {
        claims: pubkey(&successor.claims.program_id)?,
        claims_programdata: pubkey(&successor.claims.programdata_id)?,
        custody: pubkey(&successor.custody.program_id)?,
        core: pubkey(&successor.core.program_id)?,
        registry: pubkey(&successor.registry.program_id)?,
    };
    let market = arguments.market;

    let core_account = rpc.required_account(market, "Core Market")?;
    if core_account.owner != programs.core {
        return Err(Error::new("the Market is not owned by the plan's Core"));
    }
    let core = CoreState::decode(&core_account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;

    let aggregate_key = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, market.as_ref()],
        &programs.claims,
    )
    .0;
    let aggregate_account = rpc.required_account(aggregate_key, "Claims aggregate")?;
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    let position_key = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate_key.to_bytes(), arguments.owner.to_bytes())
            .map_err(|error| Error::new(format!("Position seeds: {error:?}")))?
            .as_slices(),
        &programs.claims,
    )
    .0;
    let position_account = rpc.required_account(position_key, "owner Position")?;
    let escrow = failure_escrow_v1(
        programs.claims,
        market.to_bytes(),
        aggregate_key,
        aggregate.claim_count,
    )
    .map_err(|error| Error::new(format!("failure escrow: {error}")))?;
    let escrow_account = rpc.account(escrow.position)?;
    let basis_account =
        rpc.required_account(arguments.linked_basis_record, "linked basis record")?;

    // The Realm record: its digest is Core's own, its address the Registry's
    // raw-record PDA over that digest, and it names the mint and the token
    // program this act moves collateral under.
    let realm_digest = core.identity.realm_id.to_bytes();
    let realm_raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            realm_digest.as_slice(),
        ],
        &programs.registry,
    )
    .0;
    let realm_staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            realm_digest.as_slice(),
        ],
        &programs.registry,
    )
    .0;
    let realm_account = rpc.required_account(realm_raw, "Realm record")?;
    if hash(&realm_account.data).to_bytes() != realm_digest {
        return Err(Error::new(
            "the Realm record's bytes do not hash to Core's realm_id",
        ));
    }
    let realm = RealmV1::decode(&realm_account.data)
        .map_err(|error| Error::new(format!("Realm record: {error:?}")))?;
    let mint_key = Pubkey::new_from_array(*realm.collateral_mint());
    let token_program = Pubkey::new_from_array(*realm.token_program());
    let mint_account = rpc.required_account(mint_key, "collateral mint")?;

    let replay_key = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            market.to_bytes(),
            aggregate.release_set,
            CallerRoleV1::Claims,
            aggregate.custody_context,
        )
        .as_slices(),
        &programs.custody,
    )
    .0;
    let replay_account = rpc.account(replay_key)?.ok_or_else(|| {
        Error::new(format!(
            "the Claims-role Custody replay {replay_key} does not exist; create it first with \
             devnet-claims-custody-replay-v1 -- every split, merge and payout advances that one \
             cursor"
        ))
    })?;
    let vault_key = Pubkey::find_program_address(
        &dclutch_custody::CustodyVaultSeedsV1::new(
            market.to_bytes(),
            aggregate.release_set,
            aggregate.custody_context,
            dclutch_custody::CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &programs.custody,
    )
    .0;
    let vault_account = rpc.required_account(vault_key, "HoardPrincipal vault")?;
    let external_account =
        rpc.required_account(arguments.collateral_account, "owner collateral account")?;

    let observed = ClaimsConservationObservedV1 {
        market,
        core_state: &core_account.data,
        aggregate: &aggregate_account.data,
        position: &position_account.data,
        escrow_position: escrow_account
            .as_ref()
            .map(|account| account.data.as_slice()),
        basis_record: (arguments.linked_basis_record, &basis_account.data),
        custody_replay: &replay_account.data,
        hoard_vault: &vault_account.data,
        external_collateral: (arguments.collateral_account, &external_account.data),
        collateral_mint: (mint_key, &mint_account.data),
        token_program,
        realm_raw,
        realm_staging,
    };
    let plan = plan_claims_conservation_v1(
        programs,
        observed,
        ClaimsConservationActV1 {
            direction,
            owner: arguments.owner,
            quantity: arguments.quantity,
        },
    )
    .map_err(describe_refusal)?;
    let before = read_poststate(rpc, &plan)?;
    Ok((plan, before))
}

/// Read the accounts the act writes, at finalized.
fn read_poststate(rpc: &mut Rpc, plan: &ClaimsConservationPlanV1) -> Result<StateV1> {
    let aggregate_account = rpc.required_account(plan.aggregate, "Claims aggregate")?;
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    let position_account = rpc.required_account(plan.position, "owner Position")?;
    let position = LiabilityBasisPositionViewV2::decode(&position_account.data)
        .map_err(|error| Error::new(format!("owner Position: {error:?}")))?;
    let escrow_balances = match rpc.account(plan.escrow_position)? {
        Some(account) if !account.data.is_empty() => {
            let view = LiabilityBasisPositionViewV2::decode(&account.data)
                .map_err(|error| Error::new(format!("escrow Position: {error:?}")))?;
            Some(
                (0..view.claim_count)
                    .map(|index| view.balance(&account.data, index).unwrap_or(0))
                    .collect(),
            )
        }
        _ => None,
    };
    let replay = rpc.required_account(plan.custody_replay, "Claims-role Custody replay")?;
    let replay = dclutch_custody::CustodyReplayV1::decode(&replay.data)
        .map_err(|error| Error::new(format!("Custody replay: {error:?}")))?;
    let vault = rpc.required_account(plan.hoard_vault, "HoardPrincipal vault")?;
    let vault = TokenAccount::parse_base_or_immutable_owner(&vault.data)
        .map_err(|error| Error::new(format!("vault token account: {error:?}")))?;
    let external = rpc.required_account(
        Pubkey::new_from_array(plan.request.external_collateral),
        "owner collateral account",
    )?;
    let external = TokenAccount::parse_base_or_immutable_owner(&external.data)
        .map_err(|error| Error::new(format!("owner token account: {error:?}")))?;
    Ok(StateV1 {
        market_revision: aggregate.revision,
        position_revision: position.revision,
        custody_next_revision: replay.next_revision,
        vault_atoms: vault.amount,
        external_atoms: external.amount,
        supplies: (0..aggregate.claim_count)
            .map(|index| {
                aggregate
                    .supply(&aggregate_account.data, index)
                    .unwrap_or(0)
            })
            .collect(),
        position_balances: (0..position.claim_count)
            .map(|index| position.balance(&position_account.data, index).unwrap_or(0))
            .collect(),
        escrow_balances,
    })
}

/// Turn a plan-time refusal into the sentence an operator can act on.
fn describe_refusal(error: ClaimsConservationOperatorErrorV1) -> Error {
    match error {
        ClaimsConservationOperatorErrorV1::Phase => Error::new(
            "the Market is not Open, so neither a split nor a merge is admitted: complete sets \
             move only while the Market trades. On chain this is Phase (0x5304).",
        ),
        ClaimsConservationOperatorErrorV1::Holding => Error::new(
            "the owner's Position does not hold that many complete sets at every coordinate a \
             merge burns; merge at most the smallest ordinary balance. On chain this is Holding \
             (0x5308).",
        ),
        ClaimsConservationOperatorErrorV1::Escrow => Error::new(
            "this Market refunds on failure and its derived escrow Position is absent or does \
             not hold the whole failure column, so no complete-set act is admitted until it is \
             seated. On chain this is FailureEscrowUnseated (0x5011).",
        ),
        ClaimsConservationOperatorErrorV1::Backing => Error::new(
            "the HoardPrincipal vault does not back the outstanding supply at the basis scale; \
             this is the L4 invariant and the route refuses Backing (0x5307) rather than act on \
             an under-collateralized Market.",
        ),
        other => Error::new(format!("conservation plan refused: {other:?}")),
    }
}

fn report(
    direction: ClaimsConservationDirectionV1,
    plan: &ClaimsConservationPlanV1,
    before: &StateV1,
) {
    println!("direction            {direction:?}");
    println!(
        "market               {}",
        Pubkey::new_from_array(plan.request.market)
    );
    println!("claims aggregate     {}", plan.aggregate);
    println!("owner position       {}", plan.position);
    println!("failure escrow       {}", plan.escrow_position);
    println!("refunds on failure   {}", plan.refunds_on_failure);
    println!("hoard vault          {}", plan.hoard_vault);
    println!("custody replay       {}", plan.custody_replay);
    println!("caller authority     {}", plan.custody_caller_authority);
    println!("complete sets        {}", plan.request.quantity);
    println!("basis scale          {}", plan.request.basis_scale);
    println!("collateral atoms     {}", plan.collateral_atoms);
    println!(
        "vault                {} -> {}",
        plan.request.pre_hoard_amount, plan.request.post_hoard_amount
    );
    println!(
        "owner collateral     {} -> {}",
        plan.request.pre_external_amount, plan.request.post_external_amount
    );
    println!("supplies before      {:?}", before.supplies);
    println!("position before      {:?}", before.position_balances);
    println!("held sets after      {}", plan.held_complete_sets_after);
    println!(
        "instructions         {}",
        if plan.approve.is_some() {
            "ApproveChecked + conserve"
        } else {
            "conserve"
        }
    );
    println!("frame accounts       {}", plan.instruction.accounts.len());
}

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[allow(clippy::too_many_arguments)]
fn write_evidence(
    path: &Path,
    direction: ClaimsConservationDirectionV1,
    plan: &ClaimsConservationPlanV1,
    before: &StateV1,
    after: Option<&StateV1>,
    expected: ExpectedClusterV1,
    landed: Option<&TransactionEvidence>,
) -> Result<()> {
    let state = |state: &StateV1| {
        json!({
            "marketRevision": state.market_revision,
            "positionRevision": state.position_revision,
            "custodyNextRevision": state.custody_next_revision,
            "vaultAtoms": state.vault_atoms,
            "externalAtoms": state.external_atoms,
            "supplies": state.supplies,
            "positionBalances": state.position_balances,
            "escrowBalances": state.escrow_balances,
        })
    };
    let document = json!({
        "schema": "dclutch-claims-conservation-evidence-v1",
        "cluster": expected.evidence_label(),
        "direction": match direction {
            ClaimsConservationDirectionV1::Split => "split",
            ClaimsConservationDirectionV1::Merge => "merge",
        },
        "market": Pubkey::new_from_array(plan.request.market).to_string(),
        "owner": Pubkey::new_from_array(plan.request.owner).to_string(),
        "claimsAggregate": plan.aggregate.to_string(),
        "position": plan.position.to_string(),
        "failureEscrow": plan.escrow_position.to_string(),
        "refundsOnFailure": plan.refunds_on_failure,
        "hoardVault": plan.hoard_vault.to_string(),
        "custodyReplay": plan.custody_replay.to_string(),
        "custodyCallerAuthority": plan.custody_caller_authority.to_string(),
        "quantity": plan.request.quantity,
        "basisScale": plan.request.basis_scale,
        "collateralAtoms": plan.collateral_atoms,
        "requestDigest": hex_lower(&hash(&plan.bytes).to_bytes()),
        "frameAccounts": plan.instruction.accounts.len(),
        "before": state(before),
        "after": after.map(state),
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

fn parse(
    arguments: Vec<String>,
    direction: ClaimsConservationDirectionV1,
    expected: ExpectedClusterV1,
) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut market = None;
    let mut owner = None;
    let mut collateral_account = None;
    let mut linked_basis_record = None;
    let mut quantity = None;
    let mut owner_keypair = None;
    let mut evidence = None;
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
        let value = cursor
            .next()
            .ok_or_else(|| Error::new(format!("{flag} needs a value; usage: {}", usage())))?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG if expected == ExpectedClusterV1::Devnet => {
                &mut acknowledgment
            }
            "--plan" => &mut plan,
            "--market" => &mut market,
            "--owner" => &mut owner,
            "--collateral-account" => &mut collateral_account,
            "--linked-basis-record" => &mut linked_basis_record,
            "--quantity" => &mut quantity,
            "--owner-keypair" => &mut owner_keypair,
            "--evidence" => &mut evidence,
            other => {
                return Err(Error::new(format!(
                    "unknown {} argument: {other}",
                    command(direction, expected)
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{flag} was given twice")));
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| Error::new(format!("{name} is required; usage: {}", usage())))
    };
    let address = |value: String, name: &str| -> Result<Pubkey> {
        value
            .parse()
            .map_err(|error| Error::new(format!("{name}: {error}")))
    };
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let quantity: u64 = required(quantity, "--quantity")?
        .parse()
        .map_err(|error| Error::new(format!("--quantity: {error}")))?;
    if quantity == 0 {
        return Err(Error::new(
            "--quantity must be a positive number of complete sets",
        ));
    }
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: PathBuf::from(required(plan, "--plan")?),
        market: address(required(market, "--market")?, "--market")?,
        owner: address(required(owner, "--owner")?, "--owner")?,
        collateral_account: address(
            required(collateral_account, "--collateral-account")?,
            "--collateral-account",
        )?,
        linked_basis_record: address(
            required(linked_basis_record, "--linked-basis-record")?,
            "--linked-basis-record",
        )?,
        quantity,
        owner_keypair: owner_keypair.map(PathBuf::from),
        evidence: PathBuf::from(required(evidence, "--evidence")?),
        execute,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(quantity: &str, extra: &[&str]) -> Vec<String> {
        let mut base = vec![
            "--rpc-url",
            "http://127.0.0.1:8899",
            "--plan",
            "/abs/plan.json",
            "--market",
            "11111111111111111111111111111112",
            "--owner",
            "11111111111111111111111111111113",
            "--collateral-account",
            "11111111111111111111111111111114",
            "--linked-basis-record",
            "11111111111111111111111111111115",
            "--quantity",
            quantity,
            "--evidence",
            "/abs/out.json",
        ];
        base.extend_from_slice(extra);
        base.into_iter().map(str::to_owned).collect()
    }

    #[test]
    fn the_devnet_acknowledgment_belongs_to_the_public_arms_alone() {
        let refusal = parse(
            arguments("3", &[DEVNET_ACKNOWLEDGMENT_FLAG, "genesis"]),
            ClaimsConservationDirectionV1::Split,
            ExpectedClusterV1::OwnedLoopback,
        )
        .err()
        .expect("the loopback arm refuses the acknowledgment");
        assert!(refusal.0.contains(SPLIT_COMMAND_V1));
    }

    #[test]
    fn a_zero_quantity_refuses_before_any_read() {
        let refusal = parse(
            arguments("0", &[]),
            ClaimsConservationDirectionV1::Merge,
            ExpectedClusterV1::OwnedLoopback,
        )
        .err()
        .expect("zero sets is not an act");
        assert!(refusal.0.contains("positive"));
    }

    #[test]
    fn the_four_verbs_are_distinct() {
        let verbs = [
            command(
                ClaimsConservationDirectionV1::Split,
                ExpectedClusterV1::OwnedLoopback,
            ),
            command(
                ClaimsConservationDirectionV1::Split,
                ExpectedClusterV1::Devnet,
            ),
            command(
                ClaimsConservationDirectionV1::Merge,
                ExpectedClusterV1::OwnedLoopback,
            ),
            command(
                ClaimsConservationDirectionV1::Merge,
                ExpectedClusterV1::Devnet,
            ),
        ];
        for (index, verb) in verbs.iter().enumerate() {
            assert!(verbs.iter().skip(index + 1).all(|other| other != verb));
        }
    }
}
