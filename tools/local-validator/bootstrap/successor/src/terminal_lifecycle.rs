//! Read-only production of outside terminal-lifecycle signing manifests.
//!
//! Persisted campaign evidence supplies routing hints, never state authority.
//! These producers use those hints to name bounded finalized observations,
//! reauthenticate each complete protocol graph through its existing semantic
//! owner, and emit unsigned wallet/web handoffs. They neither read keys nor
//! sign or submit a transaction.

use std::{collections::BTreeMap, io::Write, path::PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_claims_svm::liability_basis_state_v2::{
    LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
};
use dclutch_market_core_codec::{Action, CoreState, Phase, Readiness, Request};
use dclutch_operator::{
    Finality, Observation, ObservedAccount,
    resolution_core_v3::{
        ResolutionCloseFundReportV3, ResolutionCloseFundSnapshotV3, build_resolution_close_fund_v3,
    },
};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_resolution_codec::{
    SOURCE_CLOSURE_RECEIPT_BYTES_V3, SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
};
use dclutch_source_contract::{
    RECOVERY_POLICY_SCHEMA_ID_V2, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SourceResolutionPhaseV1, SourceResolutionStateV2,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_program::{
    hash::hashv,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::transfer;

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    model::SuccessorPlan,
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, WritePolicyV1},
    wallet_terminal::{
        FinalizedSnapshotV1, INPUT_FORMAT, LookupTableRequirementV1, PlanInputV1,
        ProgramSelectorsV1, RecordPairV1, RecordSelectorsV1, SelectedInputV1, authenticate_role,
        build_report, record_pair,
    },
};

const PARENT_CONTEXT_DOMAIN_V1: &[u8] = b"dclutch/wallet-terminal-parent-context/v1";
const LIFECYCLE_PLAN_FORMAT_V1: &str = "dclutch-terminal-lifecycle-plan-v1";
const LIFECYCLE_PRESTATE_DOMAIN_V1: &[u8] = b"dclutch/terminal-lifecycle-prestate/v1";

const TERMINAL_COMPOSITION_LABELS_V1: [&str; 4] = [
    "terminal_composition_descriptor_record",
    "terminal_composition_graph_record",
    "terminal_composition_translation_record",
    "terminal_composition_exposure_record",
];

pub(crate) const DIRECT_BEGIN_RETIRING_LABELS_V1: [&str; 3] = [
    "direct_begin_retiring_account_profile_record",
    "direct_begin_retiring_effect_record",
    "direct_begin_retiring_descriptor_record",
];

pub(crate) const DIRECT_NATIVE_CLOSE_LABELS_V1: [&str; 3] = [
    "direct_native_close_account_profile_record",
    "direct_native_close_effect_record",
    "direct_native_close_descriptor_record",
];

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PayoutEvidenceV1 {
    pub(crate) plan_sha256: String,
    #[serde(rename = "foundingCustodyContext")]
    pub(crate) founding_custody_context: String,
    #[serde(rename = "directSelectedManifestEntryIndex")]
    pub(crate) direct_selected_manifest_entry_index: u16,
    pub(crate) accounts: BTreeMap<String, PayoutAccountEvidenceV1>,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct PayoutAccountEvidenceV1 {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) data_sha256: String,
}

struct ArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    owner: Pubkey,
    recipient: Pubkey,
    claim_index: u32,
    quantity: Option<u64>,
}

struct LifecycleArgumentsV1 {
    origin: ClusterOriginV1,
    plan: PathBuf,
    evidence: PathBuf,
    market: Pubkey,
    payer: Pubkey,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecyclePlanV1 {
    format: &'static str,
    stage: &'static str,
    observation: LifecycleObservationV1,
    market: String,
    transaction_fee_payer: String,
    required_signers: Vec<String>,
    prestate_sha256: String,
    instructions: Vec<LifecycleInstructionV1>,
    expected_poststate: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolution_close_resume: Option<ResolutionCloseResumeV1>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleObservationV1 {
    slot: u64,
    unix_timestamp: i64,
    finality: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleInstructionV1 {
    program_id: String,
    accounts: Vec<LifecycleAccountMetaV1>,
    data_base64: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LifecycleAccountMetaV1 {
    address: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolutionCloseResumeV1 {
    source_state: String,
    closure_receipt: String,
    terminal_sequence: u64,
    closure_sequence: u64,
    role_request_digest: String,
    expected_refund_lamports: u64,
    expected_retirement_facts_sha256: String,
    expected_retirement_facts: ResolutionRetirementFactsV1,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ResolutionRetirementFactsV1 {
    market: String,
    generation: u64,
    resolution_closure_receipt: String,
    source_state: String,
    source_material: String,
    capability_manifest: String,
    terminal_certificate: String,
    beneficiary: String,
    selector: u32,
    terminal_sequence: u64,
    source_state_sha256: String,
    terminal_certificate_sha256: String,
    funding_set_sha256: String,
    source_refund_lamports: u64,
    ledger_remaining_native_principal: u64,
    ledger_rent_lamports: u64,
    ledger_lamport_surplus: u64,
    refund_lamports: u64,
    closed_at: u64,
}

pub(crate) fn run_wallet_terminal_input(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments(arguments)?;
    let plan_source = std::fs::read(&arguments.plan)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let evidence: PayoutEvidenceV1 = serde_json::from_slice(&std::fs::read(&arguments.evidence)?)?;
    authenticate_plan_source(&plan_source, &evidence.plan_sha256)?;
    require_terminal_composition_evidence(&evidence)?;

    let mut input = routed_input(&plan, &evidence, &arguments)?;
    let routed = SelectedInputV1::parse(&input, LookupTableRequirementV1::Absent)?;
    authenticate_routing_hints(&routed, &evidence)?;

    let addresses = routed.addresses();
    let mut rpc = Rpc::connect_cluster(&arguments.origin, WritePolicyV1::ReadsOnly)?;
    let floor = rpc.finalized_slot()?;
    let (slot, values) = rpc.finalized_accounts(&addresses, floor)?;
    let snapshot = FinalizedSnapshotV1::from_rpc(slot, rpc.block_time(slot)?, &addresses, values)?;

    let position_account = snapshot.required(routed.position, "Claims Position")?;
    let position = LiabilityBasisPositionViewV2::decode(&position_account.data)
        .map_err(|error| Error::new(format!("Claims Position: {error:?}")))?;
    let full_balance = position
        .balance(&position_account.data, arguments.claim_index)
        .map_err(|error| Error::new(format!("Claims Position balance: {error:?}")))?;
    let quantity = arguments.quantity.unwrap_or(full_balance);
    if quantity == 0 || quantity > full_balance {
        return Err(Error::new(format!(
            "payout quantity must be within 1..={full_balance} atoms at claim index {}",
            arguments.claim_index
        )));
    }
    input.quantity = quantity.to_string();
    input.parent_context = hex(&stable_parent_context_v1(
        &routed,
        &snapshot,
        quantity,
        arguments.claim_index,
    )?);

    let selected = SelectedInputV1::parse(&input, LookupTableRequirementV1::Absent)?;
    if selected.addresses() != addresses {
        return Err(Error::new(
            "wallet payout selectors changed after authenticated quantity/context construction",
        ));
    }
    let _authenticated = build_report(&selected, &snapshot)?;
    eprintln!(
        "wallet-terminal-payout-input: authenticated one finalized snapshot at slot {}",
        snapshot.observation.slot
    );
    stdout_json(&input)
}

/// Produce exactly one permissionless terminal-lifecycle mutation from a
/// chain-authenticated finalized prestate. The output is unsigned and safe to
/// persist before a wallet is asked to sign it.
pub(crate) fn run_terminal_lifecycle_plan(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_lifecycle_arguments(arguments)?;
    let plan_source = std::fs::read(&arguments.plan)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_source)?;
    let evidence: PayoutEvidenceV1 = serde_json::from_slice(&std::fs::read(&arguments.evidence)?)?;
    authenticate_plan_source(&plan_source, &evidence.plan_sha256)?;
    require_direct_retirement_evidence(&evidence)?;
    let persisted_market = pubkey(&required_account(&evidence, "founding_market")?.address)?;
    if persisted_market != arguments.market {
        return Err(Error::new(format!(
            "--market {} does not match founding_market evidence {persisted_market}",
            arguments.market
        )));
    }

    let core = pubkey(&plan.core.program_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let mut rpc = Rpc::connect_cluster(&arguments.origin, WritePolicyV1::ReadsOnly)?;
    let routing = finalized_snapshot(&mut rpc, &[arguments.market])?;
    let market_account = routing.required(arguments.market, "Core Market")?;
    let market = decode_routed_market(market_account, core, &plan)?;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            arguments.market.as_ref(),
            &market.identity.generation.to_le_bytes(),
        ],
        &resolution,
    )
    .0;

    let output = match market.phase {
        Phase::Terminal => plan_begin_retiring(&mut rpc, &plan, &evidence, &arguments, market)?,
        Phase::Retiring => {
            let source_route = finalized_snapshot(&mut rpc, &[arguments.market, source_state])?;
            let current_market = decode_routed_market(
                source_route.required(arguments.market, "Core Market")?,
                core,
                &plan,
            )?;
            if current_market.phase != Phase::Retiring {
                return Err(Error::new(
                    "Market phase changed while routing the retirement snapshot; rerun",
                ));
            }
            let source = source_route.account(source_state)?;
            if source.lamports == 0 {
                return Err(post_resolution_close_blocker(current_market));
            }
            plan_resolution_close(
                &mut rpc,
                &plan,
                &evidence,
                &arguments,
                current_market,
                source,
            )?
        }
        Phase::Retired => LifecyclePlanV1 {
            format: LIFECYCLE_PLAN_FORMAT_V1,
            stage: "complete",
            observation: lifecycle_observation(routing.observation),
            market: arguments.market.to_string(),
            transaction_fee_payer: arguments.payer.to_string(),
            required_signers: Vec::new(),
            prestate_sha256: hex(&lifecycle_prestate_digest(&routing)),
            instructions: Vec::new(),
            expected_poststate: BTreeMap::from([("marketPhase".into(), "Retired".into())]),
            resolution_close_resume: None,
        },
        Phase::Founding | Phase::Open => {
            return Err(Error::new(format!(
                "Market is {:?}, not terminal; provider terminalization must finalize before retirement",
                market.phase
            )));
        }
    };
    stdout_json(&output)
}

fn plan_begin_retiring(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &PayoutEvidenceV1,
    arguments: &LifecycleArgumentsV1,
    routed_market: CoreState,
) -> Result<LifecyclePlanV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let claims_programdata = pubkey(&plan.claims.programdata_id)?;
    let activation = pubkey(&plan.activation)?;
    let aggregate = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, arguments.market.as_ref()],
        &claims,
    )
    .0;
    let persisted_aggregate = pubkey(&required_account(evidence, "claims_aggregate")?.address)?;
    if aggregate != persisted_aggregate {
        return Err(Error::new(format!(
            "claims_aggregate evidence {persisted_aggregate} is not canonical {aggregate}"
        )));
    }
    let snapshot = finalized_snapshot(
        rpc,
        &[
            arguments.market,
            aggregate,
            registry,
            activation,
            core,
            core_programdata,
            claims,
            claims_programdata,
        ],
    )?;
    let market_account = snapshot.required(arguments.market, "Core Market")?;
    let market = decode_routed_market(market_account, core, plan)?;
    if market != routed_market
        || market.phase != Phase::Terminal
        || market.readiness != Readiness::Consumed
    {
        return Err(Error::new(
            "Market changed or is not Terminal/Consumed in the authoritative BeginRetiring snapshot",
        ));
    }
    let release_set = market.identity.selected_release_set.to_bytes();
    authenticate_role(
        snapshot.required(registry, "Registry program")?,
        snapshot.required(activation, "activation cache")?,
        release_set,
        ExecutionRoleV1::Core,
        snapshot.required(core, "Core program")?,
        snapshot.required(core_programdata, "Core ProgramData")?,
    )?;
    authenticate_role(
        snapshot.required(registry, "Registry program")?,
        snapshot.required(activation, "activation cache")?,
        release_set,
        ExecutionRoleV1::Claims,
        snapshot.required(claims, "Claims program")?,
        snapshot.required(claims_programdata, "Claims ProgramData")?,
    )?;
    authenticate_zero_claims(
        snapshot.required(aggregate, "Claims aggregate")?,
        aggregate,
        claims,
        market,
        hex32(&evidence.founding_custody_context)?,
    )?;

    let instruction = Instruction {
        program_id: core,
        accounts: vec![
            AccountMeta::new(arguments.market, false),
            AccountMeta::new_readonly(activation, false),
            AccountMeta::new_readonly(registry, false),
            AccountMeta::new_readonly(core, false),
            AccountMeta::new_readonly(core_programdata, false),
        ],
        data: Request::administrative(
            Action::BeginRetiring,
            market.identity.generation,
            market.identity.market_id,
        )
        .encode()
        .map_err(|error| Error::new(format!("BeginRetiring request: {error:?}")))?
        .to_vec(),
    };
    Ok(lifecycle_plan(
        "begin-retiring",
        arguments,
        &snapshot,
        vec![instruction],
        BTreeMap::from([
            ("marketPhase".into(), "Retiring".into()),
            ("claimsSupply".into(), "0".into()),
        ]),
        None,
    ))
}

fn plan_resolution_close(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    evidence: &PayoutEvidenceV1,
    arguments: &LifecycleArgumentsV1,
    market: CoreState,
    routed_source: &ObservedAccount,
) -> Result<LifecyclePlanV1> {
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let core_programdata = pubkey(&plan.core.programdata_id)?;
    let resolution = pubkey(&plan.resolution.program_id)?;
    let resolution_programdata = pubkey(&plan.resolution.programdata_id)?;
    let activation = pubkey(&plan.activation)?;
    let source_material = routed_record(
        evidence,
        "source_material_record",
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let capability_manifest = routed_record(
        evidence,
        "capability_manifest_record",
        registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let recovery_policy = match evidence.accounts.get("recovery_policy_record") {
        Some(_) => routed_record(
            evidence,
            "recovery_policy_record",
            registry,
            RECOVERY_POLICY_SCHEMA_ID_V2,
        )?,
        None => source_material,
    };
    let source = SourceResolutionStateV2::decode(&routed_source.data)
        .map_err(|error| Error::new(format!("Resolution Source state: {error:?}")))?;
    if !matches!(
        source.phase(),
        SourceResolutionPhaseV1::Resolved | SourceResolutionPhaseV1::FailureCommitted
    ) {
        return Err(Error::new(format!(
            "Resolution Source is {:?}; provider resolution/terminal admission must finish before close",
            source.phase()
        )));
    }
    let terminal = source
        .terminal_projection()
        .map_err(|error| Error::new(format!("Resolution Source terminal projection: {error:?}")))?;
    let closure_sequence = terminal
        .terminal_sequence()
        .checked_add(1)
        .ok_or_else(|| Error::new("Resolution closure sequence overflow"))?;
    let closure_receipt = Pubkey::find_program_address(
        &[
            SOURCE_CLOSURE_RECEIPT_PDA_DOMAIN_V3,
            routed_source.key.as_ref(),
            &closure_sequence.to_le_bytes(),
        ],
        &resolution,
    )
    .0;
    let certificate = market
        .terminal_receipt
        .ok_or_else(|| Error::new("Retiring Market omitted its terminal receipt"))?
        .to_bytes();
    let certificate = Pubkey::new_from_array(certificate);
    let beneficiary = Pubkey::new_from_array(market.rent_beneficiary.to_bytes());
    let funding_ledger =
        pubkey(&required_account(evidence, "founding_funding_ledger_v2_0")?.address)?;
    let keys = [
        arguments.market,
        activation,
        registry,
        core,
        core_programdata,
        resolution,
        resolution_programdata,
        source_material.raw,
        source_material.staging,
        capability_manifest.raw,
        capability_manifest.staging,
        routed_source.key,
        funding_ledger,
        certificate,
        closure_receipt,
        beneficiary,
        sysvar::clock::ID,
        sysvar::rent::ID,
        system_program::ID,
        recovery_policy.raw,
        recovery_policy.staging,
    ];
    let snapshot = finalized_snapshot(rpc, &keys)?;
    let current_market = decode_routed_market(
        snapshot.required(arguments.market, "Core Market")?,
        core,
        plan,
    )?;
    if current_market != market || current_market.phase != Phase::Retiring {
        return Err(Error::new(
            "Market changed while constructing the authoritative Resolution close snapshot; rerun",
        ));
    }
    let close_snapshot = ResolutionCloseFundSnapshotV3 {
        market: observed(&snapshot, arguments.market)?,
        activation_cache: observed(&snapshot, activation)?,
        registry_program: observed(&snapshot, registry)?,
        core_program: observed(&snapshot, core)?,
        core_programdata: observed(&snapshot, core_programdata)?,
        resolution_program: observed(&snapshot, resolution)?,
        resolution_programdata: observed(&snapshot, resolution_programdata)?,
        source_material: observed(&snapshot, source_material.raw)?,
        source_material_staging: observed(&snapshot, source_material.staging)?,
        capability_manifest: observed(&snapshot, capability_manifest.raw)?,
        capability_manifest_staging: observed(&snapshot, capability_manifest.staging)?,
        source_state: observed(&snapshot, routed_source.key)?,
        funding_ledger: observed(&snapshot, funding_ledger)?,
        certificate: observed(&snapshot, certificate)?,
        closure_destination: observed(&snapshot, closure_receipt)?,
        beneficiary: observed(&snapshot, beneficiary)?,
        clock_sysvar: observed(&snapshot, sysvar::clock::ID)?,
        rent_sysvar: observed(&snapshot, sysvar::rent::ID)?,
        system_program: observed(&snapshot, system_program::ID)?,
        recovery_policy: observed(&snapshot, recovery_policy.raw)?,
        recovery_policy_staging: observed(&snapshot, recovery_policy.staging)?,
    };
    let rent: Rent = bincode::deserialize(&close_snapshot.rent_sysvar.data)
        .map_err(|error| Error::new(format!("Rent sysvar: {error}")))?;
    let receipt_rent = rent.minimum_balance(SOURCE_CLOSURE_RECEIPT_BYTES_V3);
    if close_snapshot.closure_destination.owner != system_program::ID
        || close_snapshot.closure_destination.executable
        || !close_snapshot.closure_destination.data.is_empty()
    {
        return Err(Error::new(
            "Resolution closure receipt destination is not a vacant System account",
        ));
    }
    if close_snapshot.closure_destination.lamports > receipt_rent {
        return Err(Error::new(format!(
            "Resolution closure receipt carries {} lamports but exact rent is {receipt_rent}; surplus prepayment is refused",
            close_snapshot.closure_destination.lamports
        )));
    }
    if close_snapshot.closure_destination.lamports < receipt_rent {
        let mut projected = close_snapshot.clone();
        projected.closure_destination.lamports = receipt_rent;
        let report = build_resolution_close_fund_v3(&projected).map_err(|error| {
            Error::new(format!("Resolution close prepay authentication: {error:?}"))
        })?;
        let top_up = receipt_rent
            .checked_sub(close_snapshot.closure_destination.lamports)
            .ok_or_else(|| Error::new("Resolution closure receipt top-up underflow"))?;
        return Ok(lifecycle_plan(
            "prepay-resolution-closure-receipt",
            arguments,
            &snapshot,
            vec![transfer(&arguments.payer, &closure_receipt, top_up)],
            BTreeMap::from([
                ("closureReceipt".into(), closure_receipt.to_string()),
                ("closureReceiptLamports".into(), receipt_rent.to_string()),
                ("marketPhase".into(), "Retiring".into()),
            ]),
            Some(close_resume(&report)),
        ));
    }
    let report = build_resolution_close_fund_v3(&close_snapshot)
        .map_err(|error| Error::new(format!("Resolution CloseFund: {error:?}")))?;
    Ok(lifecycle_plan(
        "close-resolution-fund",
        arguments,
        &snapshot,
        vec![report.instruction.clone()],
        BTreeMap::from([
            ("closureReceipt".into(), report.closure_receipt.to_string()),
            ("fundingLedger".into(), "closed".into()),
            ("marketPhase".into(), "Retiring".into()),
            ("sourceState".into(), "closed".into()),
        ]),
        Some(close_resume(&report)),
    ))
}

pub(crate) fn decode_routed_market(
    account: &ObservedAccount,
    core: Pubkey,
    plan: &SuccessorPlan,
) -> Result<CoreState> {
    let market = CoreState::decode(&account.data)
        .map_err(|error| Error::new(format!("Core Market: {error:?}")))?;
    if account.owner != core
        || account.executable
        || account.key.to_bytes() != market.identity.market_id.to_bytes()
        || market.identity.registry_program.to_bytes()
            != pubkey(&plan.registry.program_id)?.to_bytes()
        || market.identity.selected_release_set.to_bytes() != hex32(&plan.release_set_id)?
    {
        return Err(Error::new(
            "Core Market owner/address/Registry/release-set routing authentication refused",
        ));
    }
    Ok(market)
}

pub(crate) fn authenticate_zero_claims(
    account: &ObservedAccount,
    expected: Pubkey,
    claims: Pubkey,
    market: CoreState,
    custody_context: [u8; 32],
) -> Result<()> {
    let aggregate = LiabilityBasisMarketViewV2::decode(&account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    if account.key != expected
        || account.owner != claims
        || account.executable
        || aggregate.logical_market != market.identity.market_id.to_bytes()
        || aggregate.release_set != market.identity.selected_release_set.to_bytes()
        || aggregate.registry_program != market.identity.registry_program.to_bytes()
        || aggregate.product_instance_id != market.identity.product_id.to_bytes()
        || aggregate.realm_id != market.identity.realm_id.to_bytes()
        || aggregate.custody_context != custody_context
        || aggregate.generation != market.identity.generation
    {
        return Err(Error::new(
            "Claims aggregate address/owner/Market/release/Product/Realm/custody/generation join refused",
        ));
    }
    for claim_index in 0..aggregate.claim_count {
        let supply = aggregate
            .supply(&account.data, claim_index)
            .map_err(|error| Error::new(format!("Claims supply {claim_index}: {error:?}")))?;
        if supply != 0 {
            return Err(Error::new(format!(
                "BeginRetiring is blocked: Claims supply at index {claim_index} is {supply}; produce and execute wallet terminal payouts first"
            )));
        }
    }
    Ok(())
}

fn post_resolution_close_blocker(market: CoreState) -> Error {
    if market.outstanding_capabilities != 0 {
        return Error::new(format!(
            "native close is blocked with {} outstanding capabilities: the immutable Direct ProgramSet evidence must publish and persist the distinct native-close descriptor/profile/effect labels before this operator can consume them",
            market.outstanding_capabilities
        ));
    }
    Error::new(
        "aggregate retirement is blocked: the authenticated Trading-to-Core Custody replay handoff for the Claims custodyContext is not yet available; Realm and token-account close remain refused",
    )
}

pub(crate) fn routed_record(
    evidence: &PayoutEvidenceV1,
    label: &str,
    registry: Pubkey,
    schema: [u8; 32],
) -> Result<RecordPairV1> {
    let persisted = required_account(evidence, label)?;
    let pair = record_pair(registry, schema, hex32(&persisted.data_sha256)?);
    let persisted_address = pubkey(&persisted.address)?;
    if pair.raw != persisted_address {
        return Err(Error::new(format!(
            "persisted {label} address {persisted_address} is not canonical {}",
            pair.raw
        )));
    }
    Ok(pair)
}

pub(crate) fn finalized_snapshot(rpc: &mut Rpc, keys: &[Pubkey]) -> Result<FinalizedSnapshotV1> {
    let mut keys = keys.to_vec();
    keys.sort_unstable();
    keys.dedup();
    let floor = rpc.finalized_slot()?;
    let (slot, values) = rpc.finalized_accounts(&keys, floor)?;
    FinalizedSnapshotV1::from_rpc(slot, rpc.block_time(slot)?, &keys, values)
}

pub(crate) fn observed(snapshot: &FinalizedSnapshotV1, key: Pubkey) -> Result<ObservedAccount> {
    snapshot.account(key).cloned()
}

fn lifecycle_plan(
    stage: &'static str,
    arguments: &LifecycleArgumentsV1,
    snapshot: &FinalizedSnapshotV1,
    instructions: Vec<Instruction>,
    expected_poststate: BTreeMap<String, String>,
    resolution_close_resume: Option<ResolutionCloseResumeV1>,
) -> LifecyclePlanV1 {
    LifecyclePlanV1 {
        format: LIFECYCLE_PLAN_FORMAT_V1,
        stage,
        observation: lifecycle_observation(snapshot.observation),
        market: arguments.market.to_string(),
        transaction_fee_payer: arguments.payer.to_string(),
        required_signers: vec![arguments.payer.to_string()],
        prestate_sha256: hex(&lifecycle_prestate_digest(snapshot)),
        instructions: instructions
            .into_iter()
            .map(lifecycle_instruction)
            .collect(),
        expected_poststate,
        resolution_close_resume,
    }
}

fn lifecycle_observation(observation: Observation) -> LifecycleObservationV1 {
    LifecycleObservationV1 {
        slot: observation.slot,
        unix_timestamp: observation.unix_timestamp,
        finality: match observation.finality {
            Finality::Finalized => "finalized",
            Finality::Confirmed => "confirmed-invalid",
            Finality::Processed => "processed-invalid",
        },
    }
}

fn lifecycle_instruction(instruction: Instruction) -> LifecycleInstructionV1 {
    LifecycleInstructionV1 {
        program_id: instruction.program_id.to_string(),
        accounts: instruction
            .accounts
            .into_iter()
            .map(|account| LifecycleAccountMetaV1 {
                address: account.pubkey.to_string(),
                is_signer: account.is_signer,
                is_writable: account.is_writable,
            })
            .collect(),
        data_base64: BASE64.encode(instruction.data),
    }
}

fn lifecycle_prestate_digest(snapshot: &FinalizedSnapshotV1) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(LIFECYCLE_PRESTATE_DOMAIN_V1);
    for account in snapshot.accounts.values() {
        hasher.update(account.key.to_bytes());
        hasher.update(account.owner.to_bytes());
        hasher.update(account.lamports.to_le_bytes());
        hasher.update([u8::from(account.executable)]);
        hasher.update(Sha256::digest(&account.data));
    }
    hasher.finalize().into()
}

fn close_resume(report: &ResolutionCloseFundReportV3) -> ResolutionCloseResumeV1 {
    let facts = &report.expected_retirement_facts;
    let facts_digest = hashv(&[
        b"dclutch/resolution-retirement-facts/v1",
        &facts.market,
        &facts.generation.to_le_bytes(),
        &facts.resolution_closure_receipt,
        &facts.source_state,
        &facts.source_material,
        &facts.capability_manifest,
        &facts.terminal_certificate,
        &facts.beneficiary,
        &facts.selector.to_le_bytes(),
        &facts.terminal_sequence.to_le_bytes(),
        &facts.source_state_digest,
        &facts.terminal_certificate_digest,
        &facts.funding_set_digest,
        &facts.source_refund_lamports.to_le_bytes(),
        &facts.ledger_remaining_native_principal.to_le_bytes(),
        &facts.ledger_rent_lamports.to_le_bytes(),
        &facts.ledger_lamport_surplus.to_le_bytes(),
        &facts.refund_lamports.to_le_bytes(),
        &facts.closed_at.to_le_bytes(),
    ])
    .to_bytes();
    ResolutionCloseResumeV1 {
        source_state: Pubkey::new_from_array(facts.source_state).to_string(),
        closure_receipt: report.closure_receipt.to_string(),
        terminal_sequence: report.terminal_sequence,
        closure_sequence: report.closure_sequence,
        role_request_digest: hex(&report.role_request_digest),
        expected_refund_lamports: report.expected_refund_lamports,
        expected_retirement_facts_sha256: hex(&facts_digest),
        expected_retirement_facts: ResolutionRetirementFactsV1 {
            market: Pubkey::new_from_array(facts.market).to_string(),
            generation: facts.generation,
            resolution_closure_receipt: Pubkey::new_from_array(facts.resolution_closure_receipt)
                .to_string(),
            source_state: Pubkey::new_from_array(facts.source_state).to_string(),
            source_material: hex(&facts.source_material),
            capability_manifest: hex(&facts.capability_manifest),
            terminal_certificate: Pubkey::new_from_array(facts.terminal_certificate).to_string(),
            beneficiary: Pubkey::new_from_array(facts.beneficiary).to_string(),
            selector: facts.selector,
            terminal_sequence: facts.terminal_sequence,
            source_state_sha256: hex(&facts.source_state_digest),
            terminal_certificate_sha256: hex(&facts.terminal_certificate_digest),
            funding_set_sha256: hex(&facts.funding_set_digest),
            source_refund_lamports: facts.source_refund_lamports,
            ledger_remaining_native_principal: facts.ledger_remaining_native_principal,
            ledger_rent_lamports: facts.ledger_rent_lamports,
            ledger_lamport_surplus: facts.ledger_lamport_surplus,
            refund_lamports: facts.refund_lamports,
            closed_at: facts.closed_at,
        },
    }
}

fn routed_input(
    plan: &SuccessorPlan,
    evidence: &PayoutEvidenceV1,
    arguments: &ArgumentsV1,
) -> Result<PlanInputV1> {
    let record_digest = |label: &str| -> Result<String> {
        Ok(required_account(evidence, label)?.data_sha256.clone())
    };
    let mint = required_account(evidence, "collateral_mint")?;
    Ok(PlanInputV1 {
        format: INPUT_FORMAT.into(),
        market: arguments.market.to_string(),
        owner: arguments.owner.to_string(),
        recipient_owner: arguments.owner.to_string(),
        recipient: arguments.recipient.to_string(),
        collateral_mint: mint.address.clone(),
        token_program: mint.owner.clone(),
        // Quantity and parent context do not select accounts. They are filled
        // from the authenticated snapshot before this input is emitted.
        quantity: "1".into(),
        claim_index: arguments.claim_index,
        transfer_index: 0,
        parent_context: hex(&[1; 32]),
        custody_context: evidence.founding_custody_context.clone(),
        release_set: plan.release_set_id.clone(),
        lookup_table: None,
        programs: ProgramSelectorsV1 {
            registry: plan.registry.program_id.clone(),
            core: plan.core.program_id.clone(),
            claims: plan.claims.program_id.clone(),
            custody: plan.custody.program_id.clone(),
        },
        records: RecordSelectorsV1 {
            realm: record_digest("realm_record")?,
            product: record_digest("product_record")?,
            result_domain: record_digest("result_domain_record")?,
            portfolio: record_digest("portfolio_record")?,
            product_basis: record_digest("linked_liability_basis_record")?,
            composition_descriptor: record_digest(TERMINAL_COMPOSITION_LABELS_V1[0])?,
            composition_graph: record_digest(TERMINAL_COMPOSITION_LABELS_V1[1])?,
            composition_translation: record_digest(TERMINAL_COMPOSITION_LABELS_V1[2])?,
            composition_exposure: record_digest(TERMINAL_COMPOSITION_LABELS_V1[3])?,
            terminal_record: record_digest("terminal_record")?,
        },
    })
}

fn authenticate_routing_hints(
    selected: &SelectedInputV1,
    evidence: &PayoutEvidenceV1,
) -> Result<()> {
    let expected = [
        ("realm_record", selected.realm.raw),
        ("product_record", selected.product.raw),
        ("result_domain_record", selected.result_domain.raw),
        ("portfolio_record", selected.portfolio.raw),
        ("linked_liability_basis_record", selected.product_basis.raw),
        (
            TERMINAL_COMPOSITION_LABELS_V1[0],
            selected.composition_descriptor.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[1],
            selected.composition_graph.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[2],
            selected.composition_translation.raw,
        ),
        (
            TERMINAL_COMPOSITION_LABELS_V1[3],
            selected.composition_exposure.raw,
        ),
        ("terminal_record", selected.terminal_coordinate.raw),
    ];
    for (label, derived) in expected {
        let persisted = pubkey(&required_account(evidence, label)?.address)?;
        if persisted != derived {
            return Err(Error::new(format!(
                "persisted {label} address {persisted} is not the canonical raw-record PDA {derived}"
            )));
        }
    }
    Ok(())
}

fn stable_parent_context_v1(
    selected: &SelectedInputV1,
    snapshot: &FinalizedSnapshotV1,
    quantity: u64,
    claim_index: u32,
) -> Result<[u8; 32]> {
    let market = snapshot.required(selected.market, "Core Market")?;
    let aggregate = snapshot.required(selected.aggregate, "Claims aggregate")?;
    let position = snapshot.required(selected.position, "Claims Position")?;
    let replay = snapshot.required(selected.custody_replay, "Claims Custody replay")?;
    let hoard = snapshot.required(selected.hoard, "Hoard token account")?;
    let recipient = snapshot.required(selected.recipient, "recipient token account")?;
    let market_digest = Sha256::digest(&market.data);
    let aggregate_digest = Sha256::digest(&aggregate.data);
    let position_digest = Sha256::digest(&position.data);
    let replay_digest = Sha256::digest(&replay.data);
    let hoard_digest = Sha256::digest(&hoard.data);
    let recipient_digest = Sha256::digest(&recipient.data);
    let quantity_bytes = quantity.to_le_bytes();
    let claim_index_bytes = claim_index.to_le_bytes();
    let transfer_index_bytes = 0_u16.to_le_bytes();
    let context = hashv(&[
        PARENT_CONTEXT_DOMAIN_V1,
        selected.market.as_ref(),
        selected.owner.as_ref(),
        selected.position.as_ref(),
        selected.recipient.as_ref(),
        &quantity_bytes,
        &claim_index_bytes,
        &transfer_index_bytes,
        &selected.release_set,
        &selected.terminal_record_digest,
        &market_digest,
        &aggregate_digest,
        &position_digest,
        &replay_digest,
        &hoard_digest,
        &recipient_digest,
    ])
    .to_bytes();
    if context == [0; 32] {
        return Err(Error::new("derived wallet payout parent context was zero"));
    }
    Ok(context)
}

pub(crate) fn authenticate_plan_source(source: &[u8], expected: &str) -> Result<()> {
    let expected = hex32(expected)?;
    let observed: [u8; 32] = Sha256::digest(source).into();
    if observed != expected {
        return Err(Error::new(format!(
            "evidence planSha256 {} does not authenticate plan {}",
            hex(&expected),
            hex(&observed)
        )));
    }
    Ok(())
}

fn require_terminal_composition_evidence(evidence: &PayoutEvidenceV1) -> Result<()> {
    let missing = TERMINAL_COMPOSITION_LABELS_V1
        .iter()
        .copied()
        .filter(|label| !evidence.accounts.contains_key(*label))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(Error::new(format!(
            "terminal payout is blocked: canonical native-composition publication evidence is missing {}",
            missing.join(", ")
        )));
    }
    Ok(())
}

pub(crate) fn require_direct_retirement_evidence(evidence: &PayoutEvidenceV1) -> Result<()> {
    for label in DIRECT_BEGIN_RETIRING_LABELS_V1
        .into_iter()
        .chain(DIRECT_NATIVE_CLOSE_LABELS_V1)
        .chain([
            "direct_program_set_record",
            "direct_execution_config_record",
            "direct_capability_root",
            "direct_trading_funding_ledger",
        ])
    {
        required_account(evidence, label).map_err(|_| {
            Error::new(format!(
                "terminal sequence is blocked: campaign evidence omitted exact Direct lifecycle label {label}"
            ))
        })?;
    }
    Ok(())
}

pub(crate) fn required_account<'a>(
    evidence: &'a PayoutEvidenceV1,
    label: &str,
) -> Result<&'a PayoutAccountEvidenceV1> {
    evidence
        .accounts
        .get(label)
        .ok_or_else(|| Error::new(format!("payout evidence is missing account label {label}")))
}

fn parse_lifecycle_arguments(arguments: Vec<String>) -> Result<LifecycleArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut evidence = None;
    let mut market = None;
    let mut payer = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--plan" => &mut plan,
            "--evidence" => &mut evidence,
            "--market" => &mut market,
            "--payer" => &mut payer,
            _ => {
                return Err(Error::new(format!(
                    "unknown terminal-lifecycle-plan argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let rpc_url = required(rpc_url, "--rpc-url")?;
    Ok(LifecycleArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: absolute(plan, "--plan")?,
        evidence: absolute(evidence, "--evidence")?,
        market: pubkey(&required(market, "--market")?)?,
        payer: pubkey(&required(payer, "--payer")?)?,
    })
}

fn parse_arguments(arguments: Vec<String>) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut acknowledgment = None;
    let mut plan = None;
    let mut evidence = None;
    let mut market = None;
    let mut owner = None;
    let mut recipient = None;
    let mut claim_index = None;
    let mut quantity = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            DEVNET_ACKNOWLEDGMENT_FLAG => &mut acknowledgment,
            "--plan" => &mut plan,
            "--evidence" => &mut evidence,
            "--market" => &mut market,
            "--owner" => &mut owner,
            "--recipient" => &mut recipient,
            "--claim-index" => &mut claim_index,
            "--quantity" => &mut quantity,
            _ => {
                return Err(Error::new(format!(
                    "unknown wallet-terminal-payout-input argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let rpc_url = required(rpc_url, "--rpc-url")?;
    Ok(ArgumentsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, acknowledgment.as_deref())?,
        plan: absolute(plan, "--plan")?,
        evidence: absolute(evidence, "--evidence")?,
        market: pubkey(&required(market, "--market")?)?,
        owner: pubkey(&required(owner, "--owner")?)?,
        recipient: pubkey(&required(recipient, "--recipient")?)?,
        claim_index: canonical_u32(&required(claim_index, "--claim-index")?, "--claim-index")?,
        quantity: quantity
            .map(|value| canonical_u64(&value, "--quantity"))
            .transpose()?,
    })
}

fn canonical_u32(value: &str, label: &str) -> Result<u32> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::new(format!(
            "{label} must be a canonical decimal u32"
        )));
    }
    value
        .parse()
        .map_err(|_| Error::new(format!("{label} must be a canonical decimal u32")))
}

fn canonical_u64(value: &str, label: &str) -> Result<u64> {
    if value.is_empty()
        || value == "0"
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::new(format!(
            "{label} must be a positive canonical decimal u64"
        )));
    }
    value
        .parse()
        .map_err(|_| Error::new(format!("{label} must be a positive canonical decimal u64")))
}

fn required(value: Option<String>, label: &str) -> Result<String> {
    value.ok_or_else(|| Error::new(format!("{label} is required")))
}

fn absolute(value: Option<String>, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(required(value, label)?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be absolute")));
    }
    Ok(path)
}

fn stdout_json(value: &impl serde::Serialize) -> Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(value)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap wallet-terminal-payout-input --rpc-url URL \\
     [--i-mean-devnet DEVNET_GENESIS_HASH] --plan ABSOLUTE_JSON \\
     --evidence ABSOLUTE_JSON --market PUBKEY --owner PUBKEY --recipient PUBKEY \\
     --claim-index U32 [--quantity U64]\n\nThis command is read-only. It uses persisted \
     campaign and terminal-publication evidence only to route one finalized account snapshot, \
     reauthenticates the complete payout graph, derives a crash-stable parent context from the \
     immutable request and authenticated prestate (never the observation slot), and emits the \
     exact dclutch-wallet-terminal-payout-plan-input-v1 accepted by the existing ALT planner. \
     Missing canonical native-composition publication evidence is a hard lifecycle blocker. \
     Mainnet-beta is refused unconditionally."
}

pub(crate) fn lifecycle_usage() -> &'static str {
    "\n  dclutch-local-successor-bootstrap terminal-lifecycle-plan --rpc-url URL \\
     [--i-mean-devnet DEVNET_GENESIS_HASH] --plan ABSOLUTE_JSON \\
     --evidence ABSOLUTE_JSON --market PUBKEY --payer PUBKEY\n\nThis command is read-only. \
     It reauthenticates one finalized lifecycle prestate and emits exactly one unsigned next \
     transaction: BeginRetiring after every Claims supply is zero, exact closure-receipt \
     prepayment, or Resolution CloseFund. Persist its manifest before signing so the closure \
     receipt and retirement facts survive a crash after Source closure. Native funding close \
     and aggregate retirement remain hard blockers until their canonical Direct selector and \
     Trading-to-Core replay-handoff evidence exist. Realm/token close is refused. Mainnet-beta \
     is refused unconditionally."
}

#[cfg(test)]
mod tests {
    use dclutch_claims_svm::liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LiabilityBasisMarketInputV2,
        encode_liability_basis_market_into_v2, liability_basis_vector_width_v2,
    };
    use dclutch_market_core_codec::{Identity, MarketIdentity};
    use dclutch_operator::{Finality, Observation, ObservedAccount};
    use solana_sdk_ids::system_program;

    use super::*;

    fn observed(key: Pubkey, tag: u8, slot: u64) -> ObservedAccount {
        ObservedAccount {
            observation: Observation {
                slot,
                unix_timestamp: 1_700_000_000,
                finality: Finality::Finalized,
            },
            key,
            owner: system_program::ID,
            lamports: 1,
            executable: false,
            data: vec![tag; 32],
        }
    }

    fn context_fixture(slot: u64) -> (SelectedInputV1, FinalizedSnapshotV1) {
        let mut value = super::super::wallet_terminal::tests::input();
        value.lookup_table = None;
        let selected = SelectedInputV1::parse(&value, LookupTableRequirementV1::Absent)
            .expect("selected input");
        let keys = [
            selected.market,
            selected.aggregate,
            selected.position,
            selected.custody_replay,
            selected.hoard,
            selected.recipient,
        ];
        let accounts = keys
            .into_iter()
            .enumerate()
            .map(|(index, key)| (key, observed(key, u8::try_from(index + 1).unwrap(), slot)))
            .collect();
        (
            selected,
            FinalizedSnapshotV1 {
                observation: Observation {
                    slot,
                    unix_timestamp: 1_700_000_000,
                    finality: Finality::Finalized,
                },
                accounts,
            },
        )
    }

    fn identity(value: u8) -> Identity {
        Identity::new([value; 32]).expect("identity")
    }

    fn terminal_market(market: Pubkey, outstanding_capabilities: u64) -> CoreState {
        CoreState {
            phase: Phase::Terminal,
            readiness: Readiness::Consumed,
            terminal_winner: 1,
            identity: MarketIdentity {
                market_id: Identity::new(market.to_bytes()).unwrap(),
                realm_id: identity(2),
                product_record: identity(3),
                product_id: identity(4),
                resolution_policy: identity(5),
                capability_manifest: identity(6),
                selected_release_set: identity(7),
                registry_program: identity(8),
                generation: 9,
            },
            outstanding_capabilities,
            principal_cap_sets: 1,
            rent_beneficiary: identity(10),
            terminal_receipt: Some(identity(11)),
        }
    }

    fn claims_aggregate(
        market: CoreState,
        claims: Pubkey,
        custody_context: [u8; 32],
        supplies: &[u64],
    ) -> ObservedAccount {
        let key = Pubkey::find_program_address(
            &[
                LIABILITY_BASIS_MARKET_SEED_V2,
                market.identity.market_id.to_bytes().as_slice(),
            ],
            &claims,
        )
        .0;
        let mut data = vec![
            0;
            liability_basis_vector_width_v2(
                LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
                u32::try_from(supplies.len()).unwrap(),
            )
            .unwrap()
        ];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 1,
                logical_market: market.identity.market_id.to_bytes(),
                release_set: market.identity.selected_release_set.to_bytes(),
                registry_program: market.identity.registry_program.to_bytes(),
                product_instance_id: market.identity.product_id.to_bytes(),
                basis_id: [12; 32],
                realm_id: market.identity.realm_id.to_bytes(),
                custody_context,
                generation: market.identity.generation,
            },
            supplies,
            &mut data,
        )
        .unwrap();
        ObservedAccount {
            observation: Observation {
                slot: 1,
                unix_timestamp: 1,
                finality: Finality::Finalized,
            },
            key,
            owner: claims,
            lamports: 1,
            executable: false,
            data,
        }
    }

    #[test]
    fn retry_context_ignores_observation_slot_but_binds_request_and_prestate() {
        let (selected_a, snapshot_a) = context_fixture(100);
        let (mut selected_b, mut snapshot_b) = context_fixture(200);
        let first = stable_parent_context_v1(&selected_a, &snapshot_a, 7, 1).unwrap();
        let retry = stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap();
        assert_eq!(first, retry, "finalized slot is not caller entropy");

        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 6, 1).unwrap()
        );
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 0).unwrap()
        );
        selected_b.owner = Pubkey::new_unique();
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        selected_b.owner = selected_a.owner;
        selected_b.terminal_record_digest[0] ^= 1;
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        selected_b.terminal_record_digest = selected_a.terminal_record_digest;
        snapshot_b
            .accounts
            .get_mut(&selected_b.custody_replay)
            .expect("replay")
            .data[0] ^= 1;
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
        snapshot_b
            .accounts
            .get_mut(&selected_b.custody_replay)
            .expect("replay")
            .data[0] ^= 1;
        selected_b.recipient = Pubkey::new_unique();
        snapshot_b
            .accounts
            .insert(selected_b.recipient, observed(selected_b.recipient, 6, 200));
        assert_ne!(
            first,
            stable_parent_context_v1(&selected_b, &snapshot_b, 7, 1).unwrap()
        );
    }

    #[test]
    fn context_refuses_a_missing_authenticated_prestate() {
        let (selected, mut snapshot) = context_fixture(100);
        snapshot.accounts.remove(&selected.custody_replay);
        let error = stable_parent_context_v1(&selected, &snapshot, 7, 1)
            .expect_err("missing replay must refuse");
        assert!(error.to_string().contains("snapshot omitted"));
    }

    #[test]
    fn missing_native_composition_is_an_explicit_lifecycle_blocker() {
        let evidence = PayoutEvidenceV1 {
            plan_sha256: hex(&[1; 32]),
            founding_custody_context: hex(&[2; 32]),
            direct_selected_manifest_entry_index: 0,
            accounts: BTreeMap::new(),
        };
        let error = require_terminal_composition_evidence(&evidence)
            .expect_err("missing composition must refuse");
        assert!(error.to_string().contains("canonical native-composition"));
        for label in TERMINAL_COMPOSITION_LABELS_V1 {
            assert!(error.to_string().contains(label));
        }
    }

    #[test]
    fn evidence_uses_the_persisted_campaign_field_names() {
        let decoded: PayoutEvidenceV1 = serde_json::from_value(serde_json::json!({
            "plan_sha256": hex(&[1; 32]),
            "foundingCustodyContext": hex(&[2; 32]),
            "directSelectedManifestEntryIndex": 0,
            "accounts": {
                "terminal_composition_descriptor_record": {
                    "address": Pubkey::new_unique().to_string(),
                    "owner": Pubkey::new_unique().to_string(),
                    "data_sha256": hex(&[3; 32]),
                    "ignoredExistingEvidenceField": true
                }
            },
            "completed": []
        }))
        .expect("campaign evidence projection");
        assert_eq!(decoded.plan_sha256, hex(&[1; 32]));
        assert_eq!(decoded.founding_custody_context, hex(&[2; 32]));
        assert_eq!(decoded.direct_selected_manifest_entry_index, 0);
    }

    #[test]
    fn direct_retirement_requires_exact_three_selector_evidence_labels() {
        let row = || PayoutAccountEvidenceV1 {
            address: Pubkey::new_unique().to_string(),
            owner: Pubkey::new_unique().to_string(),
            data_sha256: hex(&[3; 32]),
        };
        let mut accounts = BTreeMap::new();
        for label in DIRECT_BEGIN_RETIRING_LABELS_V1
            .into_iter()
            .chain(DIRECT_NATIVE_CLOSE_LABELS_V1)
            .chain([
                "direct_program_set_record",
                "direct_execution_config_record",
                "direct_capability_root",
                "direct_trading_funding_ledger",
            ])
        {
            accounts.insert(label.into(), row());
        }
        let exact = PayoutEvidenceV1 {
            plan_sha256: hex(&[1; 32]),
            founding_custody_context: hex(&[2; 32]),
            direct_selected_manifest_entry_index: 0,
            accounts: accounts.clone(),
        };
        require_direct_retirement_evidence(&exact).expect("exact Direct retirement evidence");
        for label in DIRECT_BEGIN_RETIRING_LABELS_V1 {
            let mut hostile = exact.clone();
            hostile.accounts.remove(label);
            let error = require_direct_retirement_evidence(&hostile)
                .expect_err("missing begin-retiring label must refuse");
            assert!(error.to_string().contains(label));
        }
    }

    #[test]
    fn canonical_decimal_parsers_refuse_aliases() {
        assert_eq!(canonical_u32("0", "index").unwrap(), 0);
        assert!(canonical_u32("00", "index").is_err());
        assert_eq!(canonical_u64("1", "quantity").unwrap(), 1);
        assert!(canonical_u64("0", "quantity").is_err());
        assert!(canonical_u64("01", "quantity").is_err());
    }

    #[test]
    fn begin_retiring_requires_every_claim_supply_to_be_zero() {
        let market_key = Pubkey::new_unique();
        let market = terminal_market(market_key, 1);
        let claims = Pubkey::new_unique();
        let custody_context = [13; 32];
        let zero = claims_aggregate(market, claims, custody_context, &[0, 0, 0]);
        assert!(authenticate_zero_claims(&zero, zero.key, claims, market, custody_context).is_ok());

        let live = claims_aggregate(market, claims, custody_context, &[0, 7, 0]);
        let error = authenticate_zero_claims(&live, live.key, claims, market, custody_context)
            .expect_err("live liability must block BeginRetiring");
        assert!(error.to_string().contains("index 1 is 7"));

        let error = authenticate_zero_claims(&zero, zero.key, claims, market, [14; 32])
            .expect_err("substituted custody context must refuse");
        assert!(error.to_string().contains("custody/generation join"));
    }

    #[test]
    fn lifecycle_prestate_digest_ignores_slot_but_binds_account_state() {
        let (_, first) = context_fixture(10);
        let (_, mut retry) = context_fixture(20);
        assert_eq!(
            lifecycle_prestate_digest(&first),
            lifecycle_prestate_digest(&retry),
            "observation slot is not lifecycle request entropy"
        );
        retry
            .accounts
            .values_mut()
            .next()
            .expect("account")
            .lamports += 1;
        assert_ne!(
            lifecycle_prestate_digest(&first),
            lifecycle_prestate_digest(&retry)
        );
    }

    #[test]
    fn post_close_refuses_each_unavailable_semantic_owner_by_name() {
        let market_key = Pubkey::new_unique();
        let native = post_resolution_close_blocker(terminal_market(market_key, 2));
        assert!(native.to_string().contains("native close"));
        assert!(native.to_string().contains("descriptor/profile/effect"));
        let mut aggregate = terminal_market(market_key, 0);
        aggregate.phase = Phase::Retiring;
        let replay = post_resolution_close_blocker(aggregate);
        assert!(
            replay
                .to_string()
                .contains("Trading-to-Core Custody replay handoff")
        );
        assert!(
            replay
                .to_string()
                .contains("Realm and token-account close remain refused")
        );
    }
}
