//! Create a General capability root on an authenticated cluster, from the
//! PUBLISHED release rather than from a fixture.
//!
//! One Core-signed, permissionless transaction: Core's capability route
//! validates the manifest-selected entry and its funding ledger, CPIs Trading's
//! outer, and the outer writes `CapabilityRootHeaderV1 || GeneralRootV2` at the
//! root PDA while moving the ledger's parked Rent quote into it. The account
//! frame and the envelope are the Direct driver's — that route has executed and
//! created a real capability root — and everything family-shaped comes from
//! General's own published artifacts.
//!
//! # Everything semantic is read off chain
//!
//! This is the whole point of the lane, so it is structural rather than a habit.
//! The campaign report supplies exactly two ROUTING coordinates — the Market
//! address and the Trading funding-ledger address — and nothing else. Every
//! semantic fact is then re-derived:
//!
//! * the manifest record's address comes from the Market's own
//!   `capability_manifest` identity, and its bytes are required to hash to it;
//! * the General entry is FOUND in that manifest by kind, never supplied;
//! * the ProgramSet record's address comes from that entry's `release_id`, and
//!   the set is decoded by `authenticate_general_program_set_v3`, which must
//!   report `SettlementWithActivation`;
//! * the activation descriptor is SELECTED out of that set by
//!   `general_activation_request_v1`, and its record address comes from the
//!   selected reference — schema and identity both;
//! * the account-profile and effect record addresses come from the identities
//!   that on-chain descriptor carries, under the two schemas the activation
//!   codec publishes;
//! * the root address comes from a header built out of the Market's own release
//!   set, generation and entry, and is cross-checked against
//!   `general_capability_root_address_v3`, which is a different author.
//!
//! So a run cannot pass by agreeing with the file that produced it. If the
//! founding published a seven-entry set, this refuses at the profile with
//! nothing signed; if it published the eighth entry but the wrong bytes, the
//! record derivation lands on a vacant address and refuses there.
//!
//! # The poststate is read with General's own decoder
//!
//! `GeneralRootV2::decode` must accept the tail the chain wrote, and the tail
//! must equal both `GeneralRootV2::active` and `general_root_creation_tail_v2`
//! for the exact Market, config identity and generation the chain holds. The
//! selected funding row must equal what `FundingLedgerV2::activate_in_place`
//! computes over the same manifest and slot. Two independent authorities, the
//! same bytes — which is what distinguishes an activation of General from a
//! coincidence of widths.
//!
//! # One cluster value, two independent checks
//!
//! This route used to be loopback-only, and it was loopback-only twice: the
//! campaign evidence was parsed under `OwnedLoopback` and the origin was
//! authenticated against `OwnedLoopback`, four lines apart. Those are two
//! genuinely independent facts — one about the document, one about the
//! endpoint — and a devnet arm that satisfied only one of them would accept a
//! loopback campaign report against a devnet chain, or the reverse.
//!
//! So the cluster is a VALUE, taken once by [`run_owned_loopback`] or
//! [`run_devnet`] and threaded into both checks and into the report's own
//! schema. There is no flag that reaches one check and not the other, because
//! there is no flag: the arm is chosen by which command was typed, and the two
//! checks read the same variable. The acknowledgment is admissible only on the
//! devnet arm, and on the loopback arm it is an unknown flag by name.
//!
//! Idempotent: a live Trading-owned root at the derived coordinate reports
//! `already-active`.

use std::path::PathBuf;

use dclutch_capability_activation_codec::{
    activation_account_profile_schema_v1, activation_effect_schema_v1,
};
use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1, FundingLedgerStatusV2,
    FundingLedgerV2, capability_dependency_closure_mask_v1,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1, CapabilityRootHeaderV1,
    SelectedRecordBumpsV1, set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
};
use dclutch_core_contract::ContentId;
use dclutch_general_adapter_contract::{
    activation_bundle_v1::{
        general_activation_descriptor_schema_v1, general_activation_request_v1,
    },
    release_v3::{GeneralReleaseProfileV1, authenticate_general_program_set_v3},
};
use dclutch_general_config_contract::{
    GENERAL_CAPABILITY_KIND_ID_V1, GENERAL_ROOT_BYTES_V2, GeneralRootV2,
    root::general_root_creation_tail_v2, v3::GeneralConfigV3,
};
use dclutch_market_core_codec::{
    Action, CapabilityFundingHeaderV2, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState,
    Identity, Phase as CorePhase, Request, Role,
};
use dclutch_operator::general_activation_v3::general_capability_root_address_v3;
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use serde_json::json;
use sha2::{Digest as _, Sha256};
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    signature::Signer as _,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result, campaign,
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    model::SuccessorPlan,
    plan::pubkey,
    rpc::{Rpc, WritePolicyV1},
};

pub(crate) const GENERAL_CAPABILITY_ACTIVATION_COMMAND_V1: &str =
    "local-private-validator-general-capability-activation-v1";
pub(crate) const DEVNET_GENERAL_CAPABILITY_ACTIVATION_COMMAND_V1: &str =
    "devnet-general-capability-activation-v1";

const OWNED_LOOPBACK_REPORT_SCHEMA_V1: &str =
    "dclutch-owned-loopback-general-capability-activation-report-v1";
const DEVNET_REPORT_SCHEMA_V1: &str = "dclutch-devnet-general-capability-activation-report-v1";

/// The report's schema is a function of the cluster, so a devnet report can
/// never be read as loopback evidence by a consumer that only checks schema.
const fn report_schema_v1(expected: ExpectedClusterV1) -> &'static str {
    match expected {
        ExpectedClusterV1::Devnet => DEVNET_REPORT_SCHEMA_V1,
        ExpectedClusterV1::OwnedLoopback => OWNED_LOOPBACK_REPORT_SCHEMA_V1,
    }
}

/// Domain separating this activation's caller-authority context from every
/// other family's. Core treats the context as an opaque PDA seed, so the only
/// requirement is that two families never collide on one authority address.
const GENERAL_ACTIVATION_CONTEXT_DOMAIN_V1: &[u8] = b"dclutch/general-activation-context/v1";

fn refusal(code: &str, reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: [{code}] {}", reason.as_ref()))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn sha256_hex(bytes: &[u8]) -> String {
    sha256(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-general-capability-activation-v1 \
     --rpc-url http://127.0.0.1:PORT/ \
     --plan ABSOLUTE_JSON --campaign-report ABSOLUTE_JSON \
     --payer-keypair ABSOLUTE_DISPOSABLE_JSON \
     --output ABSOLUTE_NEW_JSON [--execute]\n  \
     dclutch-local-successor-bootstrap devnet-general-capability-activation-v1 \
     --rpc-url URL --i-mean-devnet GENESIS_HASH \
     --plan ABSOLUTE_JSON --campaign-report ABSOLUTE_JSON \
     --payer-keypair ABSOLUTE_DISPOSABLE_JSON \
     --output ABSOLUTE_NEW_JSON [--execute]"
}

struct ArgumentsV1 {
    rpc_url: String,
    plan: PathBuf,
    campaign_report: PathBuf,
    payer_keypair: PathBuf,
    output: PathBuf,
    execute: bool,
    /// The cluster BOTH the campaign evidence and the origin are checked
    /// against. One field, because they are one decision stated once.
    expected: ExpectedClusterV1,
    /// Present only on the devnet arm; the loopback arm refuses the flag by
    /// name rather than ignoring it.
    acknowledgment: Option<String>,
}

fn parse_arguments(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut plan = None;
    let mut campaign_report = None;
    let mut payer_keypair = None;
    let mut output = None;
    let mut acknowledgment = None;
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
            "--plan" => &mut plan,
            "--campaign-report" => &mut campaign_report,
            "--payer-keypair" => &mut payer_keypair,
            "--output" => &mut output,
            crate::cluster::DEVNET_ACKNOWLEDGMENT_FLAG if expected == ExpectedClusterV1::Devnet => {
                &mut acknowledgment
            }
            other => return Err(refusal("input/unknown-flag", other)),
        };
        if slot.replace(value).is_some() {
            return Err(refusal("input/repeated-flag", flag));
        }
    }
    let required = |value: Option<String>, name: &str| {
        value.ok_or_else(|| refusal("input/missing-flag", format!("{name}; usage: {}", usage())))
    };
    Ok(ArgumentsV1 {
        rpc_url: required(rpc_url, "--rpc-url")?,
        plan: PathBuf::from(required(plan, "--plan")?),
        campaign_report: PathBuf::from(required(campaign_report, "--campaign-report")?),
        payer_keypair: PathBuf::from(required(payer_keypair, "--payer-keypair")?),
        output: PathBuf::from(required(output, "--output")?),
        execute,
        expected,
        acknowledgment,
    })
}

/// The raw and staging coordinates of one finalized record.
///
/// Derived from the two facts a record PDA is keyed by, never read from a file:
/// this is the same derivation `outer.rs::finalized_record_coordinates` performs
/// on chain, so an address this produces is the address the seam will require.
#[derive(Clone, Copy, Debug)]
struct RecordCoordinateV1 {
    raw: Pubkey,
    staging: Pubkey,
}

/// Bind one schema/identity pair to its two record coordinates.
///
/// The two seed domains are NOT spelled here. `dclutch-record-contract` owns
/// `RAW_RECORD_PDA_SEED_V1` and `STAGING_CURSOR_PDA_SEED_V1` and also exports
/// the constructors that place them, so a module that merely READS these
/// addresses takes each domain from `seeds.domain()`. A second spelling is a
/// second source of truth, and the failure mode when two spellings drift is an
/// address that silently stops resolving — which on this route means a
/// `required_account` refusal naming a vacant PDA and no cause.
///
/// `SchemaReleaseId` and `ContentDigest` both refuse zero. Zero here means the
/// chain handed this driver a record identity no record can have, so it is a
/// refusal rather than an address.
fn record_coordinate(
    registry: &Pubkey,
    schema: [u8; 32],
    content: [u8; 32],
) -> Result<RecordCoordinateV1> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema)
            .map_err(|_| refusal("activation/record-schema", "a record schema was all zero"))?,
        ContentDigest::new(content).map_err(|_| {
            refusal(
                "activation/record-identity",
                "a record identity was all zero",
            )
        })?,
    );
    Ok(RecordCoordinateV1 {
        raw: record_address(registry, key.raw_record_pda_seeds()),
        staging: record_address(registry, key.staging_cursor_pda_seeds()),
    })
}

fn record_address(registry: &Pubkey, seeds: RecordPdaSeedsV1) -> Pubkey {
    Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        registry,
    )
    .0
}

impl RecordCoordinateV1 {
    fn metas(self) -> [AccountMeta; 2] {
        [
            AccountMeta::new_readonly(self.raw, false),
            AccountMeta::new_readonly(self.staging, false),
        ]
    }
}

/// Read a finalized record's body and require it to hash to the identity its
/// own address was derived from.
fn read_record(
    rpc: &mut Rpc,
    registry: &Pubkey,
    coordinate: RecordCoordinateV1,
    content: [u8; 32],
    label: &str,
) -> Result<Vec<u8>> {
    let account = rpc.required_account(coordinate.raw, label)?;
    if account.owner != *registry {
        return Err(refusal(
            "activation/record-owner",
            format!("{label} at {} is not Registry-owned", coordinate.raw),
        ));
    }
    if sha256(&account.data) != content {
        return Err(refusal(
            "activation/record-content",
            format!("{label} bytes do not hash to the identity its address names"),
        ));
    }
    Ok(account.data)
}

/// Activate a General capability on an owned loopback validator.
pub(crate) fn run_owned_loopback(arguments: Vec<String>) -> Result<()> {
    run_with_cluster_v1(arguments, ExpectedClusterV1::OwnedLoopback)
}

/// Activate a General capability on acknowledged devnet.
///
/// Same route, same derivations, same refusals. What changes is one value,
/// and it changes BOTH of the two independent cluster checks below.
pub(crate) fn run_devnet(arguments: Vec<String>) -> Result<()> {
    run_with_cluster_v1(arguments, ExpectedClusterV1::Devnet)
}

/// This route's two independent cluster checks, in one place, over one value.
///
/// The first says the DOCUMENT is this cluster's; the second says the ENDPOINT
/// is. Neither implies the other — a devnet endpoint will happily serve a run
/// whose campaign report was produced on a loopback validator — and both read
/// the same `arguments.expected`, which the command name chose and no flag can
/// reach.
///
/// It is a function rather than eight inline lines so that a test can drive
/// exactly this. A test that instead re-derived the two checks from
/// `ExpectedClusterV1` would prove the enum works and say nothing about
/// whether this route consults it twice, which is the whole claim.
fn authenticate_cluster_v1(
    arguments: &ArgumentsV1,
    campaign_bytes: &[u8],
) -> Result<(campaign::CampaignTerminalEvidenceV1, ClusterOriginV1)> {
    // ENDPOINT FIRST, and the order is deliberate. `ClusterOriginV1::parse` is
    // the rail that makes accidental mainnet impossible; it costs nothing and
    // it should run before this route reads a document that may be sixteen
    // megabytes. It also means the two refusals are DISTINGUISHABLE from
    // outside: a wrong endpoint is named as a wrong endpoint instead of being
    // shadowed by whatever the document happens to be missing.
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, arguments.acknowledgment.as_deref())?;
    arguments.expected.authenticate(&origin)?;
    let evidence = campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
        campaign_bytes,
        arguments.expected,
    )?;
    Ok((evidence, origin))
}

fn run_with_cluster_v1(arguments: Vec<String>, expected: ExpectedClusterV1) -> Result<()> {
    let arguments = parse_arguments(arguments, expected)?;
    if arguments.output.exists() {
        return Err(refusal(
            "output/exists",
            format!("refusing to overwrite {}", arguments.output.display()),
        ));
    }
    let plan_bytes = std::fs::read(&arguments.plan)
        .map_err(|error| refusal("input/unreadable", format!("plan: {error}")))?;
    let campaign_bytes = std::fs::read(&arguments.campaign_report)
        .map_err(|error| refusal("input/unreadable", format!("campaign report: {error}")))?;
    // Both cluster checks BEFORE the plan is decoded, for the same reason the
    // endpoint check comes before the document check inside them: they are the
    // cheapest thing here and one of them is the accidental-mainnet rail. With
    // the decode first, a plan this route could not read made both refusals
    // unreachable -- which is not a safety hole, since nothing has connected
    // yet, but it did mean the two refusals a caller most needs to see were
    // shadowed by the third. Measured by running the binary, not reasoned:
    // every arm reported `successor plan: missing field schema` for a loopback
    // URL, a devnet URL and a foreign campaign report alike.
    let (evidence, origin) = authenticate_cluster_v1(&arguments, &campaign_bytes)?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)
        .map_err(|error| Error::new(format!("successor plan: {error}")))?;
    let mut rpc = Rpc::connect_cluster(
        &origin,
        if arguments.execute {
            WritePolicyV1::Writes
        } else {
            WritePolicyV1::ReadsOnly
        },
    )?;

    // ------------------------------------------------- the two routing facts
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
                "campaign omitted the Trading funding ledger",
            )
        })?;

    let core = pubkey(&plan.core.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let activation_cache = pubkey(&plan.activation)?;

    // --------------------------------------------------------- the Market
    let market_account = rpc.required_account(market, "Core Market state")?;
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market state: {error:?}")))?;
    if market_account.owner != core || market_state.phase != CorePhase::Open {
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

    // ------------------------------------------- the manifest, and the entry
    let manifest_id = market_state.identity.capability_manifest.to_bytes();
    let manifest_record = record_coordinate(
        &registry,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_id,
    )?;
    let manifest_body = read_record(
        &mut rpc,
        &registry,
        manifest_record,
        manifest_id,
        "capability manifest",
    )?;
    let manifest = CapabilityManifestV1::decode(&manifest_body)
        .map_err(|error| Error::new(format!("capability manifest: {error:?}")))?;

    // The General entry is found, not supplied. Exactly one entry may carry
    // General's kind: two would make "the General capability of this Market"
    // ambiguous, and the seam's merge is what keeps that from happening.
    let mut selected: Option<u16> = None;
    for index in 0..manifest.entry_count() {
        let entry = manifest
            .entry(index)
            .map_err(|error| Error::new(format!("manifest entry {index}: {error:?}")))?;
        if entry.kind_id().to_bytes() == GENERAL_CAPABILITY_KIND_ID_V1
            && selected.replace(index).is_some()
        {
            return Err(refusal(
                "activation/ambiguous-entry",
                "the manifest carries two General entries",
            ));
        }
    }
    let entry_index = selected.ok_or_else(|| {
        refusal(
            "activation/no-general-entry",
            format!("market {market} selected no General capability"),
        )
    })?;
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
    let expected_mask = 1_u16
        .checked_shl(u32::from(entry_index))
        .ok_or_else(|| Error::new("entry index shift".to_string()))?;
    if closure_mask != expected_mask {
        return Err(refusal(
            "activation/dependency-closure",
            format!(
                "entry {entry_index} closes over mask {closure_mask:#018b}; this driver carries exactly the one selected Trading ledger"
            ),
        ));
    }

    // --------------------------- the published set, and the eighth coordinate
    let program_set_record = record_coordinate(
        &registry,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        entry.release_id().to_bytes(),
    )?;
    let program_set_body = read_record(
        &mut rpc,
        &registry,
        program_set_record,
        entry.release_id().to_bytes(),
        "General ProgramSet",
    )?;
    let (set, profile) = authenticate_general_program_set_v3(
        entry.release_id().to_bytes(),
        sha256(&program_set_body),
        &program_set_body,
    )
    .map_err(|error| Error::new(format!("General ProgramSet: {error:?}")))?;
    if !profile.has_activation_entry() {
        return Err(refusal(
            "activation/unactivatable-release",
            format!(
                "the published General release is {profile:?}: it carries no activation coordinate, so no Market founded on it can ever create its capability root"
            ),
        ));
    }
    // THE EXACT PROFILE IS NOT THIS SEAM'S BUSINESS, and asserting it made this
    // driver stale the moment the catalogue grew. It refused anything but
    // `SettlementWithActivation` until 2026-09-02; `5ef3d0a3` published the
    // complete selected-action catalogue, which is sixteen entries rather than
    // eight, so the published set now classifies `CompleteV2WithActivation` and
    // this driver refused a release the project had deliberately moved to.
    //
    // Deleted rather than re-pinned, and that is the point: re-pinning would go
    // stale again at the next catalogue change, and the equality protected
    // nothing. Every address this seam borrows is SELECTED -- the manifest entry
    // by kind, the ProgramSet by that entry's `release_id`, the activation
    // descriptor by `general_activation_request_v1`, the profile and effect
    // records by the identities that descriptor carries -- and not one of them
    // is indexed by catalogue position. The property the seam actually needs is
    // `has_activation_entry`, which the branch immediately above already
    // refuses by name.
    let activation_reference = set
        .select_descriptor(
            &general_activation_request_v1()
                .map_err(|error| Error::new(format!("General activation request: {error:?}")))?,
        )
        .map_err(|error| Error::new(format!("activation descriptor selection: {error:?}")))?;
    if activation_reference.schema().to_bytes() != general_activation_descriptor_schema_v1() {
        return Err(refusal(
            "activation/descriptor-schema",
            "the selected activation entry does not carry the one schema the seam accepts",
        ));
    }

    // --------------------------------- the three records the seam reads next
    let descriptor_record = record_coordinate(
        &registry,
        activation_reference.schema().to_bytes(),
        activation_reference.program().to_bytes(),
    )?;
    let descriptor_body = read_record(
        &mut rpc,
        &registry,
        descriptor_record,
        activation_reference.program().to_bytes(),
        "General activation descriptor",
    )?;
    let descriptor = CapabilityProgramV1::decode(&descriptor_body)
        .map_err(|error| Error::new(format!("General activation descriptor: {error:?}")))?;
    let selection = dclutch_release_set_contract::CapabilityExecutionSelectionV1::new(
        entry_index,
        ContentId::new(manifest_id).map_err(|_| Error::new("manifest identity".to_string()))?,
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
    )
    .map_err(|error| Error::new(format!("execution selection: {error:?}")))?;
    // The same join `authenticate_activation_program` performs on chain, run
    // here so a mismatch refuses before anything is signed.
    descriptor
        .validate_selection(selection, entry)
        .map_err(|error| {
            refusal(
                "activation/descriptor-selection",
                format!("the published activation descriptor does not join this entry: {error:?}"),
            )
        })?;
    if usize::try_from(descriptor.root_state_bytes()).unwrap_or(usize::MAX) != GENERAL_ROOT_BYTES_V2
    {
        return Err(refusal(
            "activation/root-width",
            format!(
                "the activation descriptor declares a {}-byte root tail, not General's {GENERAL_ROOT_BYTES_V2}",
                descriptor.root_state_bytes()
            ),
        ));
    }

    let profile_record = record_coordinate(
        &registry,
        activation_account_profile_schema_v1(),
        descriptor.account_profile().to_bytes(),
    )?;
    read_record(
        &mut rpc,
        &registry,
        profile_record,
        descriptor.account_profile().to_bytes(),
        "General activation account profile",
    )?;
    let effect_record = record_coordinate(
        &registry,
        activation_effect_schema_v1(),
        descriptor.effect_schema().to_bytes(),
    )?;
    read_record(
        &mut rpc,
        &registry,
        effect_record,
        descriptor.effect_schema().to_bytes(),
        "General activation effect",
    )?;

    // ------------------------------------------------- the config, and Realm
    let config_record = record_coordinate(
        &registry,
        descriptor.config_schema().to_bytes(),
        entry.config_id().to_bytes(),
    )?;
    let config_body = read_record(
        &mut rpc,
        &registry,
        config_record,
        entry.config_id().to_bytes(),
        "GeneralConfigV3",
    )?;
    let config = GeneralConfigV3::decode(&config_body)
        .map_err(|error| Error::new(format!("GeneralConfigV3: {error:?}")))?;
    if config.program_set_id() != entry.release_id().to_bytes() {
        return Err(refusal(
            "activation/config-release",
            "the published config names another ProgramSet than the manifest entry does",
        ));
    }
    let realm_record = record_coordinate(
        &registry,
        REALM_SCHEMA_RELEASE_ID_V1,
        market_state.identity.realm_id.to_bytes(),
    )?;

    // ------------------------------------------------------------- the root
    let root_header = CapabilityRootHeaderV1::new(
        ContentId::new(release_set.to_bytes())
            .map_err(|_| Error::new("release set".to_string()))?,
        market.to_bytes(),
        generation,
        selection,
        SelectedRecordBumpsV1::default(),
    )
    .map_err(|error| Error::new(format!("root header: {error:?}")))?;
    let (root, _) = Pubkey::find_program_address(&root_header.seeds().as_slices(), &trading);
    // A second author for the same address: the General operator's planner
    // derives it from the same header without going through this module.
    let (planner_root, _) = general_capability_root_address_v3(root_header, &trading);
    if planner_root != root {
        return Err(refusal(
            "activation/root-derivation",
            "two authors disagree on the General capability-root address",
        ));
    }

    let expected_tail =
        general_root_creation_tail_v2(market.to_bytes(), entry.config_id().to_bytes(), generation)
            .map_err(|error| Error::new(format!("General creation tail: {error:?}")))?;

    if let Some(existing) = rpc.account(root)? {
        if existing.owner == trading && !existing.data.is_empty() {
            let tail = existing
                .data
                .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
                .ok_or_else(|| refusal("activation/root-width", "live root is truncated"))?;
            let state = GeneralRootV2::decode(tail)
                .map_err(|error| Error::new(format!("live root tail: {error:?}")))?;
            let report = json!({
                "schema": report_schema_v1(arguments.expected),
                "verdict": "already-active",
                "market": market.to_string(),
                "root": root.to_string(),
                "entryIndex": entry_index,
                "rootLifecycle": format!("{:?}", state.lifecycle()),
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

    // ----------------------------------------------------- the funding ledger
    let ledger_account = rpc.required_account(funding_ledger, "Trading funding ledger")?;
    if ledger_account.owner != trading {
        return Err(refusal(
            "activation/ledger-owner",
            "funding ledger is not Trading-owned",
        ));
    }
    let ledger_prestate = ledger_account.data.clone();
    let ledger = FundingLedgerV2::decode(&ledger_prestate)
        .map_err(|error| Error::new(format!("funding ledger: {error:?}")))?;
    let authenticated = ledger
        .authenticate(
            ContentId::new(manifest_id).map_err(|_| Error::new("manifest identity".to_string()))?,
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
        bytes.extend_from_slice(
            &general_activation_request_v1()
                .map_err(|error| Error::new(format!("General activation request: {error:?}")))?,
        );
        bytes
    };
    let role_request_digest = hash(&role_request).to_bytes();
    let context: [u8; 32] = {
        let mut hasher = Sha256::new();
        hasher.update(GENERAL_ACTIVATION_CONTEXT_DOMAIN_V1);
        hasher.update(market.to_bytes());
        hasher.update(generation.to_le_bytes());
        hasher.finalize().into()
    };
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

    let realm_metas = realm_record.metas();
    let manifest_metas = manifest_record.metas();
    let set_metas = program_set_record.metas();
    let config_metas = config_record.metas();
    let profile_metas = profile_record.metas();
    let effect_metas = effect_record.metas();
    let descriptor_metas = descriptor_record.metas();
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
        "schema": report_schema_v1(arguments.expected),
        "cluster": arguments.expected.evidence_label(),
        "rpcUrl": origin.redacted_url(),
        "planSha256": sha256_hex(&plan_bytes),
        "campaignReportSha256": sha256_hex(&campaign_bytes),
        "market": market.to_string(),
        "generation": generation,
        "entryIndex": entry_index,
        "releaseProfile": format!("{profile:?}"),
        "programSetRecord": program_set_record.raw.to_string(),
        "activationDescriptorRecord": descriptor_record.raw.to_string(),
        "activationAccountProfileRecord": profile_record.raw.to_string(),
        "activationEffectRecord": effect_record.raw.to_string(),
        "configRecord": config_record.raw.to_string(),
        "root": root.to_string(),
        "fundingLedger": funding_ledger.to_string(),
        "callerAuthority": caller_authority.to_string(),
        "contextSha256": sha256_hex(&context),
        "roleRequestSha256": sha256_hex(&role_request),
        "expectedRootTailSha256": sha256_hex(&expected_tail),
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
    let mut transactions = Vec::new();
    let (observation, tables) = crate::market::publish_routing_table(
        &mut rpc,
        &payer,
        "GENERAL-ACT",
        std::slice::from_ref(&instruction),
        &mut transactions,
    )?;
    let activation_evidence = rpc.send_v0(
        "activate General capability (create the composite root)",
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
    if tail.len() != GENERAL_ROOT_BYTES_V2 {
        return Err(refusal(
            "activation/poststate-width",
            format!(
                "created root tail is {} bytes, expected {GENERAL_ROOT_BYTES_V2}",
                tail.len()
            ),
        ));
    }
    if header.market() != market.to_bytes() {
        return Err(refusal(
            "activation/poststate-header",
            "created root header names another Market",
        ));
    }
    // General's own decoder, and General's own creation oracle. The seam has no
    // family decoder by design, so this is where "a General root" is checked.
    let state = GeneralRootV2::decode(tail)
        .map_err(|error| Error::new(format!("created root tail: {error:?}")))?;
    let canonical =
        GeneralRootV2::active(market.to_bytes(), entry.config_id().to_bytes(), generation)
            .map_err(|error| Error::new(format!("canonical active root: {error:?}")))?;
    if state != canonical || tail != expected_tail.as_slice() {
        return Err(refusal(
            "activation/poststate-state",
            "the created root is not what GeneralRootV2::active composes for this Market",
        ));
    }

    // The second authority on the funding half: General's own ledger transition,
    // run over the same manifest and the slot the chain recorded.
    let ledger_after = rpc.required_account(funding_ledger, "funding ledger poststate")?;
    let manifest_identity =
        ContentId::new(manifest_id).map_err(|_| Error::new("manifest identity".to_string()))?;
    let activation_slot = FundingLedgerV2::decode(&ledger_after.data)
        .map_err(|error| Error::new(format!("funding ledger poststate: {error:?}")))?
        .authenticate(manifest_identity, manifest)
        .map_err(|error| Error::new(format!("funding ledger poststate: {error:?}")))?
        .slot(entry_index)
        .map_err(|error| Error::new(format!("funding ledger poststate slot: {error:?}")))?
        .activation_slot();
    let mut expected_ledger = ledger_prestate.clone();
    FundingLedgerV2::activate_in_place(
        &mut expected_ledger,
        manifest_identity,
        manifest,
        entry_index,
        activation_slot,
    )
    .map_err(|error| Error::new(format!("selected funding activation: {error:?}")))?;
    if expected_ledger != ledger_after.data {
        return Err(refusal(
            "activation/poststate-funding",
            "the funding ledger poststate is not what FundingLedgerV2::activate_in_place computes",
        ));
    }

    let report = json!({
        "verdict": "ACTIVATED",
        "facts": facts,
        "activationSignature": activation_evidence.signature,
        "activationSlot": activation_evidence.slot,
        "feeLamports": activation_evidence.fee_lamports,
        "computeUnitsConsumed": activation_evidence.compute_units_consumed,
        "rootLamports": live_root.lamports,
        "rootBytes": live_root.data.len(),
        "rootTailSha256": sha256_hex(tail),
        "rootLifecycle": format!("{:?}", state.lifecycle()),
        "fundingActivationSlot": activation_slot,
        "ledgerLamportsAfter": ledger_after.lamports,
        "payer": payer.pubkey().to_string(),
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
    use super::*;
    use dclutch_operator::general_selected_release_v1::general_selected_release_v1;

    /// The join between the two halves of the publication closure.
    ///
    /// The founding publishes the activation records under schemas the RELEASE
    /// COMPILER names. This driver borrows them under schemas it derives from
    /// the on-chain set and descriptor and never sees the compiler's list. Those
    /// are two authorities for one address, and if they disagree the failure is
    /// a live refusal on a validator with a founding already spent -- a
    /// `required_account` on a vacant PDA, with nothing to say why.
    ///
    /// So this compiles a real release and requires the address this driver
    /// derives for each of the three records to be the address the publisher
    /// will finalize. It runs offline, in milliseconds, and it fails at the
    /// exact record that drifted.
    #[test]
    fn the_addresses_this_driver_borrows_are_the_ones_the_founding_finalizes() {
        let registry = Pubkey::new_from_array([0x77; 32]);
        let release = general_selected_release_v1(crate::general_market::test_release_input_v1())
            .expect("General release");
        let records = release.publication_records().expect("publication records");
        let published = |label: &str| {
            let record = records
                .iter()
                .find(|record| record.label == label)
                .unwrap_or_else(|| panic!("{label} record"));
            record_coordinate(&registry, record.schema, record.content_id())
                .expect("published record coordinate")
        };

        // Everything below starts from the published ProgramSet BYTES, which is
        // the only thing this driver has on chain besides the manifest entry.
        let program_set_id = sha256(&release.program_set);
        let (set, profile) = authenticate_general_program_set_v3(
            program_set_id,
            program_set_id,
            &release.program_set,
        )
        .expect("published General set");
        assert!(
            profile.has_activation_entry(),
            "a published General release must carry the activation coordinate"
        );
        // The catalogue size is deliberately NOT asserted; see the seam's own
        // comment. What the driver needs is the activation coordinate.
        assert!(profile.has_activation_entry());

        let reference = set
            .select_descriptor(&general_activation_request_v1().expect("activation request"))
            .expect("activation entry");
        assert_eq!(
            reference.schema().to_bytes(),
            general_activation_descriptor_schema_v1()
        );
        let descriptor_record = record_coordinate(
            &registry,
            reference.schema().to_bytes(),
            reference.program().to_bytes(),
        )
        .expect("record coordinate");
        assert_eq!(
            descriptor_record.raw,
            published("activation-descriptor").raw,
            "the descriptor this driver borrows is not the one the founding finalizes"
        );
        assert_eq!(
            descriptor_record.staging,
            published("activation-descriptor").staging
        );

        // And the other two come off that descriptor, exactly as they do on a
        // validator: the driver never reads the compiler's record list.
        let descriptor =
            CapabilityProgramV1::decode(&release.activation.descriptor).expect("descriptor");
        let profile_record = record_coordinate(
            &registry,
            activation_account_profile_schema_v1(),
            descriptor.account_profile().to_bytes(),
        )
        .expect("record coordinate");
        let effect_record = record_coordinate(
            &registry,
            activation_effect_schema_v1(),
            descriptor.effect_schema().to_bytes(),
        )
        .expect("record coordinate");
        assert_eq!(
            profile_record.raw,
            published("activation-account-profile").raw
        );
        assert_eq!(
            profile_record.staging,
            published("activation-account-profile").staging
        );
        assert_eq!(effect_record.raw, published("activation-effect").raw);
        assert_eq!(
            effect_record.staging,
            published("activation-effect").staging
        );

        // The config and the set are borrowed the same way, and they are the
        // two the seven actions share.
        let config_record = record_coordinate(
            &registry,
            descriptor.config_schema().to_bytes(),
            sha256(&release.config),
        )
        .expect("record coordinate");
        assert_eq!(config_record.raw, published("config").raw);
        let set_record = record_coordinate(
            &registry,
            CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            program_set_id,
        )
        .expect("record coordinate");
        assert_eq!(set_record.raw, published("program-set").raw);

        // All five are distinct accounts; a collision would mean two records
        // finalized at one PDA and an activation frame reading the wrong bytes.
        let addresses = [
            set_record.raw,
            config_record.raw,
            profile_record.raw,
            effect_record.raw,
            descriptor_record.raw,
        ];
        for (index, left) in addresses.iter().enumerate() {
            for (other, right) in addresses.iter().enumerate() {
                assert!(index == other || left != right, "record address collision");
            }
        }
    }

    /// The activation frame is the one the Direct route has already executed.
    ///
    /// Both families carry exactly one funding ledger, so the account count and
    /// the child tail are identical; only the sixteen family request bytes
    /// differ. Pinning the count here is what makes a silently dropped meta a
    /// test failure rather than a validator refusal with a spent founding.
    #[test]
    fn the_general_activation_request_is_domain_separated_and_selector_bearing() {
        let request = general_activation_request_v1().expect("activation request");
        assert_eq!(request.len(), 16);
        assert_eq!(&request[..8], b"DCGNACT1");
        assert_ne!(
            request.as_slice(),
            dclutch_direct_codec::activation_bundle_v1::direct_activation_request_v1().as_slice(),
            "two families must not present the same activation request bytes"
        );
    }

    // ---------------- the devnet arm, and its two independent checks

    /// A campaign report carrying only the three fields the cluster gate reads.
    ///
    /// Deliberately incomplete: a report that clears the gate then refuses on
    /// `genesis_hash`, which is how these tests tell "passed the cluster check"
    /// apart from "passed everything" without building a whole campaign.
    fn cluster_labelled_report(label: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema": "dclutch-successor-campaign-report-v1",
            "cluster": label,
            "mode": "execute",
        }))
        .expect("report fixture")
    }

    fn cluster_arguments(expected: ExpectedClusterV1) -> ArgumentsV1 {
        let (rpc_url, acknowledgment) = match expected {
            ExpectedClusterV1::Devnet => (
                "https://api.devnet.solana.com/".to_owned(),
                Some(crate::cluster::DEVNET_GENESIS_HASH.to_owned()),
            ),
            ExpectedClusterV1::OwnedLoopback => ("http://127.0.0.1:8899/".to_owned(), None),
        };
        ArgumentsV1 {
            rpc_url,
            plan: PathBuf::from("/plan.json"),
            campaign_report: PathBuf::from("/campaign.json"),
            payer_keypair: PathBuf::from("/payer.json"),
            output: PathBuf::from("/out.json"),
            execute: false,
            expected,
            acknowledgment,
        }
    }

    /// **The claim this arm exists to make**, driven through the route's own
    /// `authenticate_cluster_v1` rather than re-derived from the enum.
    ///
    /// Four cells per arm. The endpoint axis is varied by swapping the
    /// arguments' own origin; the document axis by swapping the report's
    /// label. A hardcoded cluster in EITHER check turns two of the eight cells
    /// green that must be red — measured: pinning the evidence parse to
    /// `OwnedLoopback` fails this test on the devnet arm's own-report cell.
    #[test]
    fn each_arm_takes_both_the_document_check_and_the_endpoint_check() {
        for (arm, own_label, foreign_label) in [
            (ExpectedClusterV1::Devnet, "devnet", "loopback"),
            (ExpectedClusterV1::OwnedLoopback, "loopback", "devnet"),
        ] {
            let own_origin = cluster_arguments(arm);
            let foreign_origin = cluster_arguments(match arm {
                ExpectedClusterV1::Devnet => ExpectedClusterV1::OwnedLoopback,
                ExpectedClusterV1::OwnedLoopback => ExpectedClusterV1::Devnet,
            });
            // The foreign origin under this arm's expectation: same endpoint
            // shape as the other cluster, still checked against `arm`.
            let crossed = ArgumentsV1 {
                expected: arm,
                ..foreign_origin
            };

            // Own document, own endpoint: clears both gates and refuses later,
            // on the field this fixture omits on purpose.
            let admitted =
                authenticate_cluster_v1(&own_origin, &cluster_labelled_report(own_label))
                    .expect_err("the fixture omits genesis_hash on purpose");
            assert!(
                admitted.0.contains("omitted genesis_hash"),
                "{arm:?} must clear both cluster gates for its own pair: {}",
                admitted.0
            );

            // Foreign document, own endpoint.
            let document =
                authenticate_cluster_v1(&own_origin, &cluster_labelled_report(foreign_label))
                    .expect_err("a foreign cluster's report is non-consumable");
            assert!(
                document.0.contains("non-consumable"),
                "{arm:?} must refuse a {foreign_label} report: {}",
                document.0
            );

            // Own document, foreign endpoint.
            let endpoint = authenticate_cluster_v1(&crossed, &cluster_labelled_report(own_label))
                .expect_err("a foreign endpoint is refused");
            assert!(
                endpoint.0.contains("refuses loopback")
                    || endpoint.0.contains("refuses every external origin")
                    || endpoint
                        .0
                        .contains(crate::cluster::DEVNET_ACKNOWLEDGMENT_FLAG),
                "{arm:?} must refuse the other cluster's endpoint: {}",
                endpoint.0
            );
        }
    }

    /// The acknowledgment is a devnet-arm flag. On the loopback arm it is an
    /// unknown flag BY NAME rather than a value quietly accepted and ignored —
    /// which is the shape that lets someone believe they targeted devnet.
    #[test]
    fn the_acknowledgment_is_admissible_only_on_the_devnet_arm() {
        let arguments = || {
            vec![
                "--rpc-url".to_owned(),
                "https://api.devnet.solana.com/".to_owned(),
                "--plan".to_owned(),
                "/plan.json".to_owned(),
                "--campaign-report".to_owned(),
                "/campaign.json".to_owned(),
                "--payer-keypair".to_owned(),
                "/payer.json".to_owned(),
                "--output".to_owned(),
                "/out.json".to_owned(),
                crate::cluster::DEVNET_ACKNOWLEDGMENT_FLAG.to_owned(),
                crate::cluster::DEVNET_GENESIS_HASH.to_owned(),
            ]
        };
        let devnet = parse_arguments(arguments(), ExpectedClusterV1::Devnet)
            .expect("the devnet arm takes the acknowledgment");
        assert_eq!(
            devnet.acknowledgment.as_deref(),
            Some(crate::cluster::DEVNET_GENESIS_HASH)
        );
        assert_eq!(devnet.expected, ExpectedClusterV1::Devnet);
        let refusal = match parse_arguments(arguments(), ExpectedClusterV1::OwnedLoopback) {
            Err(error) => error,
            Ok(_) => panic!("the loopback arm has no acknowledgment"),
        };
        assert!(
            refusal.0.contains("input/unknown-flag")
                && refusal
                    .0
                    .contains(crate::cluster::DEVNET_ACKNOWLEDGMENT_FLAG),
            "{}",
            refusal.0
        );
    }

    /// Two clusters, two report schemas. A consumer that authenticates only the
    /// schema string therefore cannot read a devnet activation as loopback
    /// evidence, or the reverse.
    #[test]
    fn the_report_schema_is_a_function_of_the_cluster() {
        assert_ne!(
            report_schema_v1(ExpectedClusterV1::Devnet),
            report_schema_v1(ExpectedClusterV1::OwnedLoopback)
        );
        assert!(report_schema_v1(ExpectedClusterV1::Devnet).contains("devnet"));
        assert!(report_schema_v1(ExpectedClusterV1::OwnedLoopback).contains("owned-loopback"));
    }
}
