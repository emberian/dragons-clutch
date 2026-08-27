//! The external-cluster campaign driver.
//!
//! `runtime.rs` is the *supervisor*: it starts a validator, owns an ephemeral
//! authority that dies with the process, and drives a chain it created. That
//! shape is loopback-only for good structural reasons and stays that way.
//!
//! This is the other shape, the one `docs/evidence/DEVNET_SMOKE_0.md` W3 records
//! as absent: a driver that launches nothing, signs with keys an operator holds
//! on disk, and reaches a cluster it did not create. What it inherits from the
//! supervisor is everything that matters — the same plan producer, the same
//! instruction builders, the same poststate verifiers, the same founding ladder.
//! It is a different *entry*, not a second implementation. (`market.rs`'s
//! `execute_found_market` already takes only `&mut Rpc`, a plan, an authority
//! and a forge; it never asked where the chain came from.)
//!
//! # The four rails, and what each one is for
//!
//! 1. **Origin.** [`crate::cluster`] admits loopback with no ceremony and a
//!    non-loopback origin only against a typed acknowledgment naming devnet's
//!    genesis hash, then re-checks the chain's own answer at connect. Mainnet is
//!    refused unconditionally at three independent points. The supervisor's
//!    `127.0.0.1` rail is preserved *as a rail* while ceasing to be the only
//!    way to state it.
//! 2. **Reads before writes.** `--execute` is opt-in. Without it the connection
//!    is [`crate::rpc::WritePolicyV1::ReadsOnly`], which is enforced by a method
//!    allowlist at the single call site every request passes through — so a
//!    preflight *cannot* write, rather than intending not to.
//! 3. **Pacing.** SMOKE-0 friction 1 measured one busy writer starving every
//!    other request from the same IP, a 1-per-20-second poll included. Every
//!    call on a devnet connection waits its turn, and this driver is a single
//!    sequential writer by construction: it never holds two write buffers open
//!    and never fans out.
//! 4. **Resumability.** Devnet dies mid-ladder — SMOKE-0 measured exactly that
//!    and resumed into the same buffer. So every stage here detects its own
//!    completion *by reading the chain*, never from a local state file. A state
//!    file can disagree with the chain; the chain cannot disagree with itself.
//!    Re-running the driver after any failure is always safe and always the
//!    right move.
//!
//! # What this driver does NOT do, deliberately
//!
//! It does not deploy programs, and it has no code path that could. Deployment
//! is `solana program deploy`'s job, it is the act that parks ~31.7 SOL of rent,
//! and under `docs/decisions/0012-devnet-iteration-substrate.md` it is a
//! *mutable* deploy that is then iterated by `Upgrade`. What the driver owes
//! that decision is the other half: [`substrate_state`] reads each role's
//! observed deployment slot and upgrade authority and compares them to what the
//! plan pinned, so **slot drift is detected and named** — under 0012 a moved
//! slot is not a deploy error, it is the fail-closed condition every open market
//! is already in.
//!
//! # Transport, and where SMOKE-0's 100× actually applies
//!
//! SMOKE-0 §3.1 measured TPU submission at ~100× `--use-rpc` for **buffer
//! writes**, and §6.4 says the rest in its own words: "the founding ladder +
//! life are RPC-shaped end to end." The 100× belongs to the ~1,310-write buffer
//! ladder, which is the CLI's, not this driver's. Re-implementing a QUIC TPU
//! client here to submit the founding's ~116 sequential transactions — each of
//! which must be confirmed before the next is built — would buy nothing the
//! measurement supports and would put a second transaction transport in a tool
//! that has one. So: [`deploy_ladder`] emits the exact `solana program` command
//! ladder with TPU as the default and `--use-rpc` as the named fallback, and the
//! driver's own traffic is paced RPC. The transport policy is stated, tested,
//! and printed; it is not silently assumed.

use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use dclutch_pyth_svm::devnet_release_v1;
use dclutch_registry_svm::{LOADER_V3_PROGRAMDATA_METADATA_BYTES, ProgramDataMetadataV3View};
use serde_json::json;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    model::SuccessorPlan,
    plan::{hex, pubkey},
    rpc::{Rpc, WritePolicyV1},
    runtime,
    seed::{KeyForge, role},
};

/// The acknowledgment flag's literal spelling, for the argument table.
pub(crate) const DEVNET_ACKNOWLEDGMENT_FLAG_NAME: &str = DEVNET_ACKNOWLEDGMENT_FLAG;

/// The roles a driver run must be handed a keypair file for.
///
/// Exactly the signers the stages below reach. `hostile-authority` is not here:
/// its only job is to prove a refusal, the proof costs a funded wallet and two
/// transaction fees, and a driver that silently demanded a second funded key to
/// run at all would be trading the operator's lamports for evidence they did not
/// ask for. It is opt-in through `--keypair-hostile-authority`.
pub(crate) const REQUIRED_ROLES: &[&str] = &[
    role::CORE_UPGRADE_AUTHORITY,
    role::COLLATERAL_MINT,
    role::COLLATERAL_WALLET,
    role::FOUNDING_BENEFICIARY,
    role::FOUNDING_FOUNDER,
    role::FOUNDING_PROJECTION_WITNESS,
    role::FOUNDING_SOURCE_FUNDER,
];

/// Every role a `--keypair-<role>` flag may name.
pub(crate) const KEYPAIR_ROLES: &[&str] = &[
    role::CORE_UPGRADE_AUTHORITY,
    role::HOSTILE_AUTHORITY,
    role::COLLATERAL_MINT,
    role::COLLATERAL_WALLET,
    role::FOUNDING_BENEFICIARY,
    role::FOUNDING_FOUNDER,
    role::FOUNDING_PROJECTION_WITNESS,
    role::FOUNDING_SOURCE_FUNDER,
    role::SUBSTITUTED_FOUNDER,
];

/// The stages a campaign passes through, in the only order a chain accepts.
///
/// Each one owns two things: a **detector** that reads the chain and says
/// whether it is already done, and (for the stages that write) an executor. The
/// detector is what makes the driver resumable, and it is deliberately the
/// *same* poststate check the supervisor runs after executing the stage — a
/// detector that agreed with a weaker condition than the verifier would let a
/// resumed run skip work that never completed.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum StageV1 {
    /// The seven roles are deployed and their observed ProgramData matches what
    /// the plan pinned. Never writes: deployment is not this tool's act.
    Substrate,
    /// The nine infrastructure record bodies are finalized at their derived
    /// coordinates.
    Publication,
    /// Core's infrastructure profile exists and verifies.
    Initialize,
    /// The release activation cache exists and verifies.
    Activation,
    /// A Market exists, founded and Open.
    Founding,
}

impl StageV1 {
    /// The canonical order. A campaign runs a prefix of this and stops.
    pub(crate) const ORDER: [Self; 5] = [
        Self::Substrate,
        Self::Publication,
        Self::Initialize,
        Self::Activation,
        Self::Founding,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Substrate => "substrate",
            Self::Publication => "publication",
            Self::Initialize => "initialize",
            Self::Activation => "activation",
            Self::Founding => "founding",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        Self::ORDER
            .into_iter()
            .find(|stage| stage.name() == value)
            .ok_or_else(|| {
                Error::new(format!(
                    "unknown stage {value:?}; the stages are {}",
                    Self::ORDER
                        .iter()
                        .map(|stage| stage.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })
    }
}

/// What one stage's detector found.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum StageStateV1 {
    /// Nothing of this stage exists on the chain yet.
    Absent,
    /// Some of it exists. Named because a partially published record set is
    /// exactly the shape a devnet outage leaves behind, and it must not read as
    /// either "done" or "untouched".
    Partial(String),
    /// The stage's own poststate verifier passes.
    Complete,
    /// It exists and is WRONG — a different chain, a different plan, or drift.
    /// Never something a resumed run may write over.
    Conflict(String),
}

impl StageStateV1 {
    fn label(&self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Partial(_) => "partial",
            Self::Complete => "complete",
            Self::Conflict(_) => "conflict",
        }
    }

    fn detail(&self) -> Option<&str> {
        match self {
            Self::Absent | Self::Complete => None,
            Self::Partial(detail) | Self::Conflict(detail) => Some(detail),
        }
    }
}

/// One role's observed deployment, read off the cluster.
#[derive(Clone, Debug)]
pub(crate) struct ObservedRoleV1 {
    pub(crate) role: String,
    pub(crate) program_id: String,
    pub(crate) programdata_id: String,
    /// `None` when the ProgramData account does not exist at all.
    pub(crate) observed_slot: Option<u64>,
    pub(crate) pinned_slot: u64,
    /// `None` for a revoked (immutable) deployment, which is what the pre-0012
    /// ceremony produces.
    pub(crate) observed_authority: Option<String>,
    pub(crate) pinned_authority: Option<String>,
    pub(crate) observed_data_len: Option<usize>,
}

impl ObservedRoleV1 {
    /// The 0012 question: are the deployed bytes still the bytes the release
    /// bound?
    ///
    /// Under decision 0012 the answer is carried entirely by the slot: the
    /// Loader writes the current slot on every `Upgrade`, and there is no path
    /// to different bytes at one program id that does not go through `Upgrade`.
    /// So an equal slot means the deployment has not moved since the release
    /// observed it, whether or not an upgrade authority still exists.
    pub(crate) fn slot_pin_holds(&self) -> bool {
        self.observed_slot == Some(self.pinned_slot)
    }
}

/// The command surface, already parsed and validated.
pub(crate) struct CampaignArgsV1 {
    pub(crate) origin: ClusterOriginV1,
    pub(crate) plan_path: PathBuf,
    pub(crate) evidence_path: Option<PathBuf>,
    pub(crate) keypairs: BTreeMap<String, [u8; 32]>,
    pub(crate) execute: bool,
    pub(crate) through: StageV1,
}

/// Read one Solana CLI keypair file and return its 32-byte secret seed.
///
/// The CLI's format is a JSON array of 64 bytes: the ed25519 secret seed
/// followed by the public key it expands to. Both halves are read and the
/// expansion is **re-derived and compared**, so a truncated, reordered, or
/// hand-edited file is a refusal here rather than a signature the cluster
/// rejects for reasons that look like something else.
pub(crate) fn read_keypair_file(path: &Path, label: &str) -> Result<[u8; 32]> {
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} keypair path must be absolute")));
    }
    let bytes: Vec<u8> = serde_json::from_slice(&fs::read(path).map_err(|error| {
        Error::new(format!("read {label} keypair {}: {error}", path.display()))
    })?)
    .map_err(|error| {
        Error::new(format!(
            "{label} keypair {} is not a JSON byte array: {error}",
            path.display()
        ))
    })?;
    if bytes.len() != 64 {
        return Err(Error::new(format!(
            "{label} keypair {} holds {} bytes; a Solana CLI keypair file is 64 (32-byte secret \
             seed then its 32-byte public key)",
            path.display(),
            bytes.len()
        )));
    }
    let mut secret = [0_u8; 32];
    let mut declared = [0_u8; 32];
    secret.copy_from_slice(bytes.get(..32).unwrap_or_default());
    declared.copy_from_slice(bytes.get(32..).unwrap_or_default());
    let derived = Keypair::new_from_array(secret);
    if derived.pubkey().to_bytes() != declared {
        return Err(Error::new(format!(
            "{label} keypair {} is inconsistent: the public key it declares is not the one its \
             secret seed expands to. This file is damaged; do not fund the address it prints.",
            path.display()
        )));
    }
    Ok(secret)
}

/// Read every role's deployment off the cluster and compare it to the plan.
///
/// Read-only, and the one stage that is read-only even under `--execute`.
pub(crate) fn substrate_state(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
) -> Result<(StageStateV1, Vec<ObservedRoleV1>)> {
    let mut observed = Vec::new();
    let mut absent = Vec::new();
    let mut drifted = Vec::new();
    for (role, pin) in runtime::role_pins(plan) {
        let programdata = pubkey(&pin.programdata_id)?;
        let account = rpc.account(programdata)?;
        let (slot, authority, data_len) = match &account {
            None => (None, None, None),
            Some(account) => {
                let view = ProgramDataMetadataV3View::parse(&account.data).map_err(|error| {
                    Error::new(format!(
                        "{role} ProgramData at {programdata} does not parse as a Loader V3 \
                         ProgramData account: {error:?}"
                    ))
                })?;
                (
                    Some(view.deployment_slot()),
                    view.upgrade_authority()
                        .map(|key| Pubkey::from(key).to_string()),
                    Some(account.data.len()),
                )
            }
        };
        let row = ObservedRoleV1 {
            role: role.to_owned(),
            program_id: pin.program_id.clone(),
            programdata_id: pin.programdata_id.clone(),
            observed_slot: slot,
            pinned_slot: pin.deployment_slot,
            observed_authority: authority,
            pinned_authority: pin.upgrade_authority.clone(),
            observed_data_len: data_len,
        };
        if account.is_none() {
            absent.push(row.role.clone());
        } else if !row.slot_pin_holds() {
            drifted.push(format!(
                "{} observed slot {} but the release binds {}",
                row.role,
                row.observed_slot
                    .map(|slot| slot.to_string())
                    .unwrap_or_else(|| "none".into()),
                row.pinned_slot
            ));
        }
        observed.push(row);
    }
    let state = if absent.len() == observed.len() {
        StageStateV1::Absent
    } else if !absent.is_empty() {
        StageStateV1::Partial(format!("not deployed: {}", absent.join(", ")))
    } else if !drifted.is_empty() {
        // Decision 0012's fail-closed condition, stated as itself. A moved slot
        // is not a broken deploy; it is a substrate that was upgraded, and
        // every open market on it is already refusing with
        // `DeploymentSlotMismatch` until a new release generation is published,
        // activated, and new markets are founded on it.
        StageStateV1::Conflict(format!(
            "SLOT DRIFT (decision 0012 fail-closed): {}. The substrate was upgraded after this \
             plan observed it. Every market founded on the old generation is refusing right now, \
             which is the designed behaviour. Re-mint this plan's release bodies from the CURRENT \
             observed ProgramData before publishing anything.",
            drifted.join("; ")
        ))
    } else {
        StageStateV1::Complete
    };
    Ok((state, observed))
}

/// Are the nine infrastructure record bodies finalized where the plan says?
pub(crate) fn publication_state(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<StageStateV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let mut present = Vec::new();
    let mut missing = Vec::new();
    let mut wrong = Vec::new();
    for (label, pair) in &plan.records {
        let raw = pubkey(&pair.raw)?;
        match rpc.account(raw)? {
            None => missing.push(label.clone()),
            Some(account) => {
                let body = runtime::decode_hex(&pair.body_hex)?;
                if account.owner != registry || account.executable || account.data != body {
                    wrong.push(label.clone());
                } else {
                    present.push(label.clone());
                }
            }
        }
    }
    let _ = registry;
    Ok(if !wrong.is_empty() {
        StageStateV1::Conflict(format!(
            "records exist at their derived addresses with bytes that are not this plan's: {}",
            wrong.join(", ")
        ))
    } else if missing.is_empty() {
        StageStateV1::Complete
    } else if present.is_empty() {
        StageStateV1::Absent
    } else {
        StageStateV1::Partial(format!(
            "{} of {} finalized; still missing: {}",
            present.len(),
            plan.records.len(),
            missing.join(", ")
        ))
    })
}

/// Does Core's infrastructure profile exist, with this plan's exact body?
pub(crate) fn initialize_state(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<StageStateV1> {
    let address = pubkey(&plan.infrastructure_profile.address)?;
    let Some(account) = rpc.account(address)? else {
        return Ok(StageStateV1::Absent);
    };
    let expected = runtime::decode_hex(&plan.infrastructure_profile.body_hex)?;
    Ok(if account.data == expected {
        StageStateV1::Complete
    } else {
        StageStateV1::Conflict(format!(
            "an infrastructure profile exists at {address} whose {} bytes are not this plan's {}",
            account.data.len(),
            expected.len()
        ))
    })
}

/// Does the release activation cache exist?
///
/// Existence is the detector; the supervisor's own `verify_activation` is the
/// verifier and runs on the executing path. A cache account at the plan's
/// derived address that fails verification is a conflict, not a resume point.
pub(crate) fn activation_state(rpc: &mut Rpc, plan: &SuccessorPlan) -> Result<StageStateV1> {
    let address = pubkey(&plan.activation)?;
    Ok(match rpc.account(address)? {
        None => StageStateV1::Absent,
        Some(_) => match runtime::verify_activation(rpc, plan) {
            Ok(()) => StageStateV1::Complete,
            Err(error) => StageStateV1::Conflict(format!(
                "a release activation cache exists at {address} that this plan does not \
                 authenticate: {error}"
            )),
        },
    })
}

/// The payer's balance against what the remaining stages will cost.
///
/// Rent is read from the cluster rather than assumed: SMOKE-0 §1.2 re-derived
/// devnet's affine `min_balance(n) = 890,880 + 6,960·n` live, and a driver that
/// hardcoded it would be carrying a fourth copy of a number the chain will tell
/// it.
#[derive(Clone, Debug)]
pub(crate) struct WalletArithmeticV1 {
    pub(crate) payer: String,
    pub(crate) balance_lamports: u64,
    pub(crate) record_rent_lamports: u64,
    pub(crate) profile_rent_lamports: u64,
    pub(crate) activation_rent_lamports: u64,
    pub(crate) estimated_fee_lamports: u64,
    pub(crate) required_lamports: u64,
}

impl WalletArithmeticV1 {
    pub(crate) fn shortfall(&self) -> u64 {
        self.required_lamports.saturating_sub(self.balance_lamports)
    }
}

/// Fee estimate per transaction, at the base signature price.
///
/// Measured-profile: SMOKE-0 §5.3 read the recent-prioritization-fee page as
/// all zeros immediately before its ladder and paid no priority fee anywhere,
/// so the base 5,000 lamports per signature is the whole cost today. It is an
/// estimate and is labelled as one; the driver prints it beside the real fees
/// it then pays.
const LAMPORTS_PER_SIGNATURE: u64 = 5_000;

/// A generous per-stage transaction count for the estimate.
///
/// Publication is `Begin -> Append… -> Finalize` per record and the founding
/// ladder is the ~116-transaction one SMOKE-0's charter names. Rounded up: an
/// estimate that under-states the requirement is the one that strands a run.
const ESTIMATED_TRANSACTIONS: u64 = 200;

pub(crate) fn wallet_arithmetic(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    payer: Pubkey,
) -> Result<WalletArithmeticV1> {
    let balance = rpc
        .call(
            "getBalance",
            &json!([payer.to_string(), {"commitment":"finalized"}]),
        )?
        .get("value")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| Error::new("getBalance omitted a u64 value"))?;
    let mut record_rent = 0_u64;
    for pair in plan.records.values() {
        let body = runtime::decode_hex(&pair.body_hex)?;
        record_rent = record_rent.saturating_add(rpc.minimum_balance(body.len())?);
    }
    let profile_rent =
        rpc.minimum_balance(runtime::decode_hex(&plan.infrastructure_profile.body_hex)?.len())?;
    // The activation cache's width is the contract's, not a number this tool
    // owns; read the account if it exists and price the plan's own profile
    // width otherwise, which is the closest honest proxy available offline.
    let activation_rent = match rpc.account(pubkey(&plan.activation)?)? {
        Some(account) => rpc.minimum_balance(account.data.len())?,
        None => profile_rent,
    };
    let fees = ESTIMATED_TRANSACTIONS.saturating_mul(LAMPORTS_PER_SIGNATURE);
    let required = record_rent
        .saturating_add(profile_rent)
        .saturating_add(activation_rent)
        .saturating_add(fees);
    Ok(WalletArithmeticV1 {
        payer: payer.to_string(),
        balance_lamports: balance,
        record_rent_lamports: record_rent,
        profile_rent_lamports: profile_rent,
        activation_rent_lamports: activation_rent,
        estimated_fee_lamports: fees,
        required_lamports: required,
    })
}

/// Authenticate the committed devnet Pyth release row against live accounts.
///
/// The row (`dclutch_pyth_svm::devnet_release_v1`, minted by SMOKE-0 at
/// `11f249ff`) states five keys, two deployment slots, and a config digest as
/// measured facts. This re-reads all eight off the cluster and compares — the
/// same joins `provider_instruction_v3::authenticate_pyth_release` makes on
/// chain, run *before* a market is founded against them rather than discovered
/// as a refusal at resolution.
pub(crate) fn authenticate_pyth_row(rpc: &mut Rpc) -> Result<Vec<(String, bool, String)>> {
    let release = devnet_release_v1().map_err(|error| {
        Error::new(format!(
            "the committed devnet Pyth row is invalid: {error:?}"
        ))
    })?;
    let receiver = Pubkey::from(release.receiver_program());
    let receiver_programdata = Pubkey::from(release.receiver_programdata());
    let receiver_config = Pubkey::from(release.receiver_config());
    let router = Pubkey::from(release.router_program());
    let router_programdata = Pubkey::from(release.router_programdata());
    let mut rows = Vec::new();

    for (label, program, programdata, expected_slot) in [
        (
            "receiver",
            receiver,
            receiver_programdata,
            release.receiver_deployment_slot(),
        ),
        (
            "router",
            router,
            router_programdata,
            release.router_deployment_slot(),
        ),
    ] {
        match rpc.account(program)? {
            None => rows.push((
                format!("{label} program {program}"),
                false,
                "account absent".into(),
            )),
            Some(account) => rows.push((
                format!("{label} program {program}"),
                account.executable,
                format!("executable={} owner={}", account.executable, account.owner),
            )),
        }
        match rpc.account(programdata)? {
            None => rows.push((
                format!("{label} programdata {programdata}"),
                false,
                "account absent".into(),
            )),
            Some(account) => {
                let view = ProgramDataMetadataV3View::parse(&account.data).map_err(|error| {
                    Error::new(format!("{label} ProgramData does not parse: {error:?}"))
                })?;
                let slot = view.deployment_slot();
                rows.push((
                    format!("{label} deployment slot"),
                    slot == expected_slot,
                    format!("observed {slot}, row binds {expected_slot}"),
                ));
                let authority = view
                    .upgrade_authority()
                    .map(|key| Pubkey::from(key).to_string())
                    .unwrap_or_else(|| "revoked".into());
                rows.push((
                    format!("{label} upgrade authority"),
                    true,
                    format!("observed {authority} (disclosed, not bound by the row)"),
                ));
            }
        }
    }

    match rpc.account(receiver_config)? {
        None => rows.push((
            format!("receiver Config {receiver_config}"),
            false,
            "account absent".into(),
        )),
        Some(account) => {
            let digest = hex(&<sha2::Sha256 as sha2::Digest>::digest(&account.data));
            let expected = hex(&release.config_digest());
            rows.push((
                "receiver Config digest".into(),
                digest == expected,
                format!("observed {digest}, row binds {expected}"),
            ));
            rows.push((
                "receiver Config owner".into(),
                account.owner == receiver,
                format!(
                    "observed {}, must be the receiver {receiver}",
                    account.owner
                ),
            ));
        }
    }
    Ok(rows)
}

/// The `solana program` ladder a deploy would run, emitted and never executed.
///
/// TPU is the default because SMOKE-0 §3.1's A/B measured it moving Trading's
/// 1.32 MB in 23 seconds against `--use-rpc`'s ~350 B/s and `Max retries
/// exceeded`. `--use-rpc` keeps its stated role as the fallback for a machine
/// whose TPU egress is blocked, which is what the runbook's advice should have
/// said all along.
pub(crate) fn deploy_ladder(plan: &SuccessorPlan, origin: &ClusterOriginV1) -> Vec<String> {
    let mut lines = vec![
        "# This driver never deploys. These are the commands a deploy would run.".into(),
        "# Transport: TPU by default (SMOKE-0 §3.1 measured ~100x over --use-rpc for buffer".into(),
        "# writes); add --use-rpc only if this machine's TPU egress is blocked, and expect".into(),
        "# minutes per hundred KB plus Max-retries resumes if you do.".into(),
        "# Run ONE of these at a time: one write-buffer saturates the whole per-IP RPC".into(),
        "# budget (SMOKE-0 friction 1), so nothing else may share this machine's IP.".into(),
    ];
    for (role, pin) in runtime::role_pins(plan) {
        lines.push(format!(
            "solana program deploy --url {} --keypair <PAYER> --program-id <{}-KEYPAIR> {}  \
             # {role}, pins slot {}",
            origin.redacted_url(),
            role.to_uppercase(),
            pin.elf_path,
            pin.deployment_slot
        ));
    }
    lines.push(
        "# Then re-read each ProgramData and re-run `prepare --ROLE-observed-programdata`: the"
            .into(),
    );
    lines.push(
        "# deployment slot the release binds is decoded out of the resulting account, never".into(),
    );
    lines.push("# supplied by a caller.".into());
    lines
}

/// Run the driver.
pub(crate) fn execute(args: CampaignArgsV1) -> Result<()> {
    let plan: SuccessorPlan = serde_json::from_slice(&fs::read(&args.plan_path)?)?;
    let plan_sha256 = hex(&<sha2::Sha256 as sha2::Digest>::digest(fs::read(
        &args.plan_path,
    )?));
    let policy = if args.execute {
        WritePolicyV1::Writes
    } else {
        WritePolicyV1::ReadsOnly
    };
    let mut rpc = Rpc::connect_cluster(&args.origin, policy)?;

    let forge = KeyForge::persisted(args.keypairs.clone(), REQUIRED_ROLES)?;
    let authority = forge.keypair(role::CORE_UPGRADE_AUTHORITY);

    // Every detector, always, before anything is written. A stage that is
    // already complete is skipped; a stage in conflict stops the run.
    let (substrate, observed_roles) = substrate_state(&mut rpc, &plan)?;
    let mut states = vec![(StageV1::Substrate, substrate)];
    states.push((StageV1::Publication, publication_state(&mut rpc, &plan)?));
    states.push((StageV1::Initialize, initialize_state(&mut rpc, &plan)?));
    states.push((StageV1::Activation, activation_state(&mut rpc, &plan)?));

    let wallet = wallet_arithmetic(&mut rpc, &plan, authority.pubkey())?;
    let pyth = match &args.origin {
        ClusterOriginV1::AcknowledgedDevnet { .. } => Some(authenticate_pyth_row(&mut rpc)?),
        // The committed row is a devnet fact. Authenticating it against a
        // local ledger that has never seen the Pyth programs would produce a
        // page of false negatives and teach nobody anything.
        ClusterOriginV1::Loopback { .. } => None,
    };

    let report = json!({
        "schema": "dclutch-successor-campaign-report-v1",
        "cluster": args.origin.label(),
        "rpc_url": args.origin.redacted_url(),
        "mode": if args.execute { "execute" } else { "preflight (reads only, enforced)" },
        "plan": args.plan_path.display().to_string(),
        "plan_sha256": plan_sha256,
        "through_stage": args.through.name(),
        "payer": authority.pubkey().to_string(),
        "keypair_derivation": forge.derivation_label(),
        "private_key_persisted": forge.persists_private_keys(),
        "stages": states.iter().map(|(stage, state)| json!({
            "stage": stage.name(),
            "state": state.label(),
            "detail": state.detail(),
        })).collect::<Vec<_>>(),
        "roles": observed_roles.iter().map(|row| json!({
            "role": row.role,
            "program_id": row.program_id,
            "programdata_id": row.programdata_id,
            "observed_deployment_slot": row.observed_slot,
            "release_binds_deployment_slot": row.pinned_slot,
            "slot_pin_holds": row.slot_pin_holds(),
            "observed_upgrade_authority": row.observed_authority,
            "plan_upgrade_authority": row.pinned_authority,
            "observed_programdata_bytes": row.observed_data_len,
            "loader_metadata_bytes": LOADER_V3_PROGRAMDATA_METADATA_BYTES,
        })).collect::<Vec<_>>(),
        "wallet": {
            "payer": wallet.payer,
            "balance_lamports": wallet.balance_lamports,
            "record_rent_lamports": wallet.record_rent_lamports,
            "profile_rent_lamports": wallet.profile_rent_lamports,
            "activation_rent_lamports": wallet.activation_rent_lamports,
            "estimated_fee_lamports": wallet.estimated_fee_lamports,
            "required_lamports": wallet.required_lamports,
            "shortfall_lamports": wallet.shortfall(),
            "may_airdrop": args.origin.may_airdrop(),
            "funding": if args.origin.may_airdrop() {
                "this origin's faucet is the campaign's own, so a shortfall is not a blocker"
            } else {
                "this driver never airdrops: the devnet faucet is rate-limited far below a \
                 campaign's needs, so a run that begged for lamports would fail INSIDE a ladder \
                 rather than here. Fund the payer address above before running with --execute."
            },
        },
        "pyth_devnet_release_authentication": pyth.as_ref().map(|rows| rows.iter().map(|(what, ok, detail)| json!({
            "fact": what,
            "holds": ok,
            "observed": detail,
        })).collect::<Vec<_>>()),
        "deploy_ladder": deploy_ladder(&plan, &args.origin),
        "transport_policy": "driver traffic: paced RPC (SMOKE-0 §6.4 -- the founding ladder and \
                             life are RPC-shaped end to end). Buffer writes: TPU, via the solana \
                             CLI, never this process (SMOKE-0 §3.1 -- ~100x, and it is the CLI's \
                             ladder, not the driver's).",
    });

    let rendered = serde_json::to_vec_pretty(&report)?;
    if let Some(path) = &args.evidence_path {
        fs::write(path, &rendered)
            .map_err(|error| Error::new(format!("write {}: {error}", path.display())))?;
    }
    let mut stdout = std::io::stdout();
    stdout.write_all(&rendered)?;
    stdout.write_all(b"\n")?;

    if !args.execute {
        return Ok(());
    }
    execute_stages(&mut rpc, &plan, &authority, &forge, &states, args.through)
}

/// Advance the chain through the requested stages, skipping what is done.
fn execute_stages(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    authority: &Keypair,
    forge: &KeyForge,
    states: &[(StageV1, StageStateV1)],
    through: StageV1,
) -> Result<()> {
    for (stage, state) in states {
        if let StageStateV1::Conflict(detail) = state {
            return Err(Error::new(format!(
                "stage {} is in conflict and a resumed run must never write over it: {detail}",
                stage.name()
            )));
        }
    }
    let substrate = states
        .iter()
        .find(|(stage, _)| *stage == StageV1::Substrate)
        .map(|(_, state)| state);
    if substrate != Some(&StageStateV1::Complete) {
        return Err(Error::new(
            "the substrate stage is not complete, and this driver never deploys. Deploy the seven \
             roles (the ladder is printed in the report above), re-run `prepare` with each role's \
             observed ProgramData, and run this again.",
        ));
    }
    let mut transactions = Vec::new();
    for (stage, state) in states {
        if *stage > through {
            break;
        }
        if *state == StageStateV1::Complete {
            eprintln!("campaign stage {}: already complete, skipped", stage.name());
            continue;
        }
        match stage {
            StageV1::Substrate => {}
            StageV1::Publication => {
                let count = runtime::publish_infrastructure_records(
                    rpc,
                    plan,
                    authority,
                    &mut transactions,
                )?;
                eprintln!("campaign stage publication: {count} record bodies finalized");
            }
            StageV1::Initialize => {
                transactions.push(rpc.send(
                    "initialize Core infrastructure profile",
                    &[runtime::initialize_instruction(
                        plan,
                        authority.pubkey(),
                        authority.pubkey(),
                    )?],
                    authority,
                )?);
                runtime::verify_profile(rpc, plan)?;
            }
            StageV1::Activation => {
                for (label, instruction) in
                    runtime::activation_instructions(plan, authority.pubkey())?
                {
                    transactions.push(rpc.send(label, &[instruction], authority)?);
                }
                runtime::verify_activation(rpc, plan)?;
            }
            StageV1::Founding => {
                return Err(Error::new(
                    "the founding stage is not wired into the driver yet. `market::\
                     execute_found_market` takes only (&mut Rpc, &plan, &market input, &authority, \
                     &forge, &mut transactions) and is already origin-agnostic, so what it needs \
                     is the run spec's `market` block reaching this entry -- the driver takes a \
                     PLAN, and the market input lives in the SPEC. Naming this rather than \
                     half-wiring it: a founding that fails midway costs real principal.",
                ));
            }
        }
    }
    let _ = forge;
    eprintln!(
        "campaign: {} transactions submitted this run",
        transactions.len()
    );
    Ok(())
}

/// The acknowledgment text the usage line prints.
pub(crate) fn acknowledgment_help() -> String {
    format!(
        "{DEVNET_ACKNOWLEDGMENT_FLAG} <GENESIS_HASH> names the cluster by identity rather than \
         by a boolean, so a command line copied to another cluster stops being true. Mainnet is \
         refused unconditionally and no flag admits it."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_order_is_the_only_order_a_chain_accepts() {
        // Publication before Initialize is not a preference: Core's
        // infrastructure initialization READS the Registry and Rent artifact
        // records, and activation reads the five role records plus the release
        // set. The enum's Ord is what `execute_stages` uses to stop at
        // `--through`, so the declaration order is load-bearing.
        assert!(StageV1::Substrate < StageV1::Publication);
        assert!(StageV1::Publication < StageV1::Initialize);
        assert!(StageV1::Initialize < StageV1::Activation);
        assert!(StageV1::Activation < StageV1::Founding);
        assert_eq!(StageV1::ORDER.len(), 5);
        for (index, stage) in StageV1::ORDER.into_iter().enumerate() {
            assert_eq!(StageV1::parse(stage.name()).expect("round trip"), stage);
            assert_eq!(
                StageV1::ORDER.get(index).copied(),
                Some(stage),
                "ORDER must be sorted"
            );
        }
        assert!(StageV1::parse("revoke").is_err());
        let refusal = StageV1::parse("nonsense").err().expect("must refuse");
        assert!(refusal.0.contains("substrate"), "{}", refusal.0);
    }

    #[test]
    fn a_damaged_keypair_file_is_refused_before_it_is_funded() {
        let dir = std::env::temp_dir().join(format!(
            "dclutch-driver-keys-{}-{}",
            std::process::id(),
            "a"
        ));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let good = Keypair::new();
        let mut bytes = good.to_bytes().to_vec();
        let path = dir.join("good.json");
        std::fs::write(&path, serde_json::to_vec(&bytes).expect("json")).expect("write");
        assert_eq!(
            Keypair::new_from_array(read_keypair_file(&path, "test").expect("good")).pubkey(),
            good.pubkey(),
            "the secret seed must expand to the file's own address"
        );

        // A file whose declared public key is not the one its secret expands
        // to. This is the case that would otherwise be discovered as a
        // signature failure on a funded address.
        if let Some(byte) = bytes.get_mut(63) {
            *byte ^= 0xff;
        }
        let tampered = dir.join("tampered.json");
        std::fs::write(&tampered, serde_json::to_vec(&bytes).expect("json")).expect("write");
        let refusal = read_keypair_file(&tampered, "test")
            .err()
            .expect("must refuse");
        assert!(refusal.0.contains("do not fund"), "{}", refusal.0);

        // Wrong width.
        let short = dir.join("short.json");
        std::fs::write(&short, b"[1,2,3]").expect("write");
        assert!(read_keypair_file(&short, "test").is_err());
        // Not absolute.
        assert!(read_keypair_file(Path::new("relative.json"), "test").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_required_role_is_one_a_keypair_flag_can_name() {
        for role in REQUIRED_ROLES {
            assert!(
                KEYPAIR_ROLES.contains(role),
                "{role} is required but no flag names it"
            );
        }
        // The hostile authority is deliberately NOT required: proving a refusal
        // costs a second funded wallet and two fees the operator did not ask
        // for.
        assert!(!REQUIRED_ROLES.contains(&role::HOSTILE_AUTHORITY));
        assert!(KEYPAIR_ROLES.contains(&role::HOSTILE_AUTHORITY));
    }

    #[test]
    fn the_deploy_ladder_defaults_to_tpu_and_never_executes() {
        // The ladder is text. There is no code path in this module that runs a
        // deploy, and this test is the statement of that: what `deploy_ladder`
        // returns is strings, and every one of them names the transport policy
        // the measurement supports.
        let joined = ["--use-rpc", "TPU"];
        let lines = deploy_ladder(
            &SuccessorPlan {
                schema: String::new(),
                genesis_boundary: Vec::new(),
                bootstrap_order: Vec::new(),
                execution_blocker: String::new(),
                account_dir: String::new(),
                registry: pin(),
                core: pin(),
                claims: pin(),
                trading: pin(),
                resolution: pin(),
                custody: pin(),
                rent_credit: pin(),
                activation: String::new(),
                release_set_id: String::new(),
                core_bootstrap: crate::model::CoreBootstrapPin {
                    upgrade_authority: String::new(),
                    genesis_programdata_sha256: String::new(),
                    post_revoke_programdata_sha256: String::new(),
                    release_recognition_requires_revoke: false,
                },
                infrastructure_profile: crate::model::InfrastructureProfilePin {
                    address: String::new(),
                    schema_id: String::new(),
                    body_sha256: String::new(),
                    body_hex: String::new(),
                    registry_artifact_release_id: String::new(),
                    rent_artifact_release_id: String::new(),
                },
                records: BTreeMap::new(),
                record_publication: String::new(),
                provider_release_id: String::new(),
                fixture_publish_time: 0,
                genesis_accounts: BTreeMap::new(),
            },
            &ClusterOriginV1::parse(
                "https://api.devnet.solana.com/",
                Some(crate::cluster::DEVNET_GENESIS_HASH),
            )
            .expect("devnet"),
        );
        let text = lines.join("\n");
        for needle in joined {
            assert!(text.contains(needle), "the ladder must name {needle}");
        }
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.starts_with("solana program deploy"))
                .count(),
            7,
            "one deploy per role, no more"
        );
    }

    fn pin() -> crate::model::ProgramPin {
        crate::model::ProgramPin {
            program_id: String::new(),
            programdata_id: String::new(),
            elf_path: "/dev/null".into(),
            elf_sha256: String::new(),
            semantic_release_id: String::new(),
            artifact_release_id: String::new(),
            upgrade_authority: None,
            deployment_slot: 0,
            deployment_source: String::new(),
            programdata_sha256: String::new(),
        }
    }
}
