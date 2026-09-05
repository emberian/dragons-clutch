//! State-selected Fractional retirement planning for an external wallet.
//!
//! The route carries only public addresses. The first finalized read lets the
//! authoritative Fractional operator discover whether the vacant/live cursor
//! selects Begin, one exact coordinate, or Finish. The second read reacquires
//! the complete graph, including any state-derived coordinate accounts, before
//! the same operator compiles one unsigned v0 transaction.

use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    os::unix::fs::OpenOptionsExt as _,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_claims::fractional::FractionalRetirementActionV3;
use dclutch_operator::fractional::{
    FractionalRetirementCoordinateSnapshotV3, FractionalRetirementDeploymentV3,
    FractionalRetirementNextPlanV3, FractionalRetirementRecordV3, FractionalRetirementSnapshotV3,
    discover_fractional_retirement_next_v3, plan_fractional_retirement_next_v3,
};
use dclutch_versioned_message_operator::ObservedAccount;
use serde::{Deserialize, Serialize};
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk::{signature::Signature, transaction::VersionedTransaction};

use crate::{DEFAULT_RPC_URL_V1, Error, RPC_URL_ENV_V1, Result, rpc};

const ROUTE_FORMAT_V1: &str = "dclutch/fractional-retirement-next-route/v1";
const PLAN_FORMAT_V1: &str = "dclutch/fractional-retirement-next-plan/v1";
const MAX_DOCUMENT_BYTES_V1: u64 = 65_536;

/// Run the state-selected Fractional retirement command.
pub fn run(arguments: Vec<String>) -> Result<()> {
    if arguments
        .first()
        .is_some_and(|value| matches!(value.as_str(), "help" | "-h" | "--help"))
    {
        if arguments.len() != 1 {
            return Err(Error::new(
                "Fractional retirement help takes no other arguments",
            ));
        }
        print!("{}", usage());
        return Ok(());
    }
    let arguments = PlanArgumentsV1::parse(arguments)?;
    let route = read_route_v1(&arguments.route)?;

    let discovery_accounts = rpc::fetch_observed_accounts_allow_absent_v1(
        &arguments.rpc,
        &route.common_addresses(),
        &route.allowed_absent(),
        route.minimum_finalized_slot,
    )?;
    let discovery_snapshot = snapshot_v1(route.payer, discovery_accounts, false)?;
    let discovery = discover_fractional_retirement_next_v3(&discovery_snapshot).map_err(lift)?;

    let mut full_addresses = route.common_addresses();
    if let (Some(position), Some(admission), Some(shard_mint)) = (
        discovery.position,
        discovery.admission,
        discovery.shard_mint,
    ) {
        full_addresses.extend([position, admission, shard_mint]);
    } else if discovery.position.is_some()
        || discovery.admission.is_some()
        || discovery.shard_mint.is_some()
    {
        return Err(Error::new(
            "Fractional retirement discovery returned a partial coordinate graph",
        ));
    }
    if let Some(lookup_table) = route.lookup_table {
        full_addresses.push(lookup_table);
    }
    let mut full_accounts = rpc::fetch_observed_accounts_allow_absent_v1(
        &arguments.rpc,
        &full_addresses,
        &route.allowed_absent(),
        discovery.observation.slot,
    )?;
    let lookup_table = if route.lookup_table.is_some() {
        Some(
            full_accounts
                .pop()
                .ok_or_else(|| Error::new("Fractional retirement lookup-table read was empty"))?,
        )
    } else {
        None
    };
    let full_snapshot = snapshot_v1(route.payer, full_accounts, discovery.coordinate.is_some())?;
    let observed_slot = full_snapshot.core_market.observation.slot;
    let recent_blockhash = rpc::fetch_latest_finalized_blockhash_v1(&arguments.rpc, observed_slot)?;
    let lookup_tables = lookup_table.as_slice();
    let plan = plan_fractional_retirement_next_v3(&full_snapshot, recent_blockhash, lookup_tables)
        .map_err(lift)?;
    let document = document_v1(&route, &full_snapshot, &plan)?;
    write_new_output_v1(&arguments.output, &encode_document_v1(&document)?)?;

    println!(
        "Wrote unsigned Fractional retirement `{}` for root {} to {}.",
        document.action,
        document.root,
        arguments.output.display()
    );
    println!(
        "Authenticated one finalized snapshot at slot {}; the cursor selected this act, not the route.",
        document.observed_slot
    );
    println!(
        "No key was read and nothing was simulated or submitted. {}",
        document.remedy
    );
    Ok(())
}

fn lift(error: dclutch_operator::fractional::Error) -> Error {
    Error::new(format!("Fractional retirement operator refused: {error:?}"))
}

#[derive(Clone)]
struct PlanArgumentsV1 {
    rpc: String,
    route: PathBuf,
    output: PathBuf,
}

impl core::fmt::Debug for PlanArgumentsV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PlanArgumentsV1")
            .field("rpc", &rpc::origin(&self.rpc))
            .field("route", &self.route)
            .field("output", &self.output)
            .finish()
    }
}

impl PlanArgumentsV1 {
    fn parse(arguments: Vec<String>) -> Result<Self> {
        let mut rpc = None;
        let mut route = None;
        let mut output = None;
        let mut iterator = arguments.into_iter();
        while let Some(argument) = iterator.next() {
            let destination = match argument.as_str() {
                "--rpc" => &mut rpc,
                "--route" => &mut route,
                "--output" => &mut output,
                other => {
                    return Err(Error::new(format!(
                        "unknown Fractional retirement argument `{other}`. Run `dclutch fractional-retirement-next --help`."
                    )));
                }
            };
            if destination.is_some() {
                return Err(Error::new(format!(
                    "Fractional retirement argument `{argument}` was repeated"
                )));
            }
            *destination = Some(iterator.next().ok_or_else(|| {
                Error::new(format!(
                    "Fractional retirement argument `{argument}` needs a value"
                ))
            })?);
        }
        let rpc = rpc.unwrap_or_else(|| {
            std::env::var(RPC_URL_ENV_V1).unwrap_or_else(|_| DEFAULT_RPC_URL_V1.to_owned())
        });
        Ok(Self {
            rpc,
            route: absolute_path_v1(route, "--route")?,
            output: absolute_path_v1(output, "--output")?,
        })
    }
}

fn absolute_path_v1(value: Option<String>, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value.ok_or_else(|| Error::new(format!("missing {label}")))?);
    if !path.is_absolute() {
        return Err(Error::new(format!("{label} must be an absolute path")));
    }
    Ok(path)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RouteWireV1 {
    format: String,
    minimum_finalized_slot: String,
    payer: String,
    root: String,
    #[serde(default)]
    lookup_table: Option<String>,
    core_market: String,
    claims_market: String,
    activation_cache: String,
    registry_program: String,
    core_program: String,
    core_programdata: String,
    claims_program: String,
    claims_programdata: String,
    trading_program: String,
    trading_programdata: String,
    rent_program: String,
    rent_programdata: String,
    rent_credit: String,
    cursor: String,
    terms_raw: String,
    terms_staging: String,
    token_behavior_raw: String,
    token_behavior_staging: String,
    rent_sysvar: String,
    system_program: String,
    token_program: String,
}

#[derive(Clone, Debug)]
struct RouteV1 {
    minimum_finalized_slot: u64,
    payer: Pubkey,
    core_market: Pubkey,
    claims_market: Pubkey,
    activation_cache: Pubkey,
    registry_program: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
    claims_program: Pubkey,
    claims_programdata: Pubkey,
    trading_program: Pubkey,
    trading_programdata: Pubkey,
    rent_program: Pubkey,
    rent_programdata: Pubkey,
    root: Pubkey,
    rent_credit: Pubkey,
    cursor: Pubkey,
    terms_raw: Pubkey,
    terms_staging: Pubkey,
    token_behavior_raw: Pubkey,
    token_behavior_staging: Pubkey,
    rent_sysvar: Pubkey,
    system_program: Pubkey,
    token_program: Pubkey,
    lookup_table: Option<Pubkey>,
}

impl RouteV1 {
    fn common_addresses(&self) -> Vec<Pubkey> {
        vec![
            self.core_market,
            self.claims_market,
            self.activation_cache,
            self.registry_program,
            self.core_program,
            self.core_programdata,
            self.claims_program,
            self.claims_programdata,
            self.trading_program,
            self.trading_programdata,
            self.rent_program,
            self.rent_programdata,
            self.root,
            self.rent_credit,
            self.cursor,
            self.terms_raw,
            self.terms_staging,
            self.token_behavior_raw,
            self.token_behavior_staging,
            self.rent_sysvar,
            self.system_program,
            self.token_program,
        ]
    }

    fn allowed_absent(&self) -> [Pubkey; 3] {
        [self.cursor, self.terms_staging, self.token_behavior_staging]
    }
}

fn read_route_v1(path: &Path) -> Result<RouteV1> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        Error::new(format!(
            "cannot inspect Fractional retirement route: {error}"
        ))
    })?;
    if !metadata.file_type().is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_DOCUMENT_BYTES_V1
    {
        return Err(Error::new(
            "--route must be one regular non-symlink file no larger than 64 KiB",
        ));
    }
    let canonical = path.canonicalize().map_err(|error| {
        Error::new(format!(
            "cannot canonicalize Fractional retirement route: {error}"
        ))
    })?;
    if canonical != path {
        return Err(Error::new(
            "--route must be an absolute canonical non-symlink path",
        ));
    }
    let bytes = fs::read(path)
        .map_err(|error| Error::new(format!("cannot read Fractional retirement route: {error}")))?;
    let wire: RouteWireV1 = serde_json::from_slice(&bytes)
        .map_err(|error| Error::new(format!("Fractional retirement route JSON: {error}")))?;
    if wire.format != ROUTE_FORMAT_V1 {
        return Err(Error::new(
            "Fractional retirement route format is not exact V1",
        ));
    }
    let minimum_finalized_slot =
        canonical_u64_v1(&wire.minimum_finalized_slot, "minimumFinalizedSlot")?;
    let route = RouteV1 {
        minimum_finalized_slot,
        payer: canonical_pubkey_v1(&wire.payer, "payer")?,
        core_market: canonical_pubkey_v1(&wire.core_market, "coreMarket")?,
        claims_market: canonical_pubkey_v1(&wire.claims_market, "claimsMarket")?,
        activation_cache: canonical_pubkey_v1(&wire.activation_cache, "activationCache")?,
        registry_program: canonical_pubkey_v1(&wire.registry_program, "registryProgram")?,
        core_program: canonical_pubkey_v1(&wire.core_program, "coreProgram")?,
        core_programdata: canonical_pubkey_v1(&wire.core_programdata, "coreProgramdata")?,
        claims_program: canonical_pubkey_v1(&wire.claims_program, "claimsProgram")?,
        claims_programdata: canonical_pubkey_v1(&wire.claims_programdata, "claimsProgramdata")?,
        trading_program: canonical_pubkey_v1(&wire.trading_program, "tradingProgram")?,
        trading_programdata: canonical_pubkey_v1(&wire.trading_programdata, "tradingProgramdata")?,
        rent_program: canonical_pubkey_v1(&wire.rent_program, "rentProgram")?,
        rent_programdata: canonical_pubkey_v1(&wire.rent_programdata, "rentProgramdata")?,
        root: canonical_pubkey_v1(&wire.root, "root")?,
        rent_credit: canonical_pubkey_v1(&wire.rent_credit, "rentCredit")?,
        cursor: canonical_pubkey_v1(&wire.cursor, "cursor")?,
        terms_raw: canonical_pubkey_v1(&wire.terms_raw, "termsRaw")?,
        terms_staging: canonical_pubkey_v1(&wire.terms_staging, "termsStaging")?,
        token_behavior_raw: canonical_pubkey_v1(&wire.token_behavior_raw, "tokenBehaviorRaw")?,
        token_behavior_staging: canonical_pubkey_v1(
            &wire.token_behavior_staging,
            "tokenBehaviorStaging",
        )?,
        rent_sysvar: canonical_pubkey_v1(&wire.rent_sysvar, "rentSysvar")?,
        system_program: canonical_pubkey_v1(&wire.system_program, "systemProgram")?,
        token_program: canonical_pubkey_v1(&wire.token_program, "tokenProgram")?,
        lookup_table: wire
            .lookup_table
            .as_deref()
            .map(|value| canonical_pubkey_v1(value, "lookupTable"))
            .transpose()?,
    };
    let common = route.common_addresses();
    if common.iter().any(|key| *key == Pubkey::default())
        || common
            .iter()
            .enumerate()
            .any(|(index, key)| common.iter().skip(index + 1).any(|other| other == key))
        || route
            .lookup_table
            .is_some_and(|table| common.contains(&table))
    {
        return Err(Error::new(
            "Fractional retirement route addresses must be nonzero and pairwise distinct",
        ));
    }
    Ok(route)
}

fn canonical_u64_v1(value: &str, field: &str) -> Result<u64> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| Error::new(format!("{field} is not a canonical decimal u64")))?;
    if parsed.to_string() != value {
        return Err(Error::new(format!(
            "{field} is not a canonical decimal u64"
        )));
    }
    Ok(parsed)
}

fn canonical_pubkey_v1(value: &str, field: &str) -> Result<Pubkey> {
    let parsed = value
        .parse::<Pubkey>()
        .map_err(|_| Error::new(format!("{field} is not a canonical base58 Solana address")))?;
    if parsed.to_string() != value || parsed == Pubkey::default() {
        return Err(Error::new(format!(
            "{field} is not a canonical nonzero base58 Solana address"
        )));
    }
    Ok(parsed)
}

fn next_account_v1(
    accounts: &mut std::vec::IntoIter<ObservedAccount>,
    label: &str,
) -> Result<ObservedAccount> {
    accounts
        .next()
        .ok_or_else(|| Error::new(format!("Fractional retirement snapshot omitted {label}")))
}

fn snapshot_v1(
    payer: Pubkey,
    accounts: Vec<ObservedAccount>,
    has_coordinate: bool,
) -> Result<FractionalRetirementSnapshotV3> {
    let mut accounts = accounts.into_iter();
    let snapshot = FractionalRetirementSnapshotV3 {
        payer,
        core_market: next_account_v1(&mut accounts, "Core Market")?,
        claims_market: next_account_v1(&mut accounts, "Claims aggregate")?,
        activation_cache: next_account_v1(&mut accounts, "activation cache")?,
        registry_program: next_account_v1(&mut accounts, "Registry program")?,
        core: FractionalRetirementDeploymentV3 {
            program: next_account_v1(&mut accounts, "Core program")?,
            programdata: next_account_v1(&mut accounts, "Core ProgramData")?,
        },
        claims: FractionalRetirementDeploymentV3 {
            program: next_account_v1(&mut accounts, "Claims program")?,
            programdata: next_account_v1(&mut accounts, "Claims ProgramData")?,
        },
        trading: FractionalRetirementDeploymentV3 {
            program: next_account_v1(&mut accounts, "Trading program")?,
            programdata: next_account_v1(&mut accounts, "Trading ProgramData")?,
        },
        rent: FractionalRetirementDeploymentV3 {
            program: next_account_v1(&mut accounts, "Rent program")?,
            programdata: next_account_v1(&mut accounts, "Rent ProgramData")?,
        },
        root: next_account_v1(&mut accounts, "Fractional root")?,
        rent_credit: next_account_v1(&mut accounts, "RentCredit")?,
        cursor: next_account_v1(&mut accounts, "retirement cursor")?,
        terms: FractionalRetirementRecordV3 {
            raw: next_account_v1(&mut accounts, "terms raw record")?,
            staging: next_account_v1(&mut accounts, "terms staging cursor")?,
        },
        token_behavior: FractionalRetirementRecordV3 {
            raw: next_account_v1(&mut accounts, "TokenBehavior raw record")?,
            staging: next_account_v1(&mut accounts, "TokenBehavior staging cursor")?,
        },
        rent_sysvar: next_account_v1(&mut accounts, "Rent sysvar")?,
        system_program: next_account_v1(&mut accounts, "System program")?,
        token_program: next_account_v1(&mut accounts, "Token-2022 program")?,
        coordinate: if has_coordinate {
            Some(FractionalRetirementCoordinateSnapshotV3 {
                position: next_account_v1(&mut accounts, "coordinate Position")?,
                admission: next_account_v1(&mut accounts, "coordinate admission")?,
                shard_mint: next_account_v1(&mut accounts, "coordinate shard Mint")?,
            })
        } else {
            None
        },
    };
    if accounts.next().is_some() {
        return Err(Error::new(
            "Fractional retirement snapshot carried surplus accounts",
        ));
    }
    Ok(snapshot)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanDocumentV1 {
    format: &'static str,
    action: &'static str,
    coordinate: Option<u32>,
    transaction_base64: String,
    observed_slot: String,
    payer: String,
    required_signers: Vec<String>,
    request_base64: String,
    request_sha256: String,
    release_set: String,
    market: String,
    root: String,
    rent_credit: String,
    artifacts: ArtifactPinsV1,
    programs: ProgramPinsV1,
    root_revision_anchor: String,
    expected_revision: String,
    representation_width: u32,
    instruction: InstructionFrameV1,
    wire: WireReportV1,
    lookup_table: Option<String>,
    consequence: &'static str,
    remedy: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactPinsV1 {
    terms: String,
    token_behavior: String,
    exposure: String,
    terms_raw: String,
    token_behavior_raw: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramPinsV1 {
    registry: String,
    core: String,
    core_programdata: String,
    claims: String,
    claims_programdata: String,
    trading: String,
    trading_programdata: String,
    rent: String,
    rent_programdata: String,
    token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionFrameV1 {
    program: String,
    accounts: usize,
    data_bytes: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WireReportV1 {
    bytes: usize,
    loaded_addresses: usize,
}

fn document_v1(
    route: &RouteV1,
    snapshot: &FractionalRetirementSnapshotV3,
    plan: &FractionalRetirementNextPlanV3,
) -> Result<PlanDocumentV1> {
    let instruction = &plan.instruction_plan;
    let request_bytes = instruction.request.to_bytes().map_err(|error| {
        Error::new(format!("Fractional retirement request encoding: {error:?}"))
    })?;
    if hash(&request_bytes).to_bytes() != instruction.request_digest
        || plan.message.required_signatures != 1
        || plan.message.message.static_account_keys().first() != Some(&route.payer)
        || plan.message.lookup_tables.as_slice() != route.lookup_table.as_slice()
        || instruction.observation != snapshot.core_market.observation
    {
        return Err(Error::new(
            "Fractional retirement serializer refused a plan/release/routing rejoin mismatch",
        ));
    }
    let unsigned = VersionedTransaction {
        signatures: vec![Signature::default(); usize::from(plan.message.required_signatures)],
        message: plan.message.message.clone(),
    };
    let packet = bincode::serialize(&unsigned).map_err(|error| {
        Error::new(format!(
            "Fractional retirement unsigned transaction serialization: {error}"
        ))
    })?;
    if packet.len() != plan.message.wire_bytes {
        return Err(Error::new(
            "Fractional retirement unsigned transaction width differs from its compiler report",
        ));
    }
    let request = instruction.request.input();
    Ok(PlanDocumentV1 {
        format: PLAN_FORMAT_V1,
        action: action_name_v1(instruction.action),
        coordinate: instruction.coordinate,
        transaction_base64: BASE64.encode(packet),
        observed_slot: instruction.observation.slot.to_string(),
        payer: route.payer.to_string(),
        required_signers: vec![route.payer.to_string()],
        request_base64: BASE64.encode(request_bytes),
        request_sha256: hex_v1(instruction.request_digest),
        release_set: hex_v1(request.release_set),
        market: Pubkey::new_from_array(request.market).to_string(),
        root: Pubkey::new_from_array(request.root).to_string(),
        rent_credit: Pubkey::new_from_array(request.rent_credit).to_string(),
        artifacts: ArtifactPinsV1 {
            terms: hex_v1(request.terms),
            token_behavior: hex_v1(request.token_behavior),
            exposure: hex_v1(request.exposure),
            terms_raw: snapshot.terms.raw.key.to_string(),
            token_behavior_raw: snapshot.token_behavior.raw.key.to_string(),
        },
        programs: ProgramPinsV1 {
            registry: snapshot.registry_program.key.to_string(),
            core: snapshot.core.program.key.to_string(),
            core_programdata: snapshot.core.programdata.key.to_string(),
            claims: snapshot.claims.program.key.to_string(),
            claims_programdata: snapshot.claims.programdata.key.to_string(),
            trading: snapshot.trading.program.key.to_string(),
            trading_programdata: snapshot.trading.programdata.key.to_string(),
            rent: snapshot.rent.program.key.to_string(),
            rent_programdata: snapshot.rent.programdata.key.to_string(),
            token: Pubkey::new_from_array(request.token_program).to_string(),
        },
        root_revision_anchor: instruction.root_revision_anchor.to_string(),
        expected_revision: instruction.expected_revision.to_string(),
        representation_width: instruction.representation_width,
        instruction: InstructionFrameV1 {
            program: instruction.instruction.program_id.to_string(),
            accounts: instruction.instruction.accounts.len(),
            data_bytes: instruction.instruction.data.len(),
        },
        wire: WireReportV1 {
            bytes: plan.message.wire_bytes,
            loaded_addresses: plan.message.loaded_addresses,
        },
        lookup_table: route.lookup_table.map(|key| key.to_string()),
        consequence: instruction.consequence,
        remedy: instruction.remedy,
    })
}

fn action_name_v1(action: FractionalRetirementActionV3) -> &'static str {
    match action {
        FractionalRetirementActionV3::Begin => "begin",
        FractionalRetirementActionV3::RetireCoordinate => "retire-coordinate",
        FractionalRetirementActionV3::Finish => "finish",
    }
}

fn hex_v1(value: [u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn encode_document_v1(document: &PlanDocumentV1) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(document)
        .map_err(|error| Error::new(format!("Fractional retirement plan JSON: {error}")))?;
    bytes.push(b'\n');
    if u64::try_from(bytes.len())
        .ok()
        .is_none_or(|length| length > MAX_DOCUMENT_BYTES_V1)
    {
        return Err(Error::new("Fractional retirement plan exceeds 64 KiB"));
    }
    Ok(bytes)
}

fn write_new_output_v1(path: &Path, bytes: &[u8]) -> Result<()> {
    if !path.is_absolute() || path.exists() {
        return Err(Error::new("--output must be one absent absolute path"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("--output has no parent directory"))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|error| Error::new(format!("cannot canonicalize --output parent: {error}")))?;
    if canonical_parent != parent || !canonical_parent.is_dir() {
        return Err(Error::new(
            "--output parent must be one canonical directory",
        ));
    }
    let temporary = canonical_parent.join(format!(
        ".dclutch-fractional-retirement-{}-{}.partial",
        std::process::id(),
        hex_v1(hash(bytes).to_bytes())
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|error| {
            Error::new(format!(
                "cannot create private Fractional retirement plan: {error}"
            ))
        })?;
    let publish = (|| -> Result<()> {
        file.write_all(bytes).map_err(|error| {
            Error::new(format!("cannot write Fractional retirement plan: {error}"))
        })?;
        file.sync_all().map_err(|error| {
            Error::new(format!("cannot sync Fractional retirement plan: {error}"))
        })?;
        drop(file);
        fs::hard_link(&temporary, path).map_err(|error| {
            Error::new(format!(
                "cannot publish absent Fractional retirement plan: {error}"
            ))
        })?;
        fs::remove_file(&temporary).map_err(|error| {
            Error::new(format!(
                "cannot remove Fractional retirement temporary: {error}"
            ))
        })?;
        File::open(&canonical_parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| {
                Error::new(format!(
                    "cannot sync Fractional retirement plan directory: {error}"
                ))
            })?;
        Ok(())
    })();
    if publish.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    publish
}

/// Explain the exact state-selected read-only planning act.
#[must_use]
pub fn usage() -> &'static str {
    "dclutch fractional-retirement-next — build the next unsigned ordered-retirement act.\n\
     \n\
     USAGE\n\
     \n\
       dclutch fractional-retirement-next --route ABSOLUTE.json --output ABSENT-ABSOLUTE.json [--rpc URL]\n\
     \n\
     The route carries only public deployment and account addresses. It cannot\n\
     choose an action, coordinate, Position, admission, or shard Mint. This\n\
     command first authenticates the finalized root and cursor to discover\n\
     those facts, then reacquires the whole graph together and writes one\n\
     mode-0600 unsigned v0 wallet handoff. The output path must not exist.\n\
     \n\
     No key is read. Nothing is signed, simulated, or submitted. If the recent\n\
     blockhash expires, reacquire state and run the command again.\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::{
        os::unix::fs::PermissionsExt as _,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn key(value: u8) -> String {
        Pubkey::new_from_array([value; 32]).to_string()
    }

    fn route_value() -> Value {
        json!({
            "format": ROUTE_FORMAT_V1,
            "minimumFinalizedSlot": "17",
            "payer": key(1),
            "root": key(2),
            "coreMarket": key(3),
            "claimsMarket": key(4),
            "activationCache": key(5),
            "registryProgram": key(6),
            "coreProgram": key(7),
            "coreProgramdata": key(8),
            "claimsProgram": key(9),
            "claimsProgramdata": key(10),
            "tradingProgram": key(11),
            "tradingProgramdata": key(12),
            "rentProgram": key(13),
            "rentProgramdata": key(14),
            "rentCredit": key(15),
            "cursor": key(16),
            "termsRaw": key(17),
            "termsStaging": key(18),
            "tokenBehaviorRaw": key(19),
            "tokenBehaviorStaging": key(20),
            "rentSysvar": key(21),
            "systemProgram": key(22),
            "tokenProgram": key(23)
        })
    }

    fn temporary_directory() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "dclutch-fractional-cli-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("temporary directory");
        path.canonicalize().expect("canonical temporary directory")
    }

    #[test]
    fn route_grammar_has_no_action_or_state_selected_account_inputs() {
        let directory = temporary_directory();
        let path = directory.join("route.json");
        fs::write(
            &path,
            serde_json::to_vec(&route_value()).expect("route JSON"),
        )
        .expect("route");
        let route = read_route_v1(&path).expect("exact route");
        assert_eq!(route.minimum_finalized_slot, 17);
        assert_eq!(route.root.to_string(), key(2));

        for field in [
            "action",
            "coordinate",
            "position",
            "admission",
            "shardMint",
            "endpoint",
            "keypair",
        ] {
            let mut hostile = route_value();
            hostile
                .as_object_mut()
                .expect("object")
                .insert(field.into(), json!("hostile"));
            fs::write(&path, serde_json::to_vec(&hostile).expect("hostile JSON"))
                .expect("hostile route");
            assert!(
                read_route_v1(&path)
                    .expect_err("unknown authority field")
                    .to_string()
                    .contains("unknown field")
            );
        }
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn route_refuses_duplicates_noncanonical_decimals_and_address_aliases() {
        let directory = temporary_directory();
        let path = directory.join("route.json");
        let duplicate =
            format!("{{\"format\":\"{ROUTE_FORMAT_V1}\",\"format\":\"{ROUTE_FORMAT_V1}\"}}");
        fs::write(&path, duplicate).expect("duplicate route");
        assert!(
            read_route_v1(&path)
                .expect_err("duplicate")
                .to_string()
                .contains("duplicate")
        );

        let mut noncanonical = route_value();
        noncanonical
            .as_object_mut()
            .expect("route object")
            .insert("minimumFinalizedSlot".into(), json!("017"));
        fs::write(&path, serde_json::to_vec(&noncanonical).expect("JSON")).expect("route");
        assert!(
            read_route_v1(&path)
                .expect_err("decimal")
                .to_string()
                .contains("canonical decimal")
        );

        let mut aliased = route_value();
        let core_market = aliased
            .as_object()
            .and_then(|object| object.get("coreMarket"))
            .cloned()
            .expect("Core Market");
        aliased
            .as_object_mut()
            .expect("route object")
            .insert("claimsMarket".into(), core_market);
        fs::write(&path, serde_json::to_vec(&aliased).expect("JSON")).expect("route");
        assert!(
            read_route_v1(&path)
                .expect_err("duplicate address")
                .to_string()
                .contains("pairwise distinct")
        );
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn output_is_private_absent_atomic_and_nonclobbering() {
        let directory = temporary_directory();
        let output = directory.join("plan.json");
        write_new_output_v1(&output, b"exact\n").expect("publish");
        assert_eq!(fs::read(&output).expect("read"), b"exact\n");
        assert_eq!(
            fs::metadata(&output)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(write_new_output_v1(&output, b"substitute\n").is_err());
        assert_eq!(fs::read(&output).expect("read"), b"exact\n");
        fs::remove_dir_all(directory).expect("cleanup");
    }

    #[test]
    fn arguments_are_absolute_exact_and_endpoint_redacted() {
        let error = PlanArgumentsV1::parse(vec![
            "--route".into(),
            "relative.json".into(),
            "--output".into(),
            "/tmp/out.json".into(),
        ])
        .expect_err("relative route");
        assert!(error.to_string().contains("--route must be an absolute"));
        let parsed = PlanArgumentsV1::parse(vec![
            "--rpc".into(),
            "https://rpc.example/SECRET".into(),
            "--route".into(),
            "/tmp/route.json".into(),
            "--output".into(),
            "/tmp/out.json".into(),
        ])
        .expect("arguments");
        assert!(!format!("{parsed:?}").contains("SECRET"));
    }
}
