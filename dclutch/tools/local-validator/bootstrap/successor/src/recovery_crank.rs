//! One crank of a market's funded ordered-recovery ladder.
//!
//! # Why this is a frame builder and a bounded wait rather than a decision
//!
//! `RelayActionV1::AdvanceRecovery` is a 32-byte instruction naming only the
//! Market generation and the terminal sequence. Which rung the ladder stands
//! on, which source it is about to enter, and when that leg expires are all
//! read by the program off the market's own state — `SourceResolutionStateV2`
//! for the phase and the active attempt, the `WindowSpecV1` record for the
//! primary deadline, and the `RecoveryPolicyV2` for every rung's own committed
//! one. So there is nothing here for a driver to decide. What there is, is a
//! frame to assemble in exactly the order the relay contract declares, a
//! certificate seat to derive at the kind the crank is about to write, and a
//! wall-clock second to be strictly past.
//!
//! # The two seconds that are not the same second
//!
//! `SourceResolutionStateV2::crank_recovery_ladder` refuses while
//! `current_unix_seconds <= due`, and `due` is the primary window's
//! `end + max_age` on the `Primary` leg and the active attempt's own committed
//! deadline on a `Recovery` leg. The last admissible second for an honest
//! observation and the first admissible second for a crank are different
//! seconds, and this driver reproduces that comparison off chain so a crank
//! that would refuse refuses here, by name, before a lamport moves — instead of
//! arriving as `DeadlineNotReached` after a cluster round trip.
//!
//! `--wait` turns that refusal into a bounded sleep through
//! `sponsored_schedule::wait_until_unix_seconds_v1`, which refuses a target
//! further away than a stated ceiling rather than sleeping for it and never
//! warps a clock it does not own. A crank cannot be brought forward: it can
//! only be waited for.
//!
//! # What it predicts, and what happens when it predicts wrong
//!
//! A crank either advances onto the next funded rung or exhausts the ladder,
//! and those two write DIFFERENT certificate seats — `RecoveryAdvanced` and
//! `Exhausted` are separate kind seeds, so the two addresses differ. The seat
//! is a caller-supplied account, so this driver has to say which one before it
//! sends. It predicts exactly as the transition does: `entering` is zero on
//! `Primary` and `active_attempt + 1` on `Recovery`, and the policy funding
//! that index is the whole of the difference between the two arms. A wrong
//! prediction cannot pass — the program derives the seat from the crank it
//! actually took and refuses an address that is not it — so the prediction is
//! fail-closed and never a second authority.

use std::path::{Path, PathBuf};

use serde_json::json;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::transfer;

use dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_relay_contract::{
    frame::{
        RelayAccountNameV1, RelayAccountPrivilegeV1, RelayFrameKindV1, relay_frame_roles_v1,
        validate_relay_frame_v1,
    },
    instruction::AdvanceRecoveryInstructionV1,
};
use dclutch_resolution_codec::{
    RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
    ResolutionCertificateKindV2,
};
use dclutch_source_contract::{
    RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryPolicyV2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceResolutionPhaseV1, SourceResolutionStateV2,
    WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};

use crate::campaign::{
    parse_campaign_terminal_evidence_with_expected_cluster_v1, read_keypair_file,
};
use crate::cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, ExpectedClusterV1};
use crate::model::{SuccessorPlan, TransactionEvidence};
use crate::plan::pubkey;
use crate::rpc::{Rpc, WritePolicyV1};
use crate::sponsored_schedule::wait_until_unix_seconds_v1;
use crate::terminal_lifecycle::routed_record;
use crate::wallet_terminal::RecordPairV1;
use crate::{Error, Result};

/// The owned-loopback command name.
pub(crate) const COMMAND_V1: &str = "local-private-validator-advance-recovery-v1";

/// The public arm.
///
/// Nothing about the instruction differs between the two clusters — the crank
/// is permissionless on both and reads the same state. The cluster decides
/// which origin is admitted, which campaign report is consumable, and which
/// label the evidence carries.
pub(crate) const COMMAND_DEVNET_V1: &str = "devnet-advance-recovery-v1";

const fn command(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => COMMAND_DEVNET_V1,
        ExpectedClusterV1::OwnedLoopback => COMMAND_V1,
    }
}

/// The exact account count this route demands, restated from the relay frame so
/// a frame that drifts fails here rather than after a cluster round trip.
const FRAME_ACCOUNTS_V1: usize = 18;

pub(crate) fn devnet_usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-advance-recovery-v1 --rpc-url URL --i-mean-devnet DEVNET_GENESIS --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY --terminal-sequence U64 --worker PUBKEY --output ABSOLUTE_JSON [--wait --max-wait-seconds I64] [--execute --worker-keypair ABSOLUTE_JSON]\n\
     \nThe public arm of the same permissionless crank. It consumes an executed devnet campaign report, refuses every non-devnet origin, and writes its evidence under the devnet label."
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-advance-recovery-v1 --rpc-url http://127.0.0.1:PORT --plan ABSOLUTE_JSON --evidence ABSOLUTE_JSON --market PUBKEY --terminal-sequence U64 --worker PUBKEY --output ABSOLUTE_JSON [--wait --max-wait-seconds I64] [--execute --worker-keypair ABSOLUTE_JSON]\n\
     \nOne crank of a market's funded ordered-recovery ladder. Which rung, which source and when it expires are read off the market's own state, so nothing economic is passed in: the driver assembles the 18-account frame the relay contract declares, derives the certificate seat at the kind this crank will write, and refuses by name while the current leg's deadline has not passed. --wait sleeps to that deadline through one bounded wait against the chain's own clock and refuses a target further away than --max-wait-seconds; it never warps. Preflight opens no key. Execute pre-funds the seat if it is short, sends one transaction, and reads the Source state back to prove the ladder moved."
}

/// Which arm of the crank the market's own state selects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CrankArmV1 {
    /// The policy funds the index this crank enters.
    Advance,
    /// It does not, so this crank ends the ladder.
    Exhaust,
}

impl CrankArmV1 {
    const fn certificate_kind(self) -> ResolutionCertificateKindV2 {
        match self {
            Self::Advance => ResolutionCertificateKindV2::RecoveryAdvanced,
            Self::Exhaust => ResolutionCertificateKindV2::Exhausted,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Advance => "advance",
            Self::Exhaust => "exhaust",
        }
    }
}

/// Parsed command line.
#[derive(Debug)]
struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    terminal_sequence: u64,
    /// The worker's ADDRESS, required even on a preflight.
    ///
    /// The worker is position zero of the frame and is PAID by the crank, so
    /// the frame cannot be projected without it and a preflight that guessed
    /// would report a transaction the execute run would not send.
    worker: Pubkey,
    worker_keypair: Option<PathBuf>,
    output: PathBuf,
    wait: bool,
    max_wait_seconds: Option<i64>,
    execute: bool,
}

/// Everything one crank is, before any key exists.
struct PlanV1 {
    instruction: Instruction,
    market: Pubkey,
    source_state: Pubkey,
    certificate: Pubkey,
    funding_ledger: Pubkey,
    generation: u64,
    terminal_sequence: u64,
    phase: SourceResolutionPhaseV1,
    active_attempt: u8,
    entering: u8,
    attempt_count: u8,
    arm: CrankArmV1,
    /// The last second at which this leg is still answerable honestly.
    due_unix_seconds: i64,
    /// What the chain's own clock said when this plan was built.
    observed_unix_seconds: i64,
    /// Lamports the certificate seat is short of its rent, and zero when it is
    /// already funded.
    seat_shortfall_lamports: u64,
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
        .worker_keypair
        .as_deref()
        .ok_or_else(|| Error::new("--execute requires --worker-keypair"))?;
    let worker = Keypair::new_from_array(read_keypair_file(path, "recovery crank worker")?);
    if worker.pubkey() != arguments.worker {
        return Err(Error::new(format!(
            "the plan was built for worker {} and --worker-keypair holds {}; the worker is \
             position zero of the frame and is the account this crank pays, so it cannot be \
             substituted after planning",
            arguments.worker,
            worker.pubkey()
        )));
    }

    // The seat is allocated by the program from lamports it already holds, so a
    // short seat is pre-funded in the SAME transaction rather than in a second
    // one nobody would remember to send. Exactly the shortfall: a seat that
    // already holds its rent is not topped up at all.
    let mut instructions = Vec::with_capacity(2);
    if plan.seat_shortfall_lamports != 0 {
        instructions.push(transfer(
            &worker.pubkey(),
            &plan.certificate,
            plan.seat_shortfall_lamports,
        ));
    }
    instructions.push(plan.instruction.clone());

    let evidence = rpc.send("Recovery ladder crank", &instructions, &worker)?;
    if let Some(error) = evidence.error.as_ref() {
        return Err(Error::new(format!(
            "the recovery crank refused on chain: {error}"
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

    // The chain is asked whether the ladder moved, rather than the send being
    // taken as proof that it did.
    let after = rpc.required_account(plan.source_state, "cranked Source resolution state")?;
    let state = SourceResolutionStateV2::decode(&after.data)
        .map_err(|error| Error::new(format!("cranked Source state: {error:?}")))?;
    let expected_phase = match plan.arm {
        CrankArmV1::Advance => SourceResolutionPhaseV1::Recovery,
        CrankArmV1::Exhaust => SourceResolutionPhaseV1::Exhausted,
    };
    let expected_attempt = match plan.arm {
        CrankArmV1::Advance => plan.entering,
        CrankArmV1::Exhaust => 0,
    };
    if state.phase() != expected_phase || state.active_attempt() != expected_attempt {
        return Err(Error::new(format!(
            "the crank landed and the Source reads phase {:?} attempt {}, not the {:?} attempt {} \
             this {} was planned to produce",
            state.phase(),
            state.active_attempt(),
            expected_phase,
            expected_attempt,
            plan.arm.label()
        )));
    }
    println!(
        "source after         phase {:?}, active attempt {} (read back from chain)",
        state.phase(),
        state.active_attempt()
    );

    write_evidence(&arguments.output, &plan, expected, Some(&evidence))?;
    Ok(())
}

/// Build the one instruction this crank will send.
fn plan(rpc: &mut Rpc, arguments: &ArgumentsV1, expected: ExpectedClusterV1) -> Result<PlanV1> {
    let plan_bytes = std::fs::read(&arguments.plan)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let evidence_bytes = std::fs::read(&arguments.evidence)?;
    let evidence =
        parse_campaign_terminal_evidence_with_expected_cluster_v1(&evidence_bytes, expected)?;

    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let activation = pubkey(&plan.activation)?;
    let market = arguments.market;

    // Every record pair is re-derived from its own schema and body digest by
    // `routed_record`, which refuses a persisted address that is not the
    // canonical one. A campaign report cannot therefore seat a junk coordinate
    // in this frame by naming one.
    let material = routed_record(
        &evidence,
        "source_material_record",
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let window = routed_record(
        &evidence,
        "window_spec_record",
        registry,
        WINDOW_SPEC_SCHEMA_ID_V1,
    )?;
    let manifest = routed_record(
        &evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    // ABSENCE IS THE ANSWER, NOT A GAP. A founding whose material bought no
    // ladder publishes no policy record and its evidence says so by leaving the
    // row out. There is nothing for this driver to crank in that case, and the
    // refusal says which market it is looking at rather than which key it could
    // not find.
    if !evidence.accounts.contains_key("recovery_policy_record") {
        return Err(Error::new(format!(
            "market {market} bought no ordered recovery walk: its founding published no \
             RecoveryPolicyV2, so the ladder this command cranks does not exist. A market with no \
             policy reaches its pre-disclosed failure through CommitDeadlineFailure instead"
        )));
    }
    let policy_pair = routed_record(
        &evidence,
        "recovery_policy_record",
        registry,
        RECOVERY_POLICY_SCHEMA_ID_V2,
    )?;
    let funding_ledger = pubkey(
        &evidence
            .accounts
            .get("resolution_funding_ledger")
            .ok_or_else(|| {
                Error::new("the campaign report names no resolution_funding_ledger to spend")
            })?
            .address,
    )?;

    // The Market's own generation, off the Market, because the wire carries it
    // and the program compares the two.
    let market_account = rpc.required_account(market, "Core Market")?;
    if market_account.owner != core {
        return Err(Error::new(format!(
            "the account at {market} is owned by {}, not the plan's Core program {core}",
            market_account.owner
        )));
    }
    let state = dclutch_market_core_codec::CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    let generation = state.identity.generation;

    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let source_account = rpc.required_account(source_state, "Source resolution state")?;
    let source = SourceResolutionStateV2::decode(&source_account.data)
        .map_err(|error| Error::new(format!("Source resolution state: {error:?}")))?;

    let policy_account = rpc.required_account(policy_pair.raw, "RecoveryPolicyV2 record")?;
    let policy = RecoveryPolicyV2::decode(&policy_account.data)
        .map_err(|error| Error::new(format!("RecoveryPolicyV2 record: {error:?}")))?;
    let window_account = rpc.required_account(window.raw, "WindowSpecV1 record")?;
    let window_spec = WindowSpecV1::decode(&window_account.data)
        .map_err(|error| Error::new(format!("WindowSpecV1 record: {error:?}")))?;

    // THE SAME TWO ARITHMETIC FACTS THE TRANSITION COMPUTES, and neither is a
    // choice: `entering` is zero on the primary leg and one past the active
    // attempt on a recovery leg, and `due` is the primary window's own closing
    // plus its liveness grace, or the active attempt's committed deadline.
    let (entering, due) = match source.phase() {
        SourceResolutionPhaseV1::Primary => (
            0_u8,
            window_spec
                .end_unix_seconds()
                .checked_add(i64::from(window_spec.max_age_seconds()))
                .ok_or_else(|| Error::new("primary window end + max_age overflows"))?,
        ),
        SourceResolutionPhaseV1::Recovery => (
            source
                .active_attempt()
                .checked_add(1)
                .ok_or_else(|| Error::new("recovery attempt index overflows"))?,
            policy
                .attempt(source.active_attempt())
                .map_err(|error| {
                    Error::new(format!(
                        "the market stands on attempt {} and the policy does not fund it: {error:?}",
                        source.active_attempt()
                    ))
                })?
                .deadline_unix_seconds(),
        ),
        other => {
            return Err(Error::new(format!(
                "market {market} stands at {other:?}: a ladder is cranked only from Primary or \
                 Recovery, and every other phase is a market that has already reached a terminal"
            )));
        }
    };
    let arm = if policy.attempt(entering).is_ok() {
        CrankArmV1::Advance
    } else {
        CrankArmV1::Exhaust
    };

    // THE WAIT, or the refusal. A crank is admissible strictly after the leg's
    // deadline, so the target is one second past it.
    let slot = rpc.finalized_slot()?;
    let mut observed = rpc.block_time(slot)?;
    if arguments.wait && observed <= due {
        let ceiling = arguments.max_wait_seconds.ok_or_else(|| {
            Error::new("--wait requires --max-wait-seconds: a wait with no ceiling cannot say what it will cost")
        })?;
        observed = wait_until_unix_seconds_v1(
            rpc,
            due.checked_add(1)
                .ok_or_else(|| Error::new("the crank's first admissible second overflows"))?,
            ceiling,
        )?;
    }
    if observed <= due {
        return Err(Error::new(format!(
            "the {} leg is due at {due} and the chain clock reads {observed}: a crank is \
             admissible STRICTLY after the deadline, because the last second an honest \
             observation may land and the first second a crank may run are different seconds. \
             This is `DeadlineNotReached` refused before a lamport moves; pass --wait \
             --max-wait-seconds to sleep to it",
            match source.phase() {
                SourceResolutionPhaseV1::Primary => "primary",
                _ => "recovery",
            }
        )));
    }

    let certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            source_state.as_ref(),
            &[arm.certificate_kind().kind_seed()],
            &arguments.terminal_sequence.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let rent_account = rpc.required_account(sysvar::rent::ID, "Rent sysvar")?;
    let rent: solana_sdk::rent::Rent = bincode::deserialize(&rent_account.data)
        .map_err(|error| Error::new(format!("Rent sysvar: {error}")))?;
    let required = rent.minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2);
    let held = rpc.account(certificate)?.map_or(0, |seat| seat.lamports);
    let seat_shortfall_lamports = required.saturating_sub(held);

    // THE FRAME, DRIVEN BY THE RELAY CONTRACT'S OWN TABLE. Signer and writable
    // come from `relay_frame_roles_v1`, never from this file, and
    // `validate_relay_frame_v1` then checks the count, every privilege and the
    // complete no-alias policy offline -- so a frame that would refuse as
    // `InvalidAccountFrame` refuses here instead.
    let filled = advance_recovery_keys_v1(&AdvanceRecoveryCoordinatesV1 {
        worker: arguments.worker,
        market,
        core,
        activation,
        source_state,
        certificate,
        material,
        window,
        policy: policy_pair,
        manifest,
        funding_ledger,
    });
    let roles = relay_frame_roles_v1(RelayFrameKindV1::AdvanceRecovery);
    if roles.len() != FRAME_ACCOUNTS_V1 {
        return Err(Error::new(format!(
            "the AdvanceRecovery frame declares {} positions and this driver fills {FRAME_ACCOUNTS_V1}",
            roles.len()
        )));
    }
    // EVERY POSITION SAYS WHAT IT IS, and the contract is asked whether it
    // agrees. Privileges and the no-alias rule below would pass a frame whose
    // eighteen keys were in the wrong order -- the recovery policy where the
    // window belongs would still be readonly and still distinct -- so the
    // ordering is checked against the frame's own role NAMES rather than
    // trusted to the order they were typed in.
    for (index, (role, (name, _))) in roles.iter().zip(filled.iter()).enumerate() {
        if role.name() != *name {
            return Err(Error::new(format!(
                "position {index} of the AdvanceRecovery frame is {:?} and this driver filled it \
                 with {name:?}",
                role.name()
            )));
        }
    }
    let keys: Vec<Pubkey> = filled.iter().map(|(_, key)| *key).collect();
    let privileges: Vec<RelayAccountPrivilegeV1> = roles
        .iter()
        .zip(keys.iter())
        .map(|(role, key)| RelayAccountPrivilegeV1 {
            key: key.to_bytes(),
            is_signer: role.is_signer(),
            is_writable: role.is_writable(),
        })
        .collect();
    validate_relay_frame_v1(RelayFrameKindV1::AdvanceRecovery, &privileges)
        .map_err(|error| Error::new(format!("AdvanceRecovery frame: {error:?}")))?;
    let accounts: Vec<AccountMeta> = roles
        .iter()
        .zip(keys.iter())
        .map(|(role, key)| AccountMeta {
            pubkey: *key,
            is_signer: role.is_signer(),
            is_writable: role.is_writable(),
        })
        .collect();

    Ok(PlanV1 {
        instruction: Instruction {
            program_id: resolution,
            accounts,
            data: AdvanceRecoveryInstructionV1::new(generation, arguments.terminal_sequence)
                .map_err(|error| Error::new(format!("AdvanceRecovery request: {error:?}")))?
                .to_bytes()
                .map_err(|error| Error::new(format!("AdvanceRecovery wire: {error:?}")))?
                .to_vec(),
        },
        market,
        source_state,
        certificate,
        funding_ledger,
        generation,
        terminal_sequence: arguments.terminal_sequence,
        phase: source.phase(),
        active_attempt: source.active_attempt(),
        entering,
        attempt_count: policy.attempt_count(),
        arm,
        due_unix_seconds: due,
        observed_unix_seconds: observed,
        seat_shortfall_lamports,
    })
}

/// Everything the eighteen positions are filled from.
struct AdvanceRecoveryCoordinatesV1 {
    worker: Pubkey,
    market: Pubkey,
    core: Pubkey,
    activation: Pubkey,
    source_state: Pubkey,
    certificate: Pubkey,
    material: RecordPairV1,
    window: RecordPairV1,
    policy: RecordPairV1,
    manifest: RecordPairV1,
    funding_ledger: Pubkey,
}

/// The frame's eighteen keys, each carrying the role name it claims to be.
///
/// One author for the order, so the check against `relay_frame_roles_v1` and
/// the metas that are actually sent cannot drift apart -- and so a test can ask
/// the same question without a chain.
fn advance_recovery_keys_v1(
    coordinates: &AdvanceRecoveryCoordinatesV1,
) -> [(RelayAccountNameV1, Pubkey); FRAME_ACCOUNTS_V1] {
    [
        (RelayAccountNameV1::Worker, coordinates.worker),
        (RelayAccountNameV1::Market, coordinates.market),
        (RelayAccountNameV1::CoreProgram, coordinates.core),
        (
            RelayAccountNameV1::RegistryActivation,
            coordinates.activation,
        ),
        (
            RelayAccountNameV1::SourceResolutionState,
            coordinates.source_state,
        ),
        (
            RelayAccountNameV1::ResolutionCertificate,
            coordinates.certificate,
        ),
        (RelayAccountNameV1::SourceMaterial, coordinates.material.raw),
        (
            RelayAccountNameV1::SourceMaterialStagingVacancy,
            coordinates.material.staging,
        ),
        (RelayAccountNameV1::WindowSpec, coordinates.window.raw),
        (
            RelayAccountNameV1::WindowSpecStagingVacancy,
            coordinates.window.staging,
        ),
        (RelayAccountNameV1::RecoveryPolicy, coordinates.policy.raw),
        (
            RelayAccountNameV1::RecoveryPolicyStagingVacancy,
            coordinates.policy.staging,
        ),
        (
            RelayAccountNameV1::CapabilityManifest,
            coordinates.manifest.raw,
        ),
        (
            RelayAccountNameV1::CapabilityManifestStagingVacancy,
            coordinates.manifest.staging,
        ),
        (
            RelayAccountNameV1::ResolutionFunding,
            coordinates.funding_ledger,
        ),
        (RelayAccountNameV1::ClockSysvar, sysvar::clock::ID),
        (RelayAccountNameV1::RentSysvar, sysvar::rent::ID),
        (RelayAccountNameV1::SystemProgram, system_program::ID),
    ]
}

fn report(plan: &PlanV1) {
    println!("market               {}", plan.market);
    println!("generation           {}", plan.generation);
    println!("source state         {}", plan.source_state);
    println!("phase                {:?}", plan.phase);
    println!("active attempt       {}", plan.active_attempt);
    println!(
        "ladder               {} funded attempts; this crank enters {}",
        plan.attempt_count, plan.entering
    );
    println!("arm                  {}", plan.arm.label());
    println!("due at               {}", plan.due_unix_seconds);
    println!("chain clock          {}", plan.observed_unix_seconds);
    println!("terminal sequence    {}", plan.terminal_sequence);
    println!("certificate seat     {}", plan.certificate);
    println!("seat shortfall       {}", plan.seat_shortfall_lamports);
    println!("funding ledger       {}", plan.funding_ledger);
    println!("frame accounts       {}", plan.instruction.accounts.len());
}

fn usage_for(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => devnet_usage(),
        ExpectedClusterV1::OwnedLoopback => usage(),
    }
}

fn write_evidence(
    path: &Path,
    plan: &PlanV1,
    expected: ExpectedClusterV1,
    landed: Option<&TransactionEvidence>,
) -> Result<()> {
    let document = json!({
        "schema": "dclutch-recovery-crank-evidence-v1",
        "cluster": expected.evidence_label(),
        "market": plan.market.to_string(),
        "generation": plan.generation,
        "sourceState": plan.source_state.to_string(),
        "phase": format!("{:?}", plan.phase),
        "activeAttempt": plan.active_attempt,
        "enteringAttempt": plan.entering,
        "fundedAttempts": plan.attempt_count,
        "arm": plan.arm.label(),
        "dueUnixSeconds": plan.due_unix_seconds,
        "observedUnixSeconds": plan.observed_unix_seconds,
        "terminalSequence": plan.terminal_sequence,
        "certificate": plan.certificate.to_string(),
        "certificateSeatShortfallLamports": plan.seat_shortfall_lamports,
        "fundingLedger": plan.funding_ledger.to_string(),
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
    let mut terminal_sequence = None;
    let mut worker = None;
    let mut worker_keypair = None;
    let mut output = None;
    let mut max_wait_seconds = None;
    let mut wait = false;
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
        if flag == "--wait" {
            if wait {
                return Err(Error::new("--wait was given twice"));
            }
            wait = true;
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
            // falls through to the unknown-argument refusal below.
            DEVNET_ACKNOWLEDGMENT_FLAG if expected == ExpectedClusterV1::Devnet => {
                &mut acknowledgment
            }
            "--plan" => &mut plan,
            "--evidence" => &mut evidence,
            "--market" => &mut market,
            "--terminal-sequence" => &mut terminal_sequence,
            "--worker" => &mut worker,
            "--worker-keypair" => &mut worker_keypair,
            "--max-wait-seconds" => &mut max_wait_seconds,
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
    if max_wait_seconds.is_some() && !wait {
        return Err(Error::new(
            "--max-wait-seconds is the ceiling on --wait and means nothing without it",
        ));
    }
    let rpc_url = required(rpc_url, "--rpc-url")?;
    let terminal_sequence = required(terminal_sequence, "--terminal-sequence")?
        .parse::<u64>()
        .map_err(|_| Error::new("--terminal-sequence must be a decimal u64"))?;
    if terminal_sequence == 0 {
        return Err(Error::new(
            "--terminal-sequence must be positive; the wire refuses zero",
        ));
    }
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: PathBuf::from(required(plan, "--plan")?),
        evidence: PathBuf::from(required(evidence, "--evidence")?),
        market: required(market, "--market")?
            .parse()
            .map_err(|error| Error::new(format!("--market: {error}")))?,
        terminal_sequence,
        worker: required(worker, "--worker")?
            .parse()
            .map_err(|error| Error::new(format!("--worker: {error}")))?,
        worker_keypair: worker_keypair.map(PathBuf::from),
        output: PathBuf::from(required(output, "--output")?),
        wait,
        max_wait_seconds: match max_wait_seconds {
            None => None,
            Some(raw) => Some(
                raw.parse::<i64>()
                    .map_err(|_| Error::new("--max-wait-seconds must be a decimal i64"))?,
            ),
        },
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
            Pubkey::new_from_array([0x21; 32]).to_string(),
            "--terminal-sequence".to_owned(),
            "1".to_owned(),
            "--worker".to_owned(),
            Pubkey::new_from_array([0x22; 32]).to_string(),
            "--output".to_owned(),
            "/abs/out.json".to_owned(),
        ];
        out.extend(extra.iter().map(|value| (*value).to_owned()));
        out
    }

    /// The eighteen positions, their privileges and the no-alias rule are the
    /// relay contract's, and this asserts that this driver reads them from
    /// there rather than restating them.
    #[test]
    fn the_frame_is_the_relay_contracts_and_this_driver_only_fills_it() {
        let roles = relay_frame_roles_v1(RelayFrameKindV1::AdvanceRecovery);
        assert_eq!(roles.len(), FRAME_ACCOUNTS_V1);
        // Position zero is the only signer and one of exactly three writable
        // protocol positions besides it: the Source that advances, the receipt
        // this crank creates, and the ledger it spends.
        assert!(roles[0].is_signer() && roles[0].is_writable());
        let writable: Vec<usize> = roles
            .iter()
            .enumerate()
            .filter(|(_, role)| role.is_writable())
            .map(|(index, _)| index)
            .collect();
        assert_eq!(writable, vec![0, 4, 5, 14]);
        assert_eq!(
            roles.iter().filter(|role| role.is_signer()).count(),
            1,
            "a permissionless crank has exactly one signer, and it is the worker it pays"
        );
    }

    /// Every key this driver fills is checked against the role the contract
    /// declares for that position, so an ordering slip cannot hide behind
    /// privileges that happen to match.
    #[test]
    fn every_filled_position_is_the_role_the_contract_declares() {
        let pair = |seed: u8| RecordPairV1 {
            schema: [seed; 32],
            digest: [seed.wrapping_add(2); 32],
            raw: Pubkey::new_from_array([seed; 32]),
            staging: Pubkey::new_from_array([seed.wrapping_add(1); 32]),
        };
        let filled = advance_recovery_keys_v1(&AdvanceRecoveryCoordinatesV1 {
            worker: Pubkey::new_from_array([0x01; 32]),
            market: Pubkey::new_from_array([0x02; 32]),
            core: Pubkey::new_from_array([0x03; 32]),
            activation: Pubkey::new_from_array([0x04; 32]),
            source_state: Pubkey::new_from_array([0x05; 32]),
            certificate: Pubkey::new_from_array([0x06; 32]),
            material: pair(0x10),
            window: pair(0x20),
            policy: pair(0x30),
            manifest: pair(0x40),
            funding_ledger: Pubkey::new_from_array([0x50; 32]),
        });
        let roles = relay_frame_roles_v1(RelayFrameKindV1::AdvanceRecovery);
        assert_eq!(roles.len(), filled.len());
        for (index, (role, (name, _))) in roles.iter().zip(filled.iter()).enumerate() {
            assert_eq!(role.name(), *name, "position {index}");
        }
        // The two record pairs a rung frame carries that the failure walk does
        // not, named rather than counted: without them a crank has no policy to
        // read and no proof the record it read is finalized.
        assert_eq!(filled[10].0, RelayAccountNameV1::RecoveryPolicy);
        assert_eq!(
            filled[11].0,
            RelayAccountNameV1::RecoveryPolicyStagingVacancy
        );
    }

    /// The two arms write two different seats, which is why the driver has to
    /// predict which one before it sends.
    #[test]
    fn the_two_arms_are_two_certificate_addresses() {
        assert_ne!(
            CrankArmV1::Advance.certificate_kind().kind_seed(),
            CrankArmV1::Exhaust.certificate_kind().kind_seed()
        );
        assert_eq!(
            CrankArmV1::Advance.certificate_kind(),
            ResolutionCertificateKindV2::RecoveryAdvanced
        );
        assert_eq!(
            CrankArmV1::Exhaust.certificate_kind(),
            ResolutionCertificateKindV2::Exhausted
        );
    }

    /// A ceiling with nothing to bound is a flag that reads as a wait and is
    /// not one.
    #[test]
    fn a_wait_ceiling_without_a_wait_refuses_by_name() {
        let refusal = parse(
            arguments(&["--max-wait-seconds", "600"]),
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect_err("a ceiling with no wait must refuse");
        assert!(
            format!("{refusal}").contains("means nothing without it"),
            "got {refusal}"
        );
        parse(
            arguments(&["--wait", "--max-wait-seconds", "600"]),
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect("a bounded wait parses");
    }

    /// The wire refuses a zero terminal sequence, so the parser does too rather
    /// than building a request that cannot encode.
    #[test]
    fn a_zero_terminal_sequence_refuses_at_the_parser() {
        let mut raw = arguments(&[]);
        let position = raw
            .iter()
            .position(|value| value == "--terminal-sequence")
            .expect("the flag");
        raw[position + 1] = "0".to_owned();
        let refusal = parse(raw, ExpectedClusterV1::OwnedLoopback)
            .expect_err("a zero terminal sequence must refuse");
        assert!(
            format!("{refusal}").contains("wire refuses zero"),
            "got {refusal}"
        );
        assert!(
            AdvanceRecoveryInstructionV1::new(1, 0).is_err(),
            "and the wire is the reason: it refuses the same value"
        );
    }

    /// The loopback arm does not take the devnet acknowledgment, and the
    /// refusal names the command that does.
    #[test]
    fn the_loopback_arm_refuses_the_devnet_acknowledgment() {
        let refusal = parse(
            arguments(&[DEVNET_ACKNOWLEDGMENT_FLAG, "whatever"]),
            ExpectedClusterV1::OwnedLoopback,
        )
        .expect_err("the loopback arm takes no devnet acknowledgment");
        assert!(format!("{refusal}").contains(COMMAND_V1), "got {refusal}");
    }
}
