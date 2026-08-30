//! First-execution driver for Direct capability activation on devnet.
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

use std::path::PathBuf;

use dclutch_capability_contract::{
    CapabilityManifestV1, FundingLedgerStatusV2, FundingLedgerV2,
    capability_dependency_closure_mask_v1,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
};
use dclutch_core_contract::ContentId;
use dclutch_direct_codec::{
    activation_bundle_v1::direct_activation_request_v1,
    successor::{DIRECT_ROOT_STATE_BYTES_V1, DirectRootStateV1},
};
use dclutch_market_core_codec::{
    Action, CapabilityFundingHeaderV2, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState,
    Identity, Phase as CorePhase, Request, Role,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
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

struct ArgumentsV1 {
    rpc_url: String,
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

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
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
        let value = iterator
            .next()
            .ok_or_else(|| refusal("input/missing-value", format!("{flag}; usage: {}", usage())))?;
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
    if acknowledgment.as_deref() != Some(DEVNET_GENESIS_HASH) {
        return Err(refusal(
            "input/devnet-acknowledgment",
            format!("--i-mean-devnet must be exactly {DEVNET_GENESIS_HASH}"),
        ));
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| refusal("input/missing-flag", format!("{name}; usage: {}", usage())))
    };
    Ok(ArgumentsV1 {
        rpc_url: required(rpc_url, "--rpc-url")?,
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

pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
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
    let evidence = campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_bytes,
        ExpectedClusterV1::Devnet,
    )?;

    let origin = ClusterOriginV1::parse(&arguments.rpc_url, Some(DEVNET_GENESIS_HASH))?;
    ExpectedClusterV1::Devnet.authenticate(&origin)?;
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
        .ok_or_else(|| refusal("activation/campaign-market", "campaign omitted founding_market"))?;
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
    let manifest_pair = record_pair(&plan, &market_input, &evidence, "capability_manifest_record")?;
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
    // against the founding's own checkpoint scalar.
    let selection = dclutch_release_set_contract::CapabilityExecutionSelectionV1::new(
        entry_index,
        ContentId::new(<[u8; 32]>::from(Sha256::digest(&manifest_body)))
            .map_err(|_| Error::new("manifest identity".to_string()))?,
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
    )
    .map_err(|error| Error::new(format!("execution selection: {error:?}")))?;
    let root_header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set.to_bytes()).map_err(|_| Error::new("release set".to_string()))?,
        market.to_bytes(),
        generation,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .map_err(|error| Error::new(format!("root header: {error:?}")))?;
    let trading = pubkey(&plan.trading.program_id)?;
    let (root, _) = Pubkey::find_program_address(&root_header.seeds().as_slices(), &trading);
    if let Some(existing) = rpc.account(root)? {
        if existing.owner == trading && !existing.data.is_empty() {
            let tail = existing
                .data
                .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .ok_or_else(|| refusal("activation/root-width", "live root is truncated"))?;
            let state = DirectRootStateV1::decode(tail)
                .map_err(|error| Error::new(format!("live root tail: {error:?}")))?;
            let report = json!({
                "schema": "dclutch-devnet-direct-capability-activation-report-v1",
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
        "schema": "dclutch-devnet-direct-capability-activation-report-v1",
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
    let payer = crate::direct_trade_producer::read_keypair_v1(&arguments.payer_keypair, "activation payer")?;
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
