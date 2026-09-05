//! First-execution driver for Direct capability activation.
//!
//! One Core-signed, permissionless transaction creates the Direct capability
//! root: Core's capability route validates the manifest-selected entry and its
//! funding ledger, CPIs Trading's outer, and the outer writes
//! `CapabilityRootHeaderV1 || DirectRootStateV1` at the root PDA while moving
//! the ledger's parked rent quote into it. Every fact this driver states is a
//! projection of an author that already exists: the sealed campaign evidence
//! and market input (records, ledger, root coordinate), the live chain (Market
//! state, generation, manifest body), and the codec's own activation bundle
//! (`direct_activation_request_v1`). Nothing here restates a layout.
//!
//! The route needs an address lookup table (the 35-account frame exceeds a
//! packet uncompressed); the founding's own `publish_routing_table` author
//! builds it. Idempotence: a live Trading-owned root at the derived coordinate
//! reports `already-active` and exits cleanly, so a rerun after any
//! interruption converges instead of double-submitting.
//!
//! # Two acknowledged endpoints, ONE author
//!
//! The route is the same on an acknowledged devnet endpoint and on an owned
//! loopback validator, so it is written once and entered twice
//! (`run_devnet`/`run_owned_loopback`), exactly as `sponsored_push` and the
//! General sibling already do. The cluster reaches the body as an
//! [`ExpectedClusterV1`] and decides exactly three things: whether
//! `--i-mean-devnet` is required or refused, which cluster the campaign
//! evidence must have been produced against, and the report schema. Every
//! other fact is derived from the chain and from the sealed artifacts, so
//! neither endpoint can drift from the other by editing one of them.
//!
//! **Why the loopback entry exists.** Until it did, the Direct execution root
//! could not be created on a local validator AT ALL — nothing else in this
//! repository creates one (Core `ActivateCapability` CPIs Trading's
//! `process_activation`, and only this frame reaches it), and the devnet entry
//! refuses a loopback origin before it reads anything. Every local Direct
//! trade therefore refused at the trade producer's root check for as long as
//! local Direct trades have existed, at every market width. That is the wall
//! `docs/evidence/SIMULATOR_POPULATION_DRIVEN_2026_08_30.md` recorded as
//! twenty-one refused fills and read as a width problem; it was an absence.

use std::path::PathBuf;

use dclutch_market::capability_manifest::{
    CapabilityManifestV1, FundingLedgerStatusV2, FundingLedgerV2,
    capability_dependency_closure_mask_v1,
};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
};
use dclutch_core_contract::ContentId;
use dclutch_trading::{
    activation_bundle_v1::direct_activation_request_v1,
    successor::{DIRECT_ROOT_STATE_BYTES_V1, DirectRootStateV1},
};
use dclutch_market::{
    Action, CapabilityFundingHeaderV2, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState,
    Identity, Phase as CorePhase, Request, Role,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use serde_json::json;
use sha2::{Digest as _, Sha256};
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    signature::{Keypair, Signer as _},
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result, campaign,
    cluster::{ClusterOriginV1, DEVNET_GENESIS_HASH, ExpectedClusterV1},
    direct_trade_producer::resolved_record_v1,
    model::{MarketRunInput, RecordPair, SuccessorPlan},
    plan::pubkey,
    rpc::{Rpc, WritePolicyV1},
};

pub(crate) const DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1: &str =
    "devnet-direct-capability-activation-v1";

/// The same route on an owned loopback validator. The General family already
/// carries this pair; Direct did not, and the missing half is what made every
/// local Direct fill unreachable.
pub(crate) const LOCAL_DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1: &str =
    "local-private-validator-direct-capability-activation-v1";

/// The report schema each endpoint emits.
///
/// The devnet string is unchanged, so a devnet report written before this pair
/// existed and one written after are the same document. The loopback endpoint
/// gets its own name rather than borrowing devnet's: a reader who greps an
/// evidence directory for what a report describes must never find a
/// `devnet-` schema over a run that never left the machine.
const fn report_schema_v1(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => "dclutch-devnet-direct-capability-activation-report-v1",
        ExpectedClusterV1::OwnedLoopback => {
            "dclutch-local-private-validator-direct-capability-activation-report-v1"
        }
    }
}

fn refusal(code: &str, reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: [{code}] {}", reason.as_ref()))
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-direct-capability-activation-v1 \
     --rpc-url DEVNET_HTTPS_URL \
     --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
     --plan ABSOLUTE_JSON --expected-plan-sha256 HEX64 \
     --market-input ABSOLUTE_JSON --expected-market-input-sha256 HEX64 \
     --campaign-report ABSOLUTE_JSON --expected-campaign-report-sha256 HEX64 \
     --payer PUBKEY --payer-keypair ABSOLUTE_RUNTIME_KEYPAIR_JSON \
     --output ABSOLUTE_NEW_JSON [--execute]"
}

pub(crate) fn owned_loopback_usage() -> &'static str {
    "dclutch-local-successor-bootstrap \
     local-private-validator-direct-capability-activation-v1 \
     --rpc-url http://127.0.0.1:PORT/ \
     --plan ABSOLUTE_JSON --expected-plan-sha256 HEX64 \
     --market-input ABSOLUTE_JSON --expected-market-input-sha256 HEX64 \
     --campaign-report ABSOLUTE_JSON --expected-campaign-report-sha256 HEX64 \
     --payer PUBKEY --payer-keypair ABSOLUTE_RUNTIME_KEYPAIR_JSON \
     --output ABSOLUTE_NEW_JSON [--execute]"
}

const fn usage_for(expected: ExpectedClusterV1) -> fn() -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => usage,
        ExpectedClusterV1::OwnedLoopback => owned_loopback_usage,
    }
}

struct ArgumentsV1 {
    rpc_url: String,
    /// Exactly what the caller passed for `--i-mean-devnet`, forwarded to
    /// `ClusterOriginV1::parse` rather than re-decided here. The origin parser
    /// owns the "acknowledgment for a loopback socket" refusal, and it is the
    /// one that can tell the operator which of the two was the typo.
    acknowledgment: Option<String>,
    plan: PathBuf,
    expected_plan_sha256: String,
    market_input: PathBuf,
    expected_market_input_sha256: String,
    campaign_report: PathBuf,
    expected_campaign_report_sha256: String,
    payer: Pubkey,
    payer_keypair: PathBuf,
    output: PathBuf,
    execute: bool,
}

fn parse_arguments(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut expected_plan = None;
    let mut market_input = None;
    let mut expected_market = None;
    let mut campaign_report = None;
    let mut expected_campaign = None;
    let mut payer = None;
    let mut payer_keypair = None;
    let mut output = None;
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        if flag == "--execute" {
            execute = true;
            continue;
        }
        let value = iterator.next().ok_or_else(|| {
            refusal(
                "input/missing-value",
                format!("{flag}; usage: {}", usage_for(expected)()),
            )
        })?;
        let slot = match flag.as_str() {
            "--rpc-url" => &mut rpc_url,
            "--i-mean-devnet" => &mut acknowledgment,
            "--plan" => &mut plan,
            "--expected-plan-sha256" => &mut expected_plan,
            "--market-input" => &mut market_input,
            "--expected-market-input-sha256" => &mut expected_market,
            "--campaign-report" => &mut campaign_report,
            "--expected-campaign-report-sha256" => &mut expected_campaign,
            "--payer" => &mut payer,
            "--payer-keypair" => &mut payer_keypair,
            "--output" => &mut output,
            other => {
                return Err(refusal("input/unknown-flag", other));
            }
        };
        if slot.replace(value).is_some() {
            return Err(refusal("input/repeated-flag", flag));
        }
    }
    match expected {
        // Unchanged for the public endpoint: the acknowledgment is required
        // and must be the exact devnet genesis hash.
        ExpectedClusterV1::Devnet => {
            if acknowledgment.as_deref() != Some(DEVNET_GENESIS_HASH) {
                return Err(refusal(
                    "input/devnet-acknowledgment",
                    format!("--i-mean-devnet must be exactly {DEVNET_GENESIS_HASH}"),
                ));
            }
        }
        // Refused HERE, ahead of the origin parser, because at this point the
        // operator's intent is unambiguous: they typed the private-validator
        // command AND a public-cluster acknowledgment. `ClusterOriginV1` would
        // also refuse, but only by saying one of the two is a typo — which is
        // the right sentence when the command name does not already settle it,
        // and the wrong one when it does.
        ExpectedClusterV1::OwnedLoopback => {
            if acknowledgment.is_some() {
                return Err(refusal(
                    "input/loopback-acknowledgment",
                    format!(
                        "{LOCAL_DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1} runs against an owned \
                         loopback validator, which needs no acknowledgment; \
                         {DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1} is the devnet endpoint"
                    ),
                ));
            }
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| {
            refusal(
                "input/missing-flag",
                format!("{name}; usage: {}", usage_for(expected)()),
            )
        })
    };
    Ok(ArgumentsV1 {
        rpc_url: required(rpc_url, "--rpc-url")?,
        acknowledgment,
        plan: PathBuf::from(required(plan, "--plan")?),
        expected_plan_sha256: required(expected_plan, "--expected-plan-sha256")?,
        market_input: PathBuf::from(required(market_input, "--market-input")?),
        expected_market_input_sha256: required(expected_market, "--expected-market-input-sha256")?,
        campaign_report: PathBuf::from(required(campaign_report, "--campaign-report")?),
        expected_campaign_report_sha256: required(
            expected_campaign,
            "--expected-campaign-report-sha256",
        )?,
        payer: pubkey(&required(payer, "--payer")?)?,
        payer_keypair: PathBuf::from(required(payer_keypair, "--payer-keypair")?),
        output: PathBuf::from(required(output, "--output")?),
        execute,
    })
}

fn pinned(path: &PathBuf, expected: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = std::fs::read(path)
        .map_err(|error| refusal("input/unreadable", format!("{label}: {error}")))?;
    let observed = sha256_hex(&bytes);
    if observed != expected {
        return Err(refusal(
            "input/sha256-mismatch",
            format!("{label} hashes to {observed}, expected {expected}"),
        ));
    }
    Ok(bytes)
}

fn record_pair(
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    evidence: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<RecordPair> {
    resolved_record_v1(plan, market_input, evidence, label)
}

fn meta_pair(pair: &RecordPair) -> Result<[AccountMeta; 2]> {
    Ok([
        AccountMeta::new_readonly(pubkey(&pair.raw)?, false),
        AccountMeta::new_readonly(pubkey(&pair.staging)?, false),
    ])
}

/// The Direct **execution** capability root, and the selection that names it.
///
/// Not to be confused with the founding checkpoint's `direct_capability_root`,
/// which is the FOUNDING-PERMIT namespace address and at which no account can
/// ever exist (see the comment in [`run`] and
/// `docs/design/EVIDENCE_REFRESH_V1.md` §3). This is the address activation
/// creates and the address the terminal sequence means.
pub(crate) struct DirectExecutionRootV1 {
    pub(crate) root: Pubkey,
    pub(crate) selection: dclutch_registry::release_set::CapabilityExecutionSelectionV1,
}

/// Derive the Direct execution capability root from its authors alone.
///
/// Pure: every argument is either a program identity from the pinned plan or a
/// fact read from finalized chain state by the caller. Nothing here is a
/// caller-supplied projection, so a second entry that reaches this function
/// with chain-read arguments derives the same address the activation driver
/// derived, by the same authors, or fails (O-016).
pub(crate) fn direct_execution_root_v1(
    trading: Pubkey,
    release_set: Identity,
    market: Pubkey,
    generation: u64,
    entry_index: u16,
    manifest_body: &[u8],
) -> Result<DirectExecutionRootV1> {
    let manifest = CapabilityManifestV1::decode(manifest_body)
        .map_err(|error| Error::new(format!("capability manifest: {error:?}")))?;
    let entry = manifest
        .entry(entry_index)
        .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;
    let selection = dclutch_registry::release_set::CapabilityExecutionSelectionV1::new(
        entry_index,
        ContentId::new(<[u8; 32]>::from(Sha256::digest(manifest_body)))
            .map_err(|_| Error::new("manifest identity".to_string()))?,
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
    )
    .map_err(|error| Error::new(format!("execution selection: {error:?}")))?;
    let root_header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set.to_bytes())
            .map_err(|_| Error::new("release set".to_string()))?,
        market.to_bytes(),
        generation,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .map_err(|error| Error::new(format!("root header: {error:?}")))?;
    Ok(DirectExecutionRootV1 {
        root: Pubkey::find_program_address(&root_header.seeds().as_slices(), &trading).0,
        selection,
    })
}

pub(crate) fn run_devnet(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::Devnet)
}

pub(crate) fn run_owned_loopback(arguments: Vec<String>) -> Result<()> {
    run(arguments, ExpectedClusterV1::OwnedLoopback)
}

fn run(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<()> {
    let arguments = parse_arguments(arguments, expected)?;
    let plan_bytes = pinned(&arguments.plan, &arguments.expected_plan_sha256, "plan")?;
    let market_bytes = pinned(
        &arguments.market_input,
        &arguments.expected_market_input_sha256,
        "market input",
    )?;
    let campaign_bytes = pinned(
        &arguments.campaign_report,
        &arguments.expected_campaign_report_sha256,
        "campaign report",
    )?;
    if arguments.output.exists() {
        return Err(refusal(
            "output/exists",
            format!("refusing to overwrite {}", arguments.output.display()),
        ));
    }
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)
        .map_err(|error| Error::new(format!("successor plan: {error}")))?;
    let market_input: MarketRunInput = serde_json::from_slice(&market_bytes)
        .map_err(|error| Error::new(format!("market input: {error}")))?;
    // The campaign that founded the Market must have been run against THIS
    // cluster. A devnet founding activated from a loopback socket, or the
    // reverse, would derive every coordinate from evidence about another chain.
    let evidence = campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_bytes,
        expected,
    )?;

    let origin = ClusterOriginV1::parse(&arguments.rpc_url, arguments.acknowledgment.as_deref())?;
    expected.authenticate(&origin)?;
    let mut rpc = Rpc::connect_cluster(
        &origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;

    // ------------------------------------------------------- evidence facts
    // The OPEN market is `founding_market` (generation + 1); the plain `market`
    // label is the Found37 market at the input generation, still in Founding.
    // The trade producer reads `founding_market` too.
    let market = evidence
        .accounts
        .get("founding_market")
        .map(|row| pubkey(&row.address))
        .transpose()?
        .ok_or_else(|| {
            refusal(
                "activation/campaign-market",
                "campaign omitted founding_market",
            )
        })?;
    let funding_ledger = evidence
        .accounts
        .get("direct_trading_funding_ledger")
        .map(|row| pubkey(&row.address))
        .transpose()?
        .ok_or_else(|| {
            refusal(
                "activation/campaign-ledger",
                "campaign omitted direct_trading_funding_ledger",
            )
        })?;
    // The founding checkpoint's `direct_capability_root` is the FOUNDING-PERMIT
    // namespace address (its selection config is the generic-founding preimage
    // digest, decision 0004). No account can ever exist there: both the
    // activation and hot paths force `selection.config == entry.config_id`,
    // so the EXECUTION root derives from the manifest entry below. The permit
    // address is reported for the record, never required.
    let founding_permit_root = evidence
        .checkpoint_direct_capability_root
        .as_deref()
        .map(pubkey)
        .transpose()?
        .or(evidence
            .accounts
            .get("direct_capability_root")
            .map(|row| pubkey(&row.address))
            .transpose()?);
    let entry_index = evidence.direct_selected_manifest_entry_index;

    let realm = record_pair(&plan, &market_input, &evidence, "realm_record")?;
    let manifest_pair = record_pair(
        &plan,
        &market_input,
        &evidence,
        "capability_manifest_record",
    )?;
    let program_set = record_pair(&plan, &market_input, &evidence, "direct_program_set_record")?;
    let config = record_pair(
        &plan,
        &market_input,
        &evidence,
        "direct_execution_config_record",
    )?;
    let activation_profile = record_pair(
        &plan,
        &market_input,
        &evidence,
        "direct_activation_account_profile_record",
    )?;
    let activation_effect = record_pair(
        &plan,
        &market_input,
        &evidence,
        "direct_activation_effect_record",
    )?;
    let activation_descriptor = record_pair(
        &plan,
        &market_input,
        &evidence,
        "direct_activation_descriptor_record",
    )?;

    // --------------------------------------------------------- chain facts
    let market_account = rpc.required_account(market, "Core Market state")?;
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market state: {error:?}")))?;
    if market_account.owner != pubkey(&plan.core.program_id)?
        || market_state.phase != CorePhase::Open
    {
        return Err(refusal(
            "activation/market-phase",
            format!(
                "market {market} is {:?}, owner {}",
                market_state.phase, market_account.owner
            ),
        ));
    }
    let generation = market_state.identity.generation;
    let release_set = market_state.identity.selected_release_set;
    if release_set.to_bytes()
        != <[u8; 32]>::try_from(
            (0..64)
                .step_by(2)
                .map(|index| {
                    u8::from_str_radix(&plan.release_set_id[index..index + 2], 16)
                        .map_err(|_| Error::new("plan release set hex".to_string()))
                })
                .collect::<Result<Vec<u8>>>()?,
        )
        .map_err(|_| Error::new("plan release set width".to_string()))?
    {
        return Err(refusal(
            "activation/release-set",
            "market selects another release set than the plan",
        ));
    }

    let manifest_body = {
        let account = rpc.required_account(pubkey(&manifest_pair.raw)?, "capability manifest")?;
        if sha256_hex(&account.data) != manifest_pair.content_sha256 {
            return Err(refusal(
                "activation/manifest-content",
                "manifest record bytes differ from their sealed digest",
            ));
        }
        account.data
    };
    if market_state.identity.capability_manifest.to_bytes()
        != <[u8; 32]>::from(Sha256::digest(&manifest_body))
    {
        return Err(refusal(
            "activation/manifest-identity",
            "market identity selects another capability manifest",
        ));
    }
    let manifest = CapabilityManifestV1::decode(&manifest_body)
        .map_err(|error| Error::new(format!("capability manifest: {error:?}")))?;
    let entry = manifest
        .entry(entry_index)
        .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;
    let current_slot = rpc.finalized_slot()?;
    if current_slot > entry.activation_deadline_slot() {
        return Err(refusal(
            "activation/deadline-elapsed",
            format!(
                "slot {current_slot} is past the entry's activation deadline {}",
                entry.activation_deadline_slot()
            ),
        ));
    }
    let closure_mask = capability_dependency_closure_mask_v1(manifest, entry_index)
        .map_err(|error| Error::new(format!("dependency closure: {error:?}")))?;
    if closure_mask
        != 1_u16
            .checked_shl(u32::from(entry_index))
            .ok_or_else(|| Error::new("entry index shift".to_string()))?
    {
        return Err(refusal(
            "activation/dependency-closure",
            format!(
                "entry {entry_index} closes over mask {closure_mask:#06b}; this driver carries exactly the one selected Trading ledger"
            ),
        ));
    }

    // The root coordinate: derived from the header seeds and cross-checked
    // against the founding's own checkpoint scalar. The derivation is SHARED
    // with the post-activation evidence refresh
    // (`docs/design/EVIDENCE_REFRESH_V1.md` §3, §4) rather than restated there:
    // a refresh that reached this address by a second implementation could
    // emit a root row describing an account this driver never created.
    let trading = pubkey(&plan.trading.program_id)?;
    let derived = direct_execution_root_v1(
        trading,
        release_set,
        market,
        generation,
        entry_index,
        &manifest_body,
    )?;
    let selection = derived.selection;
    let root = derived.root;
    if let Some(existing) = rpc.account(root)? {
        if existing.owner == trading && !existing.data.is_empty() {
            let tail = existing
                .data
                .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .ok_or_else(|| refusal("activation/root-width", "live root is truncated"))?;
            let state = DirectRootStateV1::decode(tail)
                .map_err(|error| Error::new(format!("live root tail: {error:?}")))?;
            let report = json!({
                "schema": report_schema_v1(expected),
                "verdict": "already-active",
                "market": market.to_string(),
                "root": root.to_string(),
                "rootPhase": format!("{:?}", state.phase()),
                "openMakerRootCount": state.open_maker_root_count(),
            });
            std::fs::write(
                &arguments.output,
                format!("{}\n", serde_json::to_string_pretty(&report)?),
            )?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Ok(());
        }
        if existing.owner != system_program::ID || !existing.data.is_empty() {
            return Err(refusal(
                "activation/root-occupied",
                format!("root {root} is occupied by another owner"),
            ));
        }
    }

    let ledger_account = rpc.required_account(funding_ledger, "Trading funding ledger")?;
    if ledger_account.owner != trading {
        return Err(refusal(
            "activation/ledger-owner",
            "funding ledger is not Trading-owned",
        ));
    }
    let ledger = FundingLedgerV2::decode(&ledger_account.data)
        .map_err(|error| Error::new(format!("funding ledger: {error:?}")))?;
    let authenticated = ledger
        .authenticate(
            ContentId::new(<[u8; 32]>::from(Sha256::digest(&manifest_body)))
                .map_err(|_| Error::new("manifest identity".to_string()))?,
            manifest,
        )
        .map_err(|error| Error::new(format!("funding ledger authentication: {error:?}")))?;
    let slot_state = authenticated
        .slot(entry_index)
        .map_err(|error| Error::new(format!("funding ledger slot: {error:?}")))?;
    if slot_state.status() != FundingLedgerStatusV2::Pending {
        return Err(refusal(
            "activation/ledger-status",
            format!("funding slot is {:?}, not Pending", slot_state.status()),
        ));
    }

    // ------------------------------------------------------- the instruction
    let role_request = {
        let mut bytes = Vec::with_capacity(176);
        bytes.extend_from_slice(&selection.to_bytes());
        bytes.extend_from_slice(
            &CapabilityFundingHeaderV2::new(1, 1, closure_mask)
                .map_err(|error| Error::new(format!("funding header: {error:?}")))?
                .encode(),
        );
        bytes.extend_from_slice(&direct_activation_request_v1());
        bytes
    };
    let role_request_digest = hash(&role_request).to_bytes();
    let context: [u8; 32] = {
        let mut hasher = Sha256::new();
        hasher.update(b"dclutch/direct-activation-context/v1");
        hasher.update(market.to_bytes());
        hasher.update(generation.to_le_bytes());
        hasher.finalize().into()
    };
    let core = pubkey(&plan.core.program_id)?;
    let caller_authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            release_set.to_bytes(),
            market.to_bytes(),
            ExecutionRoleV1::Core,
            context,
            role_request_digest,
        )
        .map_err(|error| Error::new(format!("caller authority seeds: {error:?}")))?
        .as_slices(),
        &core,
    )
    .0;
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::ActivateCapability,
        Role::Trading,
        Identity::new(core.to_bytes()).map_err(|_| Error::new("core identity".to_string()))?,
        Identity::new(caller_authority.to_bytes())
            .map_err(|_| Error::new("authority identity".to_string()))?,
        Identity::new(release_set.to_bytes()).map_err(|_| Error::new("release set".to_string()))?,
        Identity::new(market.to_bytes()).map_err(|_| Error::new("market identity".to_string()))?,
        Identity::new(context).map_err(|_| Error::new("context identity".to_string()))?,
        Identity::new(hash(&market_account.data).to_bytes())
            .map_err(|_| Error::new("market digest".to_string()))?,
        Identity::new(role_request_digest).map_err(|_| Error::new("request digest".to_string()))?,
        generation,
        0,
        0,
        u32::try_from(role_request.len()).map_err(|_| Error::new("request width".to_string()))?,
    )
    .map_err(|error| Error::new(format!("core effect envelope: {error:?}")))?;
    let request = Request::administrative(
        Action::ActivateCapability,
        generation,
        Identity::new(market.to_bytes()).map_err(|_| Error::new("market identity".to_string()))?,
    );
    let mut data = Vec::with_capacity(72 + 280 + role_request.len());
    data.extend_from_slice(
        &request
            .encode()
            .map_err(|error| Error::new(format!("core request: {error:?}")))?,
    );
    data.extend_from_slice(
        &envelope
            .encode()
            .map_err(|error| Error::new(format!("core envelope: {error:?}")))?,
    );
    data.extend_from_slice(&role_request);

    let registry = pubkey(&plan.registry.program_id)?;
    let activation_cache = pubkey(&plan.activation)?;
    let realm_metas = meta_pair(&realm)?;
    let manifest_metas = meta_pair(&manifest_pair)?;
    let set_metas = meta_pair(&program_set)?;
    let config_metas = meta_pair(&config)?;
    let profile_metas = meta_pair(&activation_profile)?;
    let effect_metas = meta_pair(&activation_effect)?;
    let descriptor_metas = meta_pair(&activation_descriptor)?;
    let accounts = vec![
        AccountMeta::new(market, false),
        realm_metas[0].clone(),
        realm_metas[1].clone(),
        manifest_metas[0].clone(),
        manifest_metas[1].clone(),
        AccountMeta::new(funding_ledger, false),
        AccountMeta::new(root, false),
        AccountMeta::new_readonly(activation_cache, false),
        AccountMeta::new_readonly(core, false),
        AccountMeta::new_readonly(pubkey(&plan.core.programdata_id)?, false),
        AccountMeta::new_readonly(trading, false),
        AccountMeta::new_readonly(pubkey(&plan.trading.programdata_id)?, false),
        AccountMeta::new_readonly(pubkey(&plan.resolution.program_id)?, false),
        AccountMeta::new_readonly(pubkey(&plan.resolution.programdata_id)?, false),
        AccountMeta::new_readonly(registry, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(caller_authority, false),
        // Child tail, forwarded verbatim to the Trading outer.
        set_metas[0].clone(),
        set_metas[1].clone(),
        config_metas[0].clone(),
        config_metas[1].clone(),
        profile_metas[0].clone(),
        profile_metas[1].clone(),
        effect_metas[0].clone(),
        effect_metas[1].clone(),
        AccountMeta::new_readonly(activation_cache, false),
        AccountMeta::new_readonly(core, false),
        AccountMeta::new_readonly(pubkey(&plan.core.programdata_id)?, false),
        AccountMeta::new_readonly(trading, false),
        AccountMeta::new_readonly(pubkey(&plan.trading.programdata_id)?, false),
        AccountMeta::new_readonly(registry, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        descriptor_metas[0].clone(),
        descriptor_metas[1].clone(),
    ];
    let instruction = Instruction {
        program_id: core,
        accounts,
        data,
    };

    let facts = json!({
        "schema": report_schema_v1(expected),
        "market": market.to_string(),
        "generation": generation,
        "entryIndex": entry_index,
        "root": root.to_string(),
        "foundingPermitRoot": founding_permit_root.map(|value| value.to_string()),
        "fundingLedger": funding_ledger.to_string(),
        "callerAuthority": caller_authority.to_string(),
        "contextSha256": sha256_hex(&context),
        "roleRequestSha256": sha256_hex(&role_request),
        "activationDeadlineSlot": entry.activation_deadline_slot(),
        "observedSlot": current_slot,
        "instructionAccounts": instruction.accounts.len(),
        "instructionDataBytes": instruction.data.len(),
    });
    if !arguments.execute {
        let report = json!({ "verdict": "planned", "facts": facts });
        std::fs::write(
            &arguments.output,
            format!("{}\n", serde_json::to_string_pretty(&report)?),
        )?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    // ------------------------------------------------------------- execute
    let payer = crate::direct_trade_producer::read_keypair_v1(
        &arguments.payer_keypair,
        "activation payer",
    )?;
    if payer.pubkey() != arguments.payer {
        return Err(refusal(
            "input/payer-identity",
            "payer keypair does not expand to --payer",
        ));
    }
    let mut transactions = Vec::new();
    let (observation, tables) = crate::market::publish_routing_table(
        &mut rpc,
        &payer,
        "DIRECT-ACT",
        std::slice::from_ref(&instruction),
        &mut transactions,
    )?;
    let activation_evidence = rpc.send_v0(
        "activate Direct capability (first capability root)",
        std::slice::from_ref(&instruction),
        &payer,
        observation,
        &tables,
    )?;

    // ------------------------------------------------------- poststate proof
    let live_root = rpc.required_account(root, "created capability root")?;
    if live_root.owner != trading {
        return Err(refusal(
            "activation/poststate-owner",
            "created root is not Trading-owned",
        ));
    }
    let header = CapabilityRootHeaderV1::decode(
        live_root
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or_else(|| refusal("activation/poststate-width", "created root is truncated"))?,
    )
    .map_err(|error| Error::new(format!("created root header: {error:?}")))?;
    let tail = live_root
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or_else(|| refusal("activation/poststate-width", "created root tail missing"))?;
    if tail.len() != DIRECT_ROOT_STATE_BYTES_V1 {
        return Err(refusal(
            "activation/poststate-width",
            format!("created root tail is {} bytes, expected 24", tail.len()),
        ));
    }
    let state = DirectRootStateV1::decode(tail)
        .map_err(|error| Error::new(format!("created root tail: {error:?}")))?;
    if state != DirectRootStateV1::new() || header.market() != market.to_bytes() {
        return Err(refusal(
            "activation/poststate-state",
            "created root does not carry the canonical initial state",
        ));
    }
    let ledger_after = rpc.required_account(funding_ledger, "funding ledger poststate")?;

    let report = json!({
        "verdict": "ACTIVATED",
        "facts": facts,
        "activationSignature": activation_evidence.signature,
        "activationSlot": activation_evidence.slot,
        "feeLamports": activation_evidence.fee_lamports,
        "computeUnitsConsumed": activation_evidence.compute_units_consumed,
        "rootLamports": live_root.lamports,
        "rootBytes": live_root.data.len(),
        "rootPhase": format!("{:?}", state.phase()),
        "ledgerLamportsAfter": ledger_after.lamports,
        "tableTransactions": transactions
            .iter()
            .map(|transaction| json!({
                "label": transaction.label,
                "signature": transaction.signature,
                "slot": transaction.slot,
            }))
            .collect::<Vec<_>>(),
    });
    std::fs::write(
        &arguments.output,
        format!("{}\n", serde_json::to_string_pretty(&report)?),
    )?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ArgumentsV1, DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1,
        LOCAL_DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1, owned_loopback_usage, parse_arguments,
        report_schema_v1, run_owned_loopback, sha256_hex, usage,
    };
    use crate::cluster::{ClusterOriginV1, DEVNET_GENESIS_HASH, ExpectedClusterV1};

    fn argv(pairs: &[(&str, &str)]) -> Vec<String> {
        pairs
            .iter()
            .flat_map(|(flag, value)| [(*flag).to_owned(), (*value).to_owned()])
            .collect()
    }

    fn complete(extra: &[(&str, &str)]) -> Vec<String> {
        let mut base = vec![
            ("--rpc-url", "http://127.0.0.1:34500/"),
            ("--plan", "/tmp/plan.json"),
            ("--expected-plan-sha256", "11"),
            ("--market-input", "/tmp/market.json"),
            ("--expected-market-input-sha256", "22"),
            ("--campaign-report", "/tmp/campaign.json"),
            ("--expected-campaign-report-sha256", "33"),
            ("--payer", "11111111111111111111111111111112"),
            ("--payer-keypair", "/tmp/payer.json"),
            ("--output", "/tmp/out.json"),
        ];
        base.extend_from_slice(extra);
        argv(&base)
    }

    fn parsed(arguments: Vec<String>, expected: ExpectedClusterV1) -> ArgumentsV1 {
        parse_arguments(arguments, expected).expect("arguments")
    }

    /// `ArgumentsV1` deliberately carries no `Debug`: it names a payer keypair
    /// path, and an arguments struct that prints itself is one panic away from
    /// putting that path in a log. So a refusal is read out by hand.
    fn refused(arguments: Vec<String>, expected: ExpectedClusterV1, why: &str) -> String {
        match parse_arguments(arguments, expected) {
            Ok(_) => panic!("{why}"),
            Err(error) => error.to_string(),
        }
    }

    /// The devnet endpoint is EXACTLY what it was. Its acknowledgment is still
    /// required and still checked against the one genesis hash, so the live
    /// devnet flagship's activation route is unchanged by the loopback twin.
    #[test]
    fn the_devnet_endpoint_still_requires_the_exact_genesis_acknowledgment() {
        // The acknowledgment is decided before the origin is ever parsed, so
        // the URL in hand does not enter this refusal.
        let missing = refused(
            complete(&[]),
            ExpectedClusterV1::Devnet,
            "a devnet run without the acknowledgment must refuse",
        );
        assert!(
            missing.contains("[input/devnet-acknowledgment]"),
            "{missing}"
        );
        assert!(missing.contains(DEVNET_GENESIS_HASH), "{missing}");

        let wrong = refused(
            complete(&[("--i-mean-devnet", "not-the-genesis-hash")]),
            ExpectedClusterV1::Devnet,
            "a wrong acknowledgment must refuse",
        );
        assert!(wrong.contains("[input/devnet-acknowledgment]"), "{wrong}");

        let accepted = parsed(
            complete(&[("--i-mean-devnet", DEVNET_GENESIS_HASH)]),
            ExpectedClusterV1::Devnet,
        );
        assert_eq!(
            accepted.acknowledgment.as_deref(),
            Some(DEVNET_GENESIS_HASH)
        );
    }

    /// The loopback endpoint refuses the acknowledgment by NAME, ahead of the
    /// origin parser. The command already settles which cluster was meant, so
    /// "one of these two is a typo" would be the wrong sentence here.
    #[test]
    fn the_loopback_endpoint_refuses_a_devnet_acknowledgment_by_name() {
        let text = refused(
            complete(&[("--i-mean-devnet", DEVNET_GENESIS_HASH)]),
            ExpectedClusterV1::OwnedLoopback,
            "a loopback run carrying a devnet acknowledgment must refuse",
        );
        assert!(text.contains("[input/loopback-acknowledgment]"), "{text}");
        assert!(
            text.contains(LOCAL_DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1),
            "{text}"
        );
        assert!(
            text.contains(DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1),
            "the refusal must name the endpoint the operator probably wanted: {text}"
        );

        let accepted = parsed(complete(&[]), ExpectedClusterV1::OwnedLoopback);
        assert!(accepted.acknowledgment.is_none());
        assert_eq!(accepted.rpc_url, "http://127.0.0.1:34500/");
    }

    /// A loopback endpoint pointed at a public cluster is refused by the shared
    /// cluster authenticator, not by anything this module restates. Reaching
    /// that refusal proves the origin is authenticated before any write policy
    /// is chosen.
    #[test]
    fn each_endpoint_refuses_the_other_endpoints_origin() {
        // The gate `run` actually calls, exercised directly.
        //
        // Driving it through `run_owned_loopback` cannot reach it: the three
        // pinned inputs are read and JSON-parsed FIRST, so any argv this test
        // could invent refuses on a document shape long before the origin is
        // authenticated. An earlier draft pointed those inputs at `/tmp` and
        // admitted the read failure as a pass, which made the test a coin flip
        // on what else lives in `/tmp` — it passed 501 times and failed once
        // on this machine, for exactly that reason. A test that cannot reach
        // its subject should say so by moving, not by widening what it accepts.
        let devnet =
            ClusterOriginV1::parse("https://api.devnet.solana.com", Some(DEVNET_GENESIS_HASH))
                .expect("acknowledged devnet origin");
        let loopback =
            ClusterOriginV1::parse("http://127.0.0.1:34500/", None).expect("loopback origin");

        ExpectedClusterV1::OwnedLoopback
            .authenticate(&loopback)
            .expect("the private endpoint admits its own origin");
        ExpectedClusterV1::Devnet
            .authenticate(&devnet)
            .expect("the public endpoint admits its own origin");

        let public_on_private = ExpectedClusterV1::OwnedLoopback
            .authenticate(&devnet)
            .expect_err("the private endpoint must refuse a public origin")
            .to_string();
        assert!(
            public_on_private.contains("owned loopback"),
            "{public_on_private}"
        );

        let private_on_public = ExpectedClusterV1::Devnet
            .authenticate(&loopback)
            .expect_err("the public endpoint must refuse a loopback origin")
            .to_string();
        assert!(
            private_on_public.contains("refuses loopback"),
            "{private_on_public}"
        );
    }

    /// A refusal writes nothing, and it refuses before opening an RPC socket.
    #[test]
    fn a_refused_activation_creates_no_output_file() {
        let directory = std::env::temp_dir().join(format!(
            "dclutch-activation-refusal-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).expect("owned test directory");
        let plan = directory.join("plan.json");
        std::fs::write(&plan, "{}").expect("test plan");
        let output = directory.join("never-written.json");
        let text = run_owned_loopback(argv(&[
            ("--rpc-url", "http://127.0.0.1:34500/"),
            ("--plan", plan.display().to_string().as_str()),
            ("--expected-plan-sha256", sha256_hex(b"{}").as_str()),
            ("--market-input", plan.display().to_string().as_str()),
            ("--expected-market-input-sha256", sha256_hex(b"{}").as_str()),
            ("--campaign-report", plan.display().to_string().as_str()),
            (
                "--expected-campaign-report-sha256",
                sha256_hex(b"{}").as_str(),
            ),
            ("--payer", "11111111111111111111111111111112"),
            (
                "--payer-keypair",
                directory.join("payer.json").display().to_string().as_str(),
            ),
            ("--output", output.display().to_string().as_str()),
        ]))
        .expect_err("an empty plan document must refuse")
        .to_string();
        assert!(text.contains("successor plan"), "{text}");
        assert!(
            !output.exists(),
            "a refused activation must write nothing at {}",
            output.display()
        );
        std::fs::remove_dir_all(&directory).ok();
    }

    /// Two endpoints, two report schemas, and the devnet one is the string it
    /// has always been.
    #[test]
    fn each_endpoint_names_its_own_cluster_in_its_report_schema() {
        assert_eq!(
            report_schema_v1(ExpectedClusterV1::Devnet),
            "dclutch-devnet-direct-capability-activation-report-v1"
        );
        assert_eq!(
            report_schema_v1(ExpectedClusterV1::OwnedLoopback),
            "dclutch-local-private-validator-direct-capability-activation-report-v1"
        );
        assert_ne!(
            report_schema_v1(ExpectedClusterV1::Devnet),
            report_schema_v1(ExpectedClusterV1::OwnedLoopback),
        );
        assert!(
            !report_schema_v1(ExpectedClusterV1::OwnedLoopback).contains("devnet"),
            "a loopback run must never emit a devnet-labelled schema"
        );
    }

    /// The two usage strings name their own commands and their own origins, so
    /// a reader of `--help` cannot pick the endpoint that will refuse them.
    #[test]
    fn the_two_usage_lines_name_their_own_endpoint_and_origin() {
        assert!(usage().contains(DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1));
        assert!(usage().contains("--i-mean-devnet"));
        assert!(
            owned_loopback_usage().contains(LOCAL_DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1),
            "{}",
            owned_loopback_usage()
        );
        assert!(
            !owned_loopback_usage().contains("--i-mean-devnet"),
            "the loopback usage must not advertise a flag it refuses"
        );
        assert!(owned_loopback_usage().contains("http://127.0.0.1:PORT/"));
    }
}
