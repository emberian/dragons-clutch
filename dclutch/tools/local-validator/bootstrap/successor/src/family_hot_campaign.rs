//! One durable production caller shared by the General and Series families.
//!
//! # Why this module exists
//!
//! General and Series both had semantics, artifacts and ProgramTests, and
//! neither had a caller that ever ran against a validator. The project
//! doctrine is that an artifact builder without a caller is parked, not
//! product, so the gap was the whole distance between "the family works" and
//! "the family is real".
//!
//! What the two families share is not their semantics — it is their
//! *transaction discipline*. Both are hot-path capability families addressed
//! through the family-neutral `DCLTHOT3` envelope
//! ([`HotExecutionEnvelopeV3`]), both publish commit-last evidence as
//! top-level program return data, and both need a caller that can be killed at
//! any instant and resumed without ever minting a second transaction identity
//! for one intended action. That discipline is this module; the family
//! semantics stay with the families.
//!
//! # The durable ladder
//!
//! Each action advances one journal file through four phases, and the file is
//! renamed into place so a reader never observes a half-written phase:
//!
//! ```text
//! Planned    the frame and the request exist, no key has been opened
//! Prepared   the exact signed packet, its digest and its last-valid height
//! Submitted  those exact bytes went to the RPC at least once
//! Finalized  the transaction is in finalized history with its ACK
//! ```
//!
//! A restart from `Prepared` may send only the bytes already on disk. A
//! restart from `Submitted` is poll-only and cannot rebuild a signature. This
//! is the same rule `direct_trade.rs` and `sponsored_push.rs` follow, and it
//! is the reason the RPC layer authenticates return data at the same boundary
//! it authenticates the packet: an ACK read from a log line would be a
//! projection the validator may truncate, and commit-last evidence is exactly
//! the claim that must not be weakened.
//!
//! # What each family reaches today, stated exactly
//!
//! **General** reaches the real accelerator ELF. Every one of the seven
//! authored settlement actions
//! ([`dclutch_general_adapter_contract::release_v3::GENERAL_ACTIONS_V3`]) is
//! driven as one finalized localhost transaction whose top-level return data
//! is a typed [`AcceleratorAckV2`]. That is the read-only evaluation half of
//! the family: the accelerator owns no account, signs nothing, and performs no
//! CPI. The commit half — Trading's `process_hot_execution_v3` writing the
//! capability root and returning a `DCLTHAK3` ack — additionally requires a
//! founded Market whose capability manifest selects General, and no driver in
//! this tree founds one. The Market-selection hook that names those
//! requirements exactly is [`general_market_selection_requirements_v1`].
//!
//! **Series** reaches request compilation and no further, and this module says
//! so by refusing rather than by pretending. Its Hot builders
//! (`build_series_{prepare,consume,expire}_hot_v3`) have no caller anywhere in
//! the tree, and `projected_open_composition_v4`, the Consume chain's
//! terminal, has none either. The family request bytes this module compiles
//! are real and are checked against the kernel's own decoder, so the wire is
//! proved; the route that would carry them is not built. Naming that as debt
//! is the honest report, and a caller that "ran" against a route nobody
//! dispatches would not be evidence of anything.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_program_contract::hot_v3::HotExecutionEnvelopeV3;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2, ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorAckV2,
    AcceleratorDispositionV2, AcceleratorRequestV2, AuthenticatedScratchPageV2, RequestTransportV2,
    SCRATCH_PAGE_HEADER_BYTES_V2, ScratchPageKindV2,
};
use dclutch_general_adapter_contract::{
    account_rules_v3::general_account_profile_fixed_count_v3,
    hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_candidate_bank_len_v3,
        general_hot_scalar_count_v3, identity, scalar,
    },
    local_state_v3::{
        GeneralLocalStateHeaderV3, GeneralLocalStateKindV3, encode_general_local_state_v3_atomic,
        general_local_state_len_v3,
    },
    release_v3::GENERAL_ACTIONS_V3,
    runtime_selection::{RUNTIME_SELECTION_CURSOR_BYTES_V2, consider_verified_candidate_v2},
    runtime_settlement::RuntimeSettlementActionV2,
    runtime_width::{VerifiedCandidateHeaderV2, VerifiedCandidateV2, verified_candidate_len},
    state_artifacts_v3::{GeneralReadonlyEvidenceKindV3, general_readonly_evidence_v3},
};
use dclutch_general_codec::{
    Action, MAX_SELECTION_CRITERIA, SelectionCriterion, SelectionPolicyV1,
    successor_request_v2::ControllerRequestV2,
};
use dclutch_general_config_contract::v3::{GeneralConfigV3, GeneralConfigV3Input};
use dclutch_series_v3_kernel::request::{
    SERIES_ACTION_HEADER_BYTES_V3, SeriesActionRequestV3, SeriesActionV3,
    encode_series_action_header_v3,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
};
use solana_sdk_ids::sysvar;

use crate::{
    Error, Result,
    campaign::read_keypair_file,
    general_settlement_fixture::{
        frozen_selection_v1, initialized_cursor_v1, settle_native_v1, settlement_revision_v1,
        settlement_rows_v1, terminal_fixture_v1,
    },
    plan::hex,
    rpc::{Rpc, SignedVersionedPacketV1},
};

/// Command that drives the General family's seven authored actions.
pub(crate) const GENERAL_COMMAND_V1: &str = "local-private-validator-general-hot-campaign-v1";
/// Command that drives the Series family's occurrence actions.
pub(crate) const SERIES_COMMAND_V1: &str = "local-private-validator-series-hot-campaign-v1";

const GENERAL_JOURNAL_SCHEMA_V1: &str = "dclutch-local-general-hot-campaign-journal-v1";
const SERIES_JOURNAL_SCHEMA_V1: &str = "dclutch-local-series-hot-campaign-journal-v1";
const GENERAL_EVIDENCE_SCHEMA_V1: &str = "dclutch-local-general-hot-campaign-evidence-v1";
const SERIES_EVIDENCE_SCHEMA_V1: &str = "dclutch-local-series-hot-campaign-evidence-v1";

/// PDA seed of the readonly accelerator caller.
///
/// Duplicated from `dclutch-general-accelerator-test-caller-sbf` rather than
/// imported: that crate is a `no_std` SBF program in another workspace, and a
/// host tool taking a build dependency on an SBF entrypoint to read one byte
/// string is a worse coupling than one named constant with a test that pins it.
const GENERAL_ACCELERATOR_CALLER_AUTHORITY_SEED_V1: &[u8] = b"general-accelerator-test-caller";

// These four offsets were restated here as literals -- 18, 0, 4, 5 -- which is
// the third copy of a frame table the producer never emitted, in the one file
// that would have gone stale in silence. They are imported now, from the crate
// this campaign already depends on, and that crate derives them from the
// producer's own `HOT_*_ACCOUNT_V3` coordinates.
use dclutch_execution_strategy_contract::admitted_v3::{
    ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3, ADMITTED_INSTRUCTIONS_ACCOUNT_V3,
    ADMITTED_RUNTIME_ACCOUNTS_START_V3, ADMITTED_TRADING_PROGRAM_ACCOUNT_V3,
};

/// Runtime coordinate carrying the authenticated immutable config.
const RUNTIME_CONFIG_COORDINATE: u16 = 1;
/// Runtime coordinate carrying the Product graph-root body.
const RUNTIME_PRODUCT_COORDINATE: u16 = 2;
/// Runtime coordinate carrying the primary General state.
const RUNTIME_PRIMARY_STATE_COORDINATE: u16 = 5;

/// The candidate the selection cursor already holds.
const FIRST_CANDIDATE_V1: [u8; 32] = [0xb4; 32];
/// The strictly better candidate `Consider` submits against it.
const BEST_CANDIDATE_V1: [u8; 32] = [0xb5; 32];

/// Which family a campaign drives.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FamilyV1 {
    /// The General settlement family and its seven authored actions.
    General,
    /// The Series occurrence family.
    Series,
}

impl FamilyV1 {
    const fn journal_schema(self) -> &'static str {
        match self {
            Self::General => GENERAL_JOURNAL_SCHEMA_V1,
            Self::Series => SERIES_JOURNAL_SCHEMA_V1,
        }
    }

    const fn evidence_schema(self) -> &'static str {
        match self {
            Self::General => GENERAL_EVIDENCE_SCHEMA_V1,
            Self::Series => SERIES_EVIDENCE_SCHEMA_V1,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Series => "Series",
        }
    }
}

/// One durable phase of one action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum FamilyHotPhaseV1 {
    /// The frame and family request exist; no key has been opened.
    Planned,
    /// The exact signed packet is on disk and may only be resent.
    Prepared,
    /// Those exact bytes reached the RPC; recovery is poll-only.
    Submitted,
    /// The transaction is in finalized history with its authenticated ACK.
    Finalized,
}

/// The durable record of one action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FamilyHotJournalV1 {
    schema: String,
    family: String,
    action: String,
    action_index: u16,
    outcome_count: u32,
    phase: FamilyHotPhaseV1,
    caller_program: String,
    accelerator_program: String,
    family_request_base64: String,
    family_request_sha256: String,
    instruction_data_sha256: String,
    accelerator_request_sha256: String,
    input_bank_sha256: String,
    scratch_page_count: u32,
    account_count: usize,
    legacy_packet_bytes: Option<usize>,
    signed_packet_base64: Option<String>,
    signed_packet_sha256: Option<String>,
    expected_signature: Option<String>,
    last_valid_block_height: Option<u64>,
    finalized_slot: Option<u64>,
    compute_units_consumed: Option<u64>,
    return_data_producer: Option<String>,
    return_data_base64: Option<String>,
    ack_disposition: Option<String>,
}

/// The campaign's whole-run evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FamilyHotEvidenceV1 {
    schema: String,
    family: String,
    cluster: String,
    rpc_url: String,
    caller_program: String,
    accelerator_program: String,
    outcome_count: u32,
    action_count: usize,
    actions: Vec<FamilyHotActionEvidenceV1>,
}

/// One finalized action inside the campaign evidence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FamilyHotActionEvidenceV1 {
    action: String,
    signature: String,
    finalized_slot: u64,
    compute_units_consumed: u64,
    legacy_packet_bytes: usize,
    account_count: usize,
    scratch_page_count: u32,
    ack_disposition: String,
    ack_sha256: String,
    journal: String,
}

struct ArgumentsV1 {
    rpc_url: Option<String>,
    accelerator: Pubkey,
    caller: Pubkey,
    payer: Option<PathBuf>,
    account_dir: PathBuf,
    journal_dir: PathBuf,
    evidence: PathBuf,
    outcome_count: u32,
    execute: bool,
}

/// Run one family campaign against a local validator.
pub(crate) fn run(arguments: Vec<String>, family: FamilyV1) -> Result<()> {
    let parsed = parse_arguments(arguments, family)?;
    match family {
        FamilyV1::General => run_general(&parsed),
        FamilyV1::Series => run_series(&parsed),
    }
}

fn parse_arguments(arguments: Vec<String>, family: FamilyV1) -> Result<ArgumentsV1> {
    let mut rpc_url = None;
    let mut accelerator = None;
    let mut caller = None;
    let mut payer = None;
    let mut account_dir = None;
    let mut journal_dir = None;
    let mut evidence = None;
    let mut outcome_count = None;
    let mut execute = false;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        if argument == "--execute" {
            if execute {
                return Err(Error::new("--execute may be supplied only once"));
            }
            execute = true;
            continue;
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            "--accelerator" => &mut accelerator,
            "--caller" => &mut caller,
            "--payer-keypair" => &mut payer,
            "--account-dir" => &mut account_dir,
            "--journal-dir" => &mut journal_dir,
            "--evidence" => &mut evidence,
            "--outcome-count" => &mut outcome_count,
            _ => return Err(Error::new(format!("unknown argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    // The width is not a preference. Six of the seven General actions
    // serialise past Solana's 1,232-byte legacy ceiling at N=258, so a
    // campaign that claimed that width would be recording routes no validator
    // would accept. N=1 is where the packet clause holds, and it is the only
    // width this caller offers until it learns to route through a table.
    let outcome_count = match outcome_count {
        None => 1,
        Some(value) => value
            .parse::<u32>()
            .map_err(|error| Error::new(format!("--outcome-count: {error}")))?,
    };
    if outcome_count == 0 {
        return Err(Error::new("--outcome-count must be positive"));
    }
    // `--rpc-url` and `--payer-keypair` are required only to execute. Without
    // `--execute` this command opens no socket and no key file: it emits the
    // genesis account fixtures the validator must be started with, which is a
    // step that necessarily happens before any validator exists to talk to.
    if execute {
        if rpc_url.is_none() {
            return Err(Error::new("--rpc-url is required with --execute"));
        }
        if payer.is_none() {
            return Err(Error::new("--payer-keypair is required with --execute"));
        }
    }
    let _ = family;
    Ok(ArgumentsV1 {
        rpc_url,
        accelerator: parse_key(
            accelerator.ok_or_else(|| Error::new("--accelerator is required"))?,
            "--accelerator",
        )?,
        caller: parse_key(
            caller.ok_or_else(|| Error::new("--caller is required"))?,
            "--caller",
        )?,
        payer: payer
            .map(|value| absolute_path(value, "--payer-keypair"))
            .transpose()?,
        account_dir: absolute_path(
            account_dir.ok_or_else(|| Error::new("--account-dir is required"))?,
            "--account-dir",
        )?,
        journal_dir: absolute_path(
            journal_dir.ok_or_else(|| Error::new("--journal-dir is required"))?,
            "--journal-dir",
        )?,
        evidence: absolute_path(
            evidence.ok_or_else(|| Error::new("--evidence is required"))?,
            "--evidence",
        )?,
        outcome_count,
        execute,
    })
}

fn parse_key(value: String, label: &str) -> Result<Pubkey> {
    value
        .parse::<Pubkey>()
        .map_err(|error| Error::new(format!("{label}: {error}")))
}

fn absolute_path(value: String, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be an absolute path")));
    }
    Ok(path)
}

/// The Series arm: compile the real wire, then refuse to pretend it has a route.
///
/// This is deliberately not a stub that "succeeds". It builds the exact family
/// request the Series kernel decodes, proves the bytes round-trip through
/// [`SeriesActionRequestV3::decode`], and then reports the precise reason no
/// transaction follows. The refusal names the two missing pieces so the next
/// lane inherits a coordinate rather than a mood.
fn run_series(arguments: &ArgumentsV1) -> Result<()> {
    let mut compiled = Vec::new();
    for action in [
        SeriesActionV3::Prepare,
        SeriesActionV3::Consume,
        SeriesActionV3::Expire,
    ] {
        let request = compile_series_request_v1(action)?;
        compiled.push((action, request));
    }
    fs::create_dir_all(&arguments.journal_dir)?;
    for (action, request) in &compiled {
        let journal = FamilyHotJournalV1 {
            schema: FamilyV1::Series.journal_schema().to_owned(),
            family: FamilyV1::Series.label().to_owned(),
            action: format!("{action:?}"),
            action_index: 0,
            outcome_count: arguments.outcome_count,
            phase: FamilyHotPhaseV1::Planned,
            caller_program: arguments.caller.to_string(),
            accelerator_program: arguments.accelerator.to_string(),
            family_request_base64: BASE64.encode(request),
            family_request_sha256: hex(&Sha256::digest(request)),
            instruction_data_sha256: String::new(),
            accelerator_request_sha256: String::new(),
            input_bank_sha256: String::new(),
            scratch_page_count: 0,
            account_count: 0,
            legacy_packet_bytes: None,
            signed_packet_base64: None,
            signed_packet_sha256: None,
            expected_signature: None,
            last_valid_block_height: None,
            finalized_slot: None,
            compute_units_consumed: None,
            return_data_producer: None,
            return_data_base64: None,
            ack_disposition: None,
        };
        write_json_atomic_v1(
            &arguments
                .journal_dir
                .join(format!("series-{action:?}.json")),
            &journal,
        )?;
    }
    Err(Error::new(format!(
        "REFUSED: compiled {} exact Series family requests (Prepare, Consume, Expire) and wrote \
         their planned journals to {}, but NO Series action has a dispatched Hot route to run \
         through. The blocker is not that the builders lack callers -- it is upstream of that. \
         programs/dclutch-series-shadow-sbf/program-test/README.md states it plainly: \"Until the \
         common authenticated Shadow callback is committed, this crate exposes only the real-ELF \
         loader, selected-build gate, route-order contract, and rollback snapshot support. It does \
         not install a provisional entrypoint or pass artifacts at runtime.\" Series is a \
         ShadowAot family, so that callback is the seam every one of its actions would enter \
         Trading through. Downstream of it, build_series_{{prepare,consume,expire}}_hot_v3 in \
         crates/dclutch-operator/src/series_hot_v3.rs have no caller anywhere, and each would \
         additionally need a SeriesOccurrenceHotStateV3: 38 family-neutral fixed accounts, seven \
         Shadow strategy extras including a deployed accelerator Program and ProgramData, \
         Registry-FINALIZED occurrence and Ticket records with their Merkle siblings, and two \
         CheckedRelease values a release checker only mints after matching a finalized \
         ArtifactRelease against live Loader metadata. Submitting anything here would be a caller \
         that ran against nothing, so it refuses instead.",
        compiled.len(),
        arguments.journal_dir.display()
    )))
}

/// Compile one exact Series family request.
///
/// The shape rules are the kernel's, not this caller's: Prepare, Consume and
/// Expire are all occurrence-bound and carry both an occurrence and a ticket,
/// and Prepare additionally requires a zero expected ticket revision because
/// no ticket state exists yet.
fn compile_series_request_v1(action: SeriesActionV3) -> Result<Vec<u8>> {
    let template = ContentId::new([0x51; 32])
        .map_err(|error| Error::new(format!("Series template identity: {error:?}")))?;
    let occurrence = ContentId::new([0x52; 32])
        .map_err(|error| Error::new(format!("Series occurrence identity: {error:?}")))?;
    let ticket = ContentId::new([0x53; 32])
        .map_err(|error| Error::new(format!("Series ticket identity: {error:?}")))?;
    let expected_ticket_revision = match action {
        SeriesActionV3::Prepare => 0,
        _ => 1,
    };
    let header = encode_series_action_header_v3(
        action,
        template,
        Some(occurrence),
        Some(ticket),
        1,
        expected_ticket_revision,
        0,
    )
    .map_err(|error| Error::new(format!("Series {action:?} header: {error:?}")))?;
    let bytes = header.to_vec();
    if bytes.len() != SERIES_ACTION_HEADER_BYTES_V3 {
        return Err(Error::new("Series header width changed"));
    }
    // Encode then hostile-decode our own candidate. A caller that emits bytes
    // its own family cannot read has proved nothing.
    let decoded = SeriesActionRequestV3::decode(&bytes)
        .map_err(|error| Error::new(format!("Series {action:?} request: {error:?}")))?;
    if decoded.action() != action || decoded.proof_count() != 0 {
        return Err(Error::new(format!(
            "Series {action:?} request did not decode to the action it encoded"
        )));
    }
    Ok(bytes)
}

/// The General arm: drive all seven authored actions against the real ELF.
fn run_general(arguments: &ArgumentsV1) -> Result<()> {
    fs::create_dir_all(&arguments.journal_dir)?;
    fs::create_dir_all(&arguments.account_dir)?;
    let (authority, _) = Pubkey::find_program_address(
        &[GENERAL_ACCELERATOR_CALLER_AUTHORITY_SEED_V1],
        &arguments.caller,
    );
    let mut rpc = match (&arguments.rpc_url, arguments.execute) {
        (Some(url), true) => Some(Rpc::connect(url)?),
        _ => None,
    };
    let payer = match (&arguments.payer, arguments.execute) {
        (Some(path), true) => Some(Keypair::new_from_array(read_keypair_file(path, "payer")?)),
        _ => None,
    };
    // The whole native chain is derived before anything is signed. A refusal
    // here means the campaign never reaches the validator, which is where a
    // broken chain should be found.
    let steps = general_campaign_steps_v1(arguments.outcome_count)?;
    let mut evidence_actions = Vec::with_capacity(steps.len());
    for step in &steps {
        if let Some(finalized) =
            execute_general_action_v1(rpc.as_mut(), arguments, payer.as_ref(), authority, step)?
        {
            evidence_actions.push(finalized);
        }
    }
    if !arguments.execute {
        println!(
            "prepared {} General steps covering all {} authored actions: genesis accounts in {}, \
             planned journals in {}. Start a validator with --account-dir pointed at the account \
             directory, then rerun with --execute --rpc-url URL --payer-keypair PATH.",
            steps.len(),
            GENERAL_ACTIONS_V3.len(),
            arguments.account_dir.display(),
            arguments.journal_dir.display()
        );
        return Ok(());
    }
    let evidence = FamilyHotEvidenceV1 {
        schema: FamilyV1::General.evidence_schema().to_owned(),
        family: FamilyV1::General.label().to_owned(),
        cluster: "local-private-validator".to_owned(),
        rpc_url: arguments.rpc_url.clone().unwrap_or_default(),
        caller_program: arguments.caller.to_string(),
        accelerator_program: arguments.accelerator.to_string(),
        outcome_count: arguments.outcome_count,
        action_count: evidence_actions.len(),
        actions: evidence_actions,
    };
    write_json_atomic_v1(&arguments.evidence, &evidence)?;
    Ok(())
}

/// One action's complete planned -> prepared -> submitted -> finalized ladder.
fn execute_general_action_v1(
    rpc: Option<&mut Rpc>,
    arguments: &ArgumentsV1,
    payer: Option<&Keypair>,
    authority: Pubkey,
    step: &GeneralStepV1,
) -> Result<Option<FamilyHotActionEvidenceV1>> {
    let width = arguments.outcome_count;
    let action = step.action;
    let action_index = step.ordinal;
    let label = format!("General {} at runtime width {width}", step.label);
    let journal_path = arguments.journal_dir.join(format!(
        "general-{action_index:02}-{}.json",
        step.label.replace(' ', "-")
    ));

    let family_request = step.request_bytes.clone();
    let mut instruction_data = general_envelope_v1(&family_request)?;
    instruction_data.extend_from_slice(&family_request);

    let bank = general_input_bank_v1(width, action, arguments.caller, step.coordinates);
    let bank_digest = ContentId::new(Sha256::digest(&bank).into())
        .map_err(|error| Error::new(format!("bank digest: {error:?}")))?;
    let scalar_count = general_hot_scalar_count_v3(action, width)
        .map_err(|error| Error::new(format!("scalar count: {error:?}")))?;
    let request = AcceleratorRequestV2::new(
        RequestTransportV2::ScratchPages,
        content_v1(1)?,
        content_v1(2)?,
        content_v1(3)?,
        content_v1(4)?,
        bank_digest,
        width,
        scalar_count,
        GENERAL_HOT_COMMON_IDENTITIES_V3,
        0,
        &[],
    )
    .map_err(|error| Error::new(format!("accelerator request: {error:?}")))?;
    let mut request_bytes = vec![0_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V2];
    request
        .encode_into(&mut request_bytes)
        .map_err(|error| Error::new(format!("accelerator request bytes: {error:?}")))?;
    let page_count = request.chunk_count();

    // Every account the accelerator reads is a genesis fixture owned by the
    // caller program, at a derived address both phases agree on without
    // exchanging a key. `prepare` writes them; the validator loads them.
    let request_account = campaign_account_key_v1(arguments.caller, step.ordinal, "request", 0);
    if !arguments.execute {
        write_genesis_account_v1(
            &arguments.account_dir,
            request_account,
            arguments.caller,
            &request_bytes,
        )?;
    }

    let mut page_keys = Vec::new();
    for page_index in 0..page_count {
        let key =
            campaign_account_key_v1(arguments.caller, step.ordinal, "scratch-page", page_index);
        if !arguments.execute {
            let page_bytes = general_scratch_page_v1(
                &bank,
                bank_digest,
                width,
                scalar_count,
                page_index,
                arguments.caller,
            )?;
            write_genesis_account_v1(&arguments.account_dir, key, arguments.caller, &page_bytes)?;
        }
        page_keys.push(key);
    }

    let runtime_data = step.runtime.clone();

    let fixed_count = usize::from(
        general_account_profile_fixed_count_v3(action)
            .map_err(|error| Error::new(format!("{label}: account geometry: {error:?}")))?,
    );
    let dummy = Pubkey::new_from_array([0xa4; 32]);
    let mut frame = vec![dummy; ADMITTED_RUNTIME_ACCOUNTS_START_V3 + fixed_count];
    *frame
        .get_mut(ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3)
        .ok_or_else(|| Error::new("authority frame"))? = authority;
    *frame
        .get_mut(ADMITTED_INSTRUCTIONS_ACCOUNT_V3)
        .ok_or_else(|| Error::new("instructions frame"))? = sysvar::instructions::ID;
    *frame
        .get_mut(ADMITTED_TRADING_PROGRAM_ACCOUNT_V3)
        .ok_or_else(|| Error::new("Trading frame"))? = arguments.caller;
    for (coordinate, data) in runtime_data {
        let key = campaign_account_key_v1(
            arguments.caller,
            step.ordinal,
            "runtime",
            u32::from(coordinate),
        );
        if !arguments.execute {
            write_genesis_account_v1(&arguments.account_dir, key, arguments.caller, &data)?;
        }
        let slot = frame
            .get_mut(ADMITTED_RUNTIME_ACCOUNTS_START_V3 + usize::from(coordinate))
            .ok_or_else(|| Error::new(format!("{label}: runtime coordinate {coordinate}")))?;
        *slot = key;
    }
    frame.extend(page_keys.iter().copied());

    let mut metas = Vec::with_capacity(frame.len() + 2);
    metas.push(AccountMeta::new_readonly(request_account, false));
    metas.push(AccountMeta::new_readonly(arguments.accelerator, false));
    metas.extend(
        frame
            .iter()
            .map(|key| AccountMeta::new_readonly(*key, false)),
    );
    let account_count = metas.len();
    let instruction = Instruction {
        program_id: arguments.caller,
        accounts: metas,
        data: instruction_data.clone(),
    };

    let mut journal = FamilyHotJournalV1 {
        schema: FamilyV1::General.journal_schema().to_owned(),
        family: FamilyV1::General.label().to_owned(),
        action: step.label.clone(),
        action_index,
        outcome_count: width,
        phase: FamilyHotPhaseV1::Planned,
        caller_program: arguments.caller.to_string(),
        accelerator_program: arguments.accelerator.to_string(),
        family_request_base64: BASE64.encode(&family_request),
        family_request_sha256: hex(&Sha256::digest(&family_request)),
        instruction_data_sha256: hex(&Sha256::digest(&instruction_data)),
        accelerator_request_sha256: hex(&Sha256::digest(&request_bytes)),
        input_bank_sha256: hex(&Sha256::digest(&bank)),
        scratch_page_count: page_count,
        account_count,
        legacy_packet_bytes: None,
        signed_packet_base64: None,
        signed_packet_sha256: None,
        expected_signature: None,
        last_valid_block_height: None,
        finalized_slot: None,
        compute_units_consumed: None,
        return_data_producer: None,
        return_data_base64: None,
        ack_disposition: None,
    };

    // A journal already on disk is the authority on where this action got to,
    // and it is read BEFORE anything is written: writing `Planned` first would
    // erase the very packet this run is supposed to resend. Reading it before
    // touching a key is what makes the ladder a resume rather than a restart.
    let resumed = read_journal_v1(&journal_path)?;
    if let Some(previous) = &resumed {
        authenticate_resumed_journal_v1(previous, &journal, &label)?;
    } else {
        write_json_atomic_v1(&journal_path, &journal)?;
    }

    let (Some(rpc), Some(payer)) = (rpc, payer) else {
        return Ok(None);
    };
    let packet = match resumed.as_ref().and_then(persisted_packet_v1) {
        Some(packet) => {
            // Reauthenticate the persisted bytes against this run's rebuilt
            // instruction before trusting them. A journal whose packet no
            // longer matches the frame it claims is a refusal, not a resend.
            Rpc::authenticate_signed_legacy_packet(&label, &packet)?;
            journal = resumed.clone().unwrap_or(journal);
            packet
        }
        None => {
            // Prepared: the exact bytes are durable before the first send.
            let packet = rpc.prepare_signed_legacy_packet(&label, &[instruction.clone()], payer)?;
            journal.phase = FamilyHotPhaseV1::Prepared;
            journal.legacy_packet_bytes = Some(
                BASE64
                    .decode(&packet.packet_base64)
                    .map_err(|error| Error::new(format!("{label}: packet base64: {error}")))?
                    .len(),
            );
            journal.signed_packet_base64 = Some(packet.packet_base64.clone());
            journal.signed_packet_sha256 = Some(packet.packet_sha256.clone());
            journal.expected_signature = Some(packet.signature.clone());
            journal.last_valid_block_height = Some(packet.last_valid_block_height);
            write_json_atomic_v1(&journal_path, &journal)?;
            packet
        }
    };

    // A finalized journal is already the answer; polling it again is free and
    // is what makes rerunning a completed campaign a no-op instead of a second
    // set of transactions.
    if journal.phase != FamilyHotPhaseV1::Finalized {
        // Submitted: those exact bytes, once. A resend of the same signature is
        // deduplicated by the cluster, which is what makes a crash here safe.
        rpc.submit_signed_legacy_packet(&label, &packet)?;
        journal.phase = FamilyHotPhaseV1::Submitted;
        write_json_atomic_v1(&journal_path, &journal)?;
    }

    // Finalized: poll only. This never rebuilds a signature.
    let finalized = rpc.confirm_signed_legacy_packet(&label, &packet)?;
    if let Some(error) = finalized.evidence.error.as_ref() {
        return Err(Error::new(format!("{label}: transaction failed: {error}")));
    }
    let return_data = finalized
        .return_data
        .ok_or_else(|| Error::new(format!("{label}: published no top-level return data")))?;
    if return_data.program != arguments.caller {
        return Err(Error::new(format!(
            "{label}: return data producer was {} rather than the invoked caller",
            return_data.program
        )));
    }
    let ack = AcceleratorAckV2::decode(&return_data.data)
        .map_err(|error| Error::new(format!("{label}: accelerator ack: {error:?}")))?;
    let request_digest = ContentId::new(Sha256::digest(&request_bytes).into())
        .map_err(|error| Error::new(format!("{label}: request digest: {error:?}")))?;
    ack.validate_request(request, request_digest)
        .map_err(|error| Error::new(format!("{label}: ack did not bind its request: {error:?}")))?;
    let disposition = match ack.disposition() {
        AcceleratorDispositionV2::Accepted => "accepted",
        AcceleratorDispositionV2::Refused => "refused",
    };

    journal.phase = FamilyHotPhaseV1::Finalized;
    journal.finalized_slot = Some(finalized.evidence.slot);
    journal.compute_units_consumed = finalized.evidence.compute_units_consumed;
    journal.return_data_producer = Some(return_data.program.to_string());
    journal.return_data_base64 = Some(BASE64.encode(&return_data.data));
    journal.ack_disposition = Some(disposition.to_owned());
    write_json_atomic_v1(&journal_path, &journal)?;

    Ok(Some(FamilyHotActionEvidenceV1 {
        action: step.label.clone(),
        signature: packet.signature,
        finalized_slot: finalized.evidence.slot,
        compute_units_consumed: finalized
            .evidence
            .compute_units_consumed
            .unwrap_or_default(),
        legacy_packet_bytes: journal.legacy_packet_bytes.unwrap_or_default(),
        account_count,
        scratch_page_count: page_count,
        ack_disposition: disposition.to_owned(),
        ack_sha256: hex(&Sha256::digest(&return_data.data)),
        journal: journal_path.display().to_string(),
    }))
}

/// The deterministic address of one campaign-owned account.
///
/// These accounts hold data owned by the caller program, and **a host cannot
/// write into an account it does not own** — only the owning program can, and
/// the readonly accelerator caller has no instruction that would. So they are
/// genesis fixtures: `prepare` writes one `<address>.json` per account and the
/// validator is started with `--account ADDRESS FILE` for each, exactly the
/// mechanism `dclutch-successor-validator` uses for its Loader pairs.
///
/// That makes the address a *derived* coordinate rather than a fresh keypair:
/// prepare and execute run in different processes and must agree on it without
/// passing a key. Nothing signs as these accounts, so a domain-separated
/// digest is the whole requirement.
fn campaign_account_key_v1(caller: Pubkey, ordinal: u16, role: &str, index: u32) -> Pubkey {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/local-family-hot-campaign/account/v1");
    hasher.update([0]);
    hasher.update(caller.to_bytes());
    hasher.update([0]);
    hasher.update(ordinal.to_le_bytes());
    hasher.update([0]);
    hasher.update(role.as_bytes());
    hasher.update([0]);
    hasher.update(index.to_le_bytes());
    Pubkey::new_from_array(hasher.finalize().into())
}

/// One genesis account fixture the validator is started with.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct GenesisAccountFileV1 {
    pubkey: String,
    account: GenesisAccountBodyV1,
}

/// The account body in the validator's own `--account` JSON shape.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct GenesisAccountBodyV1 {
    lamports: u64,
    data: [String; 2],
    owner: String,
    executable: bool,
    rent_epoch: u64,
}

/// Write one genesis account fixture for the validator to load.
fn write_genesis_account_v1(
    directory: &Path,
    key: Pubkey,
    owner: Pubkey,
    data: &[u8],
) -> Result<()> {
    // Rent-exemption for the exact width, with the same floor the ProgramTest
    // fixture uses. These accounts are never closed and never pay rent from
    // the campaign, so the only requirement is that the runtime accepts them.
    let lamports = 1_000_000_u64.max(u64::try_from(data.len()).unwrap_or_default() * 10);
    let file = GenesisAccountFileV1 {
        pubkey: key.to_string(),
        account: GenesisAccountBodyV1 {
            lamports,
            data: [BASE64.encode(data), "base64".to_owned()],
            owner: owner.to_string(),
            executable: false,
            rent_epoch: 0,
        },
    };
    write_json_atomic_v1(&directory.join(format!("{key}.json")), &file)
}

/// The exact `DCLTHOT3` envelope preceding one General family request.
fn general_envelope_v1(family_request: &[u8]) -> Result<Vec<u8>> {
    let width = u32::try_from(family_request.len())
        .map_err(|_| Error::new("family request width overflow"))?;
    let envelope = HotExecutionEnvelopeV3::new(width, [0xd1; 32], [0xd2; 32], 1, [0xd3; 32])
        .map_err(|error| Error::new(format!("Hot envelope: {error:?}")))?;
    Ok(envelope.to_bytes().to_vec())
}

/// Encode one exact General controller request at named coordinates.
///
/// The coordinates are not decoration. A settlement step names the revision it
/// expects, the candidate page its row came from, the execution within that
/// page and the manifest ordinal — four independent authenticated facts, and
/// deriving any one from another is refused on chain without a runtime write.
fn general_request_v1(
    action: Action,
    candidate_id: Option<[u8; 32]>,
    expected_revision: u64,
    page_index: u32,
    execution_index: u8,
    manifest_order_index: u8,
) -> Result<Vec<u8>> {
    let request = ControllerRequestV2 {
        action,
        expected_revision,
        candidate_id,
        page_index,
        execution_index,
        manifest_order_index,
        state_bump: 1,
        // Close is the one action that creates a terminal record, and the one
        // that must carry a nonzero bump for it.
        terminal_record_bump: if action == Action::Close { 2 } else { 0 },
    };
    Ok(request
        .to_bytes()
        .map_err(|error| Error::new(format!("General {action:?} request: {error:?}")))?
        .to_vec())
}

fn content_v1(value: u8) -> Result<ContentId> {
    ContentId::new([value; 32]).map_err(|error| Error::new(format!("content identity: {error:?}")))
}

fn general_product_record_v1() -> Vec<u8> {
    vec![0xb1; 64]
}

fn general_product_id_v1() -> [u8; 32] {
    Sha256::digest(general_product_record_v1()).into()
}

fn general_config_v1(width: u32) -> Result<Vec<u8>> {
    Ok(GeneralConfigV3::new(GeneralConfigV3Input {
        capacity_profile_id: [1; 32],
        claim_basis_id: [2; 32],
        program_set_id: [3; 32],
        generation: 9,
        price_scale: u64::from(width),
        collection_slots: 10,
        selection_slots: 10,
        settlement_slots: 10,
        max_orders_per_candidate: 4,
        max_pages_per_candidate: 4,
        continuation_reward_lamports: 1,
        selection_policy_id: [0xb3; 32],
        quote_surplus_beneficiary: [0xc2; 32],
    })
    .map_err(|error| Error::new(format!("General config: {error:?}")))?
    .to_bytes()
    .to_vec())
}

/// The campaign's selection policy.
///
/// The last active criterion must be `MinimizeCandidateId`: the policy is a
/// total order or it is not a decision procedure, and the codec enforces that.
fn general_policy_v1() -> Result<SelectionPolicyV1> {
    let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
    *criteria
        .get_mut(1)
        .ok_or_else(|| Error::new("selection criteria width"))? =
        SelectionCriterion::MinimizeQuoteSurplus;
    *criteria
        .get_mut(2)
        .ok_or_else(|| Error::new("selection criteria width"))? =
        SelectionCriterion::MinimizeCandidateId;
    Ok(SelectionPolicyV1 {
        policy_id: [0xb3; 32],
        criterion_count: 3,
        criteria,
    })
}

/// One runtime-width verified candidate.
fn general_verified_candidate_v1(
    width: u32,
    candidate_id: [u8; 32],
    candidate_coordinate: u32,
    revision: u64,
    filled_lots: u64,
    quote_debit: u64,
    quote_credit: u64,
) -> Result<Vec<u8>> {
    let count = usize::try_from(width).map_err(|_| Error::new("outcome count"))?;
    let mut verified = vec![
        0_u8;
        verified_candidate_len(width)
            .map_err(|error| Error::new(format!("verified width: {error:?}")))?
    ];
    VerifiedCandidateV2::encode_into(
        VerifiedCandidateHeaderV2 {
            outcome_count: width,
            page_count: 1,
            candidate_coordinate,
            revision,
            candidate_id,
            product_id: general_product_id_v1(),
            batch_id: [0xb2; 32],
            filled_lots,
            quote_debit,
            quote_credit,
            price_scale: u64::from(width),
        },
        &vec![7; count],
        &vec![7; count],
        &mut verified,
    )
    .map_err(|error| Error::new(format!("verified candidate: {error:?}")))?;
    Ok(verified)
}

/// Fold one verified candidate into a prior selection cursor.
fn general_selection_body_v1(
    prior: &[u8; RUNTIME_SELECTION_CURSOR_BYTES_V2],
    verified: &[u8],
    submitted_count: u64,
) -> Result<[u8; RUNTIME_SELECTION_CURSOR_BYTES_V2]> {
    let policy = general_policy_v1()?;
    let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut next = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    consider_verified_candidate_v2(
        policy,
        prior,
        verified,
        submitted_count,
        &mut scratch,
        &mut next,
    )
    .map_err(|error| Error::new(format!("consider candidate: {error:?}")))?;
    Ok(next)
}

/// Wrap one cursor body in its General local-state envelope.
fn general_local_state_v1(
    kind: GeneralLocalStateKindV3,
    width: u32,
    body: &[u8],
) -> Result<Vec<u8>> {
    let state_len = general_local_state_len_v3(kind, width)
        .map_err(|error| Error::new(format!("local state width: {error:?}")))?;
    let mut scratch = vec![0_u8; state_len];
    let mut state = vec![0_u8; state_len];
    encode_general_local_state_v3_atomic(
        GeneralLocalStateHeaderV3 {
            kind,
            bump: 1,
            rent_principal: 1,
            beneficiary: [0xc1; 32],
        },
        body,
        &mut scratch,
        &mut state,
    )
    .map_err(|error| Error::new(format!("local state envelope: {error:?}")))?;
    Ok(state)
}

/// The coordinate one action's readonly evidence of a given kind occupies.
fn general_evidence_coordinate_v1(
    action: Action,
    kind: GeneralReadonlyEvidenceKindV3,
) -> Result<u16> {
    let mut index = 0_u16;
    while let Ok(evidence) = general_readonly_evidence_v3(action, index) {
        if evidence.kind == kind {
            return Ok(evidence.coordinate);
        }
        index = index
            .checked_add(1)
            .ok_or_else(|| Error::new("evidence index overflow"))?;
    }
    Err(Error::new(format!(
        "{action:?} declares no readonly evidence of kind {kind:?}"
    )))
}

/// One step of the General campaign.
///
/// A step is not the same thing as an action. `Collect` and `Distribute` each
/// consume three settlement rows, so the seven authored actions occupy eleven
/// steps, and the settlement half of that list is a CHAIN: every step reads the
/// cursor the previous one produced. A caller that submitted these as seven
/// independent transactions would be correctly refused at the second one.
pub(crate) struct GeneralStepV1 {
    /// The authored action this step invokes.
    action: Action,
    /// Position in the campaign, and the journal's filename order.
    ordinal: u16,
    /// Human label distinguishing repeated actions ("Collect row 1").
    label: String,
    /// The exact compiled family request.
    request_bytes: Vec<u8>,
    /// The request's own page / execution / manifest-order coordinates.
    ///
    /// The register bank REPEATS these, and the accelerator authenticates the
    /// two against each other. Building the bank without them left every
    /// settlement row claiming coordinate zero while its request named a real
    /// one, which is a disagreement the ELF is right to refuse.
    coordinates: (u32, u8, u8),
    /// The runtime accounts this step's frame carries.
    runtime: BTreeMap<u16, Vec<u8>>,
}

/// Build the complete eleven-step General campaign.
///
/// The whole plan is derived up front, natively, before a single transaction is
/// signed. That is deliberate: the settlement cursor each on-chain step reads
/// is the output of the previous native transition, so the plan is where the
/// chain's correctness lives. If the native chain refuses, the campaign never
/// reaches the validator at all — which is the right place to find out.
pub(crate) fn general_campaign_steps_v1(width: u32) -> Result<Vec<GeneralStepV1>> {
    let product_id = general_product_id_v1();
    let fixture = terminal_fixture_v1(width, product_id)?;
    let rows = settlement_rows_v1(&fixture)?;
    let config = general_config_v1(width)?;
    let product = general_product_record_v1();
    let mut steps: Vec<GeneralStepV1> = Vec::new();

    let mut push = |action: Action,
                    label: String,
                    request_bytes: Vec<u8>,
                    coordinates: (u32, u8, u8),
                    runtime: BTreeMap<u16, Vec<u8>>|
     -> Result<()> {
        let ordinal = u16::try_from(steps.len()).map_err(|_| Error::new("step ordinal"))?;
        steps.push(GeneralStepV1 {
            action,
            ordinal,
            label,
            request_bytes,
            coordinates,
            runtime,
        });
        Ok(())
    };

    // --- selection tier -----------------------------------------------------
    let vacant = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let incumbent = general_verified_candidate_v1(width, FIRST_CANDIDATE_V1, 1, 1, 7, 7, 0)?;
    let opened = general_selection_body_v1(&vacant, &incumbent, 0)?;
    let better = general_verified_candidate_v1(width, BEST_CANDIDATE_V1, 2, 2, 9, 8, 0)?;

    let mut consider = BTreeMap::new();
    consider.insert(RUNTIME_CONFIG_COORDINATE, config.clone());
    consider.insert(RUNTIME_PRODUCT_COORDINATE, product.clone());
    consider.insert(
        RUNTIME_PRIMARY_STATE_COORDINATE,
        general_local_state_v1(GeneralLocalStateKindV3::Selection, width, &opened)?,
    );
    consider.insert(
        general_evidence_coordinate_v1(
            Action::Consider,
            GeneralReadonlyEvidenceKindV3::SelectionPolicy,
        )?,
        general_policy_v1()?
            .to_bytes()
            .map_err(|error| Error::new(format!("policy bytes: {error:?}")))?
            .to_vec(),
    );
    consider.insert(
        general_evidence_coordinate_v1(
            Action::Consider,
            GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate,
        )?,
        better,
    );
    push(
        Action::Consider,
        "Consider".to_owned(),
        general_request_v1(Action::Consider, Some(BEST_CANDIDATE_V1), 1, 2, 0, 0)?,
        (2, 0, 0),
        consider,
    )?;

    let mut freeze = BTreeMap::new();
    freeze.insert(RUNTIME_CONFIG_COORDINATE, config.clone());
    freeze.insert(RUNTIME_PRODUCT_COORDINATE, product.clone());
    freeze.insert(
        RUNTIME_PRIMARY_STATE_COORDINATE,
        general_local_state_v1(GeneralLocalStateKindV3::Selection, width, &opened)?,
    );
    push(
        Action::Freeze,
        "Freeze".to_owned(),
        general_request_v1(Action::Freeze, None, 1, 0, 0, 0)?,
        (0, 0, 0),
        freeze,
    )?;

    // --- settlement tier ----------------------------------------------------
    // Initialize reads the FROZEN selection of the real verified certificate,
    // the verifier that minted it, and the certificate itself. It reads no
    // primary state: the settlement state is what it is about to create.
    let opened_for_verified = general_selection_body_v1(&vacant, &fixture.verified, 0)?;
    let frozen = frozen_selection_v1(&opened_for_verified)?;
    let mut initialize = BTreeMap::new();
    initialize.insert(RUNTIME_CONFIG_COORDINATE, config.clone());
    initialize.insert(RUNTIME_PRODUCT_COORDINATE, product.clone());
    initialize.insert(
        general_evidence_coordinate_v1(
            Action::InitializeSettlement,
            GeneralReadonlyEvidenceKindV3::FrozenSelection,
        )?,
        frozen.to_vec(),
    );
    initialize.insert(
        general_evidence_coordinate_v1(
            Action::InitializeSettlement,
            GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
        )?,
        fixture.verifier.clone(),
    );
    initialize.insert(
        general_evidence_coordinate_v1(
            Action::InitializeSettlement,
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
        )?,
        fixture.verified.clone(),
    );
    push(
        Action::InitializeSettlement,
        "InitializeSettlement".to_owned(),
        general_request_v1(
            Action::InitializeSettlement,
            Some(fixture.candidate_id),
            0,
            0,
            0,
            0,
        )?,
        (0, 0, 0),
        initialize,
    )?;

    let mut cursor = initialized_cursor_v1(&fixture)?;
    for (action, native) in [
        (Action::Collect, RuntimeSettlementActionV2::Collect),
        (Action::Distribute, RuntimeSettlementActionV2::Distribute),
    ] {
        // Materialize sits between the two row runs: the collected inventory
        // has to exist as a complete set before anything is distributed.
        if action == Action::Distribute {
            let mut materialize = BTreeMap::new();
            materialize.insert(RUNTIME_CONFIG_COORDINATE, config.clone());
            materialize.insert(RUNTIME_PRODUCT_COORDINATE, product.clone());
            materialize.insert(
                RUNTIME_PRIMARY_STATE_COORDINATE,
                general_local_state_v1(GeneralLocalStateKindV3::Settlement, width, &cursor)?,
            );
            materialize.insert(
                general_evidence_coordinate_v1(
                    Action::Materialize,
                    GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
                )?,
                fixture.verified.clone(),
            );
            push(
                Action::Materialize,
                "Materialize".to_owned(),
                general_request_v1(
                    Action::Materialize,
                    Some(fixture.candidate_id),
                    settlement_revision_v1(&cursor)?,
                    0,
                    0,
                    0,
                )?,
                (0, 0, 0),
                materialize,
            )?;
            cursor = settle_native_v1(
                &fixture,
                &cursor,
                RuntimeSettlementActionV2::Materialize,
                None,
                0,
            )?;
        }
        for (index, row) in rows.iter().enumerate() {
            let manifest = fixture
                .manifests
                .get(row.manifest_index)
                .ok_or_else(|| Error::new("manifest absent"))?
                .clone();
            let mut runtime = BTreeMap::new();
            runtime.insert(RUNTIME_CONFIG_COORDINATE, config.clone());
            runtime.insert(RUNTIME_PRODUCT_COORDINATE, product.clone());
            runtime.insert(
                RUNTIME_PRIMARY_STATE_COORDINATE,
                general_local_state_v1(GeneralLocalStateKindV3::Settlement, width, &cursor)?,
            );
            runtime.insert(
                general_evidence_coordinate_v1(
                    action,
                    GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
                )?,
                fixture.verified.clone(),
            );
            runtime.insert(
                general_evidence_coordinate_v1(
                    action,
                    GeneralReadonlyEvidenceKindV3::SettlementManifest,
                )?,
                manifest.clone(),
            );
            push(
                action,
                format!("{action:?} row {index}"),
                general_request_v1(
                    action,
                    Some(fixture.candidate_id),
                    settlement_revision_v1(&cursor)?,
                    row.page_index,
                    row.execution_index,
                    row.manifest_order_index,
                )?,
                (
                    row.page_index,
                    row.execution_index,
                    row.manifest_order_index,
                ),
                runtime,
            )?;
            cursor = settle_native_v1(
                &fixture,
                &cursor,
                native,
                Some(&manifest),
                u32::from(row.manifest_order_index),
            )?;
        }
    }

    let mut close = BTreeMap::new();
    close.insert(RUNTIME_CONFIG_COORDINATE, config);
    close.insert(RUNTIME_PRODUCT_COORDINATE, product);
    close.insert(
        RUNTIME_PRIMARY_STATE_COORDINATE,
        general_local_state_v1(GeneralLocalStateKindV3::Settlement, width, &cursor)?,
    );
    close.insert(
        general_evidence_coordinate_v1(
            Action::Close,
            GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
        )?,
        fixture.verified.clone(),
    );
    push(
        Action::Close,
        "Close".to_owned(),
        general_request_v1(
            Action::Close,
            Some(fixture.candidate_id),
            settlement_revision_v1(&cursor)?,
            0,
            0,
            0,
        )?,
        (0, 0, 0),
        close,
    )?;
    Ok(steps)
}

/// The canonical register bank one action reads.
fn general_input_bank_v1(
    width: u32,
    action: Action,
    caller: Pubkey,
    coordinates: (u32, u8, u8),
) -> Vec<u8> {
    let len = general_hot_candidate_bank_len_v3(action, width).unwrap_or_default();
    let mut bank = vec![0_u8; len];
    let settlement = matches!(
        action,
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close
    );
    let initialize = action == Action::InitializeSettlement;
    let close = action == Action::Close;
    let collect = action == Action::Collect;
    let distribute = action == Action::Distribute;
    for (coordinate, value) in [
        (scalar::OUTCOME_COUNT, u64::from(width)),
        (scalar::GENERATION, 9),
        (scalar::CLAIMS_MARKET_REVISION, 7),
        (scalar::OWNER_POSITION_REVISION, 3),
        (
            scalar::SETTLEMENT_POSITION_REVISION,
            if initialize { 0 } else { 5 },
        ),
        (scalar::OBSERVED_POSITION_LAMPORTS, 200),
        (scalar::OBSERVED_ADMISSION_LAMPORTS, 300),
        (scalar::POSITION_RENT_PRINCIPAL, 101),
        (scalar::ADMISSION_RENT_PRINCIPAL, 202),
        (
            scalar::CUSTODY_EXPECTED_REVISION,
            if initialize { 0 } else { 11 },
        ),
        (scalar::TRANSFER_INDEX, 2),
        (scalar::CUSTODY_REPLAY_RENT_LAMPORTS, 303),
        (scalar::CUSTODY_VAULT_RENT_LAMPORTS, 404),
        (scalar::SETTLEMENT_POSITION_PRESENT, u64::from(settlement)),
        (scalar::POSITION_TABLE_COUNT, u64::from(!close)),
    ] {
        write_bank_scalar_v1(&mut bank, coordinate, value);
    }
    for (coordinate, value) in [
        (identity::PARENT_REQUEST_DIGEST, [1; 32]),
        (identity::RELEASE_SET, [2; 32]),
        (identity::MARKET, [3; 32]),
        (identity::PRODUCT_RECORD_DIGEST, general_product_id_v1()),
        (identity::SEMANTIC_BASIS_ID, [5; 32]),
        (identity::LINKED_BASIS_RECORD_DIGEST, [6; 32]),
        (identity::REALM, [7; 32]),
        (identity::TRADING_PROGRAM, caller.to_bytes()),
        (identity::CUSTODY_SOURCE, [15; 32]),
        (identity::CUSTODY_DESTINATION, [16; 32]),
        (identity::MINT, [17; 32]),
        (identity::TOKEN_PROGRAM, [18; 32]),
        (identity::SETTLEMENT_POSITION_OWNER, [9; 32]),
        (identity::RENT_CREDIT, [10; 32]),
        (identity::RENT_PROGRAM, [11; 32]),
        (identity::GENERAL_ROOT, [12; 32]),
        (identity::PAYER, if initialize { [13; 32] } else { [0; 32] }),
        // The route-dependent identities. These are the ones a first pass
        // silently leaves zero, and zero is not a neutral value here: for the
        // two VAULT_CONTEXT registers it is what the ENABLED route requires,
        // so the polarity is inverted and a missing write reads as "this route
        // is live" on every action where it is not.
        (
            identity::RENT_REFUND,
            if initialize || close {
                [14; 32]
            } else {
                [0; 32]
            },
        ),
        (
            identity::CUSTODY_SOURCE_OWNER,
            if collect { [19; 32] } else { [0; 32] },
        ),
        (
            identity::SOURCE_VAULT_CONTEXT,
            if collect { [0; 32] } else { [20; 32] },
        ),
        (
            identity::CUSTODY_DESTINATION_OWNER,
            if distribute || close {
                [21; 32]
            } else {
                [0; 32]
            },
        ),
        (
            identity::DESTINATION_VAULT_CONTEXT,
            if distribute || close {
                [0; 32]
            } else {
                [22; 32]
            },
        ),
    ] {
        write_bank_identity_v1(action, &mut bank, width, coordinate, value);
    }
    // The request's coordinates, repeated into the bank the accelerator reads.
    // These are authenticated against the request, so a bank that left them
    // zero would disagree with every settlement row it carried.
    let (page_index, execution_index, manifest_order_index) = coordinates;
    write_bank_scalar_v1(&mut bank, scalar::PAGE_INDEX, u64::from(page_index));
    write_bank_scalar_v1(
        &mut bank,
        scalar::EXECUTION_INDEX,
        u64::from(execution_index),
    );
    write_bank_scalar_v1(
        &mut bank,
        scalar::MANIFEST_ORDER_INDEX,
        u64::from(manifest_order_index),
    );
    bank
}

fn write_bank_scalar_v1(bank: &mut [u8], coordinate: u32, value: u64) {
    let Ok(index) = usize::try_from(coordinate) else {
        return;
    };
    let Some(start) = index.checked_mul(8) else {
        return;
    };
    if let Some(slot) = bank.get_mut(start..start.saturating_add(8)) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
}

fn write_bank_identity_v1(
    action: Action,
    bank: &mut [u8],
    width: u32,
    coordinate: u32,
    value: [u8; 32],
) {
    let Ok(scalar_count) = general_hot_scalar_count_v3(action, width) else {
        return;
    };
    let Ok(scalars) = usize::try_from(scalar_count) else {
        return;
    };
    let Ok(index) = usize::try_from(coordinate) else {
        return;
    };
    let Some(start) = scalars
        .checked_mul(8)
        .and_then(|base| index.checked_mul(32).and_then(|off| base.checked_add(off)))
    else {
        return;
    };
    if let Some(slot) = bank.get_mut(start..start.saturating_add(32)) {
        slot.copy_from_slice(&value);
    }
}

/// One authenticated input scratch page carrying its slice of the bank.
fn general_scratch_page_v1(
    bank: &[u8],
    bank_digest: ContentId,
    width: u32,
    scalar_count: u32,
    page_index: u32,
    caller: Pubkey,
) -> Result<Vec<u8>> {
    let page_request = AcceleratorRequestV2::new(
        RequestTransportV2::ScratchPages,
        content_v1(1)?,
        content_v1(2)?,
        content_v1(3)?,
        content_v1(4)?,
        bank_digest,
        width,
        scalar_count,
        GENERAL_HOT_COMMON_IDENTITIES_V3,
        page_index,
        &[],
    )
    .map_err(|error| Error::new(format!("page request: {error:?}")))?;
    let start = usize::try_from(page_request.chunk_offset())
        .map_err(|_| Error::new("page offset overflow"))?;
    let end = start
        .checked_add(ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2)
        .unwrap_or(bank.len())
        .min(bank.len());
    let payload = bank
        .get(start..end)
        .ok_or_else(|| Error::new("page payload out of range"))?;
    let page = AuthenticatedScratchPageV2::new(
        ScratchPageKindV2::Input,
        ContentId::new(caller.to_bytes())
            .map_err(|error| Error::new(format!("caller identity: {error:?}")))?,
        content_v1(1)?,
        content_v1(4)?,
        bank_digest,
        width,
        scalar_count,
        GENERAL_HOT_COMMON_IDENTITIES_V3,
        page_index,
        payload,
    )
    .map_err(|error| Error::new(format!("scratch page: {error:?}")))?;
    let mut bytes = vec![0_u8; SCRATCH_PAGE_HEADER_BYTES_V2 + page.payload().len()];
    page.encode_into(&mut bytes)
        .map_err(|error| Error::new(format!("scratch page bytes: {error:?}")))?;
    Ok(bytes)
}

/// What a founded Market must carry before General is selectable on it.
///
/// This is the Market-selection hook. It authors nothing and reads no chain;
/// it states, in one place a caller can consult, the facts a Market has to
/// already hold before `process_hot_execution_v3` will route a General action
/// to it. Every one of these is checked on chain today — this function exists
/// so the requirement has a name in the caller rather than living only as
/// seven scattered refusals.
pub(crate) fn general_market_selection_requirements_v1() -> Vec<&'static str> {
    vec![
        "the Market's capability manifest carries a CapabilityEntryV1 whose kind_id, release_id \
         and config_id equal the CapabilityExecutionSelectionV1 written into the Trading \
         capability root header",
        "the selection's capability_release is a CapabilityProgramSetV2 whose selector_offset is \
         10, which is CONTROLLER_REQUEST_ACTION_OFFSET_V2 -- the General action byte IS the \
         family discriminant",
        "the set carries one entry per authored action, so a release admitting fewer than the \
         seven GENERAL_ACTIONS_V3 cannot route the whole family",
        "the selection's executor_role is Trading and the capability root PDA derives from the \
         selection seeds under the Market-selected Trading program",
        "the EffectProgram named by the descriptor decodes as effect-kernel V4, which is what \
         encode_general_effect_program_v4_atomic emits and what the Hot executor accepts",
        "the strategy disposition is AdmittedAot, because General's sole dynamic span is \
         AccountProfile-owned and no RequestProfile writes its selector -- the real accelerator \
         ELF must therefore be deployed and admitted",
    ]
}

/// Read one durable journal, if this action has been attempted before.
fn read_journal_v1(path: &Path) -> Result<Option<FamilyHotJournalV1>> {
    match fs::read(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Error::new(format!("{}: {error}", path.display()))),
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
    }
}

/// Refuse a journal that describes a different action than the one being run.
///
/// Resuming is only safe when the thing on disk is the same intent. The
/// request digest is the strongest cheap statement of that: it covers the
/// action, the candidate, the revision and every coordinate the family request
/// carries. A journal-directory reuse across widths or callers is a mistake
/// worth catching before a key is opened, not after a second signature exists.
fn authenticate_resumed_journal_v1(
    previous: &FamilyHotJournalV1,
    current: &FamilyHotJournalV1,
    label: &str,
) -> Result<()> {
    if previous.family != current.family
        || previous.action != current.action
        || previous.outcome_count != current.outcome_count
        || previous.caller_program != current.caller_program
        || previous.accelerator_program != current.accelerator_program
        || previous.family_request_sha256 != current.family_request_sha256
        || previous.instruction_data_sha256 != current.instruction_data_sha256
    {
        return Err(Error::new(format!(
            "{label}: the journal already in this directory describes a different action; \
             prepare a new campaign under a new journal directory rather than resuming across \
             intents"
        )));
    }
    Ok(())
}

/// The durable packet a resumed journal carries, if it reached `Prepared`.
fn persisted_packet_v1(journal: &FamilyHotJournalV1) -> Option<SignedVersionedPacketV1> {
    Some(SignedVersionedPacketV1 {
        signature: journal.expected_signature.clone()?,
        packet_base64: journal.signed_packet_base64.clone()?,
        packet_sha256: journal.signed_packet_sha256.clone()?,
        last_valid_block_height: journal.last_valid_block_height?,
    })
}

fn write_json_atomic_v1<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{} has no parent directory", path.display())))?;
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.partial");
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(&temporary, &bytes)?;
    // Rename is what makes a reader never observe a half-written phase.
    fs::rename(&temporary, path)?;
    Ok(())
}

/// Usage text for both family commands.
pub(crate) fn usage() -> &'static str {
    "  local-private-validator-general-hot-campaign-v1 --rpc-url URL --accelerator PUBKEY \\\n\
     \x20   --caller PUBKEY --payer-keypair PATH --journal-dir DIR --evidence PATH \\\n\
     \x20   [--outcome-count N] [--execute]\n\
     \x20 local-private-validator-series-hot-campaign-v1 (same flags; refuses with the exact \
     reason Series has no dispatched route)\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seed must match the deployed caller, and a rename is a redeploy.
    #[test]
    fn the_caller_authority_seed_is_the_one_the_program_derives() {
        assert_eq!(
            GENERAL_ACCELERATOR_CALLER_AUTHORITY_SEED_V1,
            b"general-accelerator-test-caller"
        );
    }

    /// All seven authored actions compile to the exact 64-byte V2 wire.
    #[test]
    fn every_authored_general_action_compiles_to_the_exact_request_width() {
        for action in GENERAL_ACTIONS_V3 {
            let bytes = general_request_v1(
                action,
                match action {
                    Action::Freeze => None,
                    _ => Some(FIRST_CANDIDATE_V1),
                },
                1,
                0,
                0,
                0,
            )
            .expect("request");
            assert_eq!(bytes.len(), 64, "{action:?}");
            let decoded = ControllerRequestV2::decode(&bytes).expect("decode");
            assert_eq!(decoded.action, action);
        }
    }

    /// The Series wire round-trips through the kernel's own decoder.
    #[test]
    fn every_series_occurrence_action_round_trips_through_the_kernel_decoder() {
        for action in [
            SeriesActionV3::Prepare,
            SeriesActionV3::Consume,
            SeriesActionV3::Expire,
        ] {
            let bytes = compile_series_request_v1(action).expect("request");
            assert_eq!(bytes.len(), SERIES_ACTION_HEADER_BYTES_V3, "{action:?}");
            let decoded = SeriesActionRequestV3::decode(&bytes).expect("decode");
            assert_eq!(decoded.action(), action);
        }
    }

    /// The bank is exactly the width the accelerator recomputes for itself.
    #[test]
    fn the_input_bank_is_the_canonical_width_for_its_outcome_count() {
        let caller = Pubkey::new_from_array([0xa2; 32]);
        for width in [1_u32, 4, 258] {
            for action in GENERAL_ACTIONS_V3 {
                // Per-ACTION now: an action with no per-outcome tail has a bank
                // that does not grow with the Product width.
                let expected =
                    general_hot_candidate_bank_len_v3(action, width).expect("bank width");
                assert_eq!(
                    general_input_bank_v1(width, action, caller, (0, 0, 0)).len(),
                    expected,
                    "{action:?} at width {width}"
                );
            }
        }
    }

    fn journal_for(action: Action, request_digest: &str) -> FamilyHotJournalV1 {
        FamilyHotJournalV1 {
            schema: GENERAL_JOURNAL_SCHEMA_V1.to_owned(),
            family: "General".to_owned(),
            action: format!("{action:?}"),
            action_index: 0,
            outcome_count: 1,
            phase: FamilyHotPhaseV1::Planned,
            caller_program: "caller".to_owned(),
            accelerator_program: "accelerator".to_owned(),
            family_request_base64: String::new(),
            family_request_sha256: request_digest.to_owned(),
            instruction_data_sha256: request_digest.to_owned(),
            accelerator_request_sha256: String::new(),
            input_bank_sha256: String::new(),
            scratch_page_count: 0,
            account_count: 0,
            legacy_packet_bytes: None,
            signed_packet_base64: None,
            signed_packet_sha256: None,
            expected_signature: None,
            last_valid_block_height: None,
            finalized_slot: None,
            compute_units_consumed: None,
            return_data_producer: None,
            return_data_base64: None,
            ack_disposition: None,
        }
    }

    /// Resuming across two different intents is a refusal, not a resend.
    #[test]
    fn a_journal_describing_another_action_refuses_to_be_resumed() {
        let mine = journal_for(Action::Freeze, "aa");
        assert!(authenticate_resumed_journal_v1(&mine, &mine, "test").is_ok());
        let other_action = journal_for(Action::Consider, "aa");
        assert!(authenticate_resumed_journal_v1(&other_action, &mine, "test").is_err());
        let other_request = journal_for(Action::Freeze, "bb");
        assert!(authenticate_resumed_journal_v1(&other_request, &mine, "test").is_err());
        let mut other_width = journal_for(Action::Freeze, "aa");
        other_width.outcome_count = 258;
        assert!(authenticate_resumed_journal_v1(&other_width, &mine, "test").is_err());
    }

    /// A journal below `Prepared` carries no packet to resend.
    #[test]
    fn only_a_prepared_journal_offers_a_packet_to_resume() {
        let mut journal = journal_for(Action::Freeze, "aa");
        assert!(persisted_packet_v1(&journal).is_none());
        journal.expected_signature = Some("signature".to_owned());
        journal.signed_packet_base64 = Some("packet".to_owned());
        journal.signed_packet_sha256 = Some("digest".to_owned());
        // Still none: the last-valid height is what bounds the resend, and a
        // packet without it cannot be told from an expired one.
        assert!(persisted_packet_v1(&journal).is_none());
        journal.last_valid_block_height = Some(7);
        let packet = persisted_packet_v1(&journal).expect("packet");
        assert_eq!(packet.signature, "signature");
        assert_eq!(packet.last_valid_block_height, 7);
    }

    /// The campaign plan covers every authored action, and chains settlement.
    #[test]
    fn the_step_plan_covers_all_seven_actions_and_advances_the_settlement_chain() {
        let steps = general_campaign_steps_v1(1).expect("plan");
        // Seven actions, eleven steps: Collect and Distribute each consume the
        // three manifest rows.
        assert_eq!(steps.len(), 11);
        for action in GENERAL_ACTIONS_V3 {
            assert!(
                steps.iter().any(|step| step.action == action),
                "{action:?} is not in the plan"
            );
        }
        assert_eq!(
            steps
                .iter()
                .filter(|step| step.action == Action::Collect)
                .count(),
            3
        );
        assert_eq!(
            steps
                .iter()
                .filter(|step| step.action == Action::Distribute)
                .count(),
            3
        );
        // Materialize must sit between the last Collect and the first
        // Distribute: the complete set has to exist before anything is paid
        // out, and a plan that ordered them otherwise would refuse on chain.
        let last_collect = steps
            .iter()
            .rposition(|step| step.action == Action::Collect)
            .expect("collect");
        let materialize = steps
            .iter()
            .position(|step| step.action == Action::Materialize)
            .expect("materialize");
        let first_distribute = steps
            .iter()
            .position(|step| step.action == Action::Distribute)
            .expect("distribute");
        assert!(last_collect < materialize && materialize < first_distribute);
        // Every step has a distinct ordinal, which is what keeps their genesis
        // accounts and journals from colliding.
        let ordinals: std::collections::BTreeSet<u16> =
            steps.iter().map(|step| step.ordinal).collect();
        assert_eq!(ordinals.len(), steps.len());
    }

    /// The selection-hook list is a checklist, not a mood.
    #[test]
    fn the_market_selection_hook_names_every_required_fact() {
        let requirements = general_market_selection_requirements_v1();
        assert_eq!(requirements.len(), 6);
        assert!(requirements.iter().all(|line| !line.is_empty()));
    }
}

/// Open a selection cursor over one verified candidate, for fixture tests.
#[cfg(test)]
pub(crate) fn selection_body_for_tests_v1(
    verified: &[u8],
) -> Result<[u8; RUNTIME_SELECTION_CURSOR_BYTES_V2]> {
    let vacant = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    general_selection_body_v1(&vacant, verified, 0)
}
