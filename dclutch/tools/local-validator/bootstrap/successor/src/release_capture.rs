//! Key-free, read-only capture of the two live evidence bundles consumed by
//! the permanent-id checked release path.
//!
//! Neither command accepts a release identity, artifact identity, Loader
//! coordinate, or account body from the caller. Infrastructure coordinates are
//! discovered from the finalized singleton profile and then re-read together
//! in the one nine-account context the mixed deployment-set authenticator
//! consumes. The five mutable roles are observed as five Program/ProgramData
//! pairs in one finalized ten-account context.

use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Component, Path, PathBuf},
    str::FromStr as _,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::{
    ARTIFACT_RELEASE_BYTES_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ArtifactReleaseV1,
    DeploymentObservationV1, require_slot_pinned_release_v1,
};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_registry::release_set::{
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1,
    ProtocolInfrastructureProfileV1,
};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use solana_program::rent::Rent;
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use crate::{
    Error, Result,
    cluster::{ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG},
    rpc::{Rpc, RpcAccount, WritePolicyV1},
    upgrade::{CHECKED_ROLE_ORDER_V1, DevnetUpgradeTargetsV1},
};

const PUBLIC_DEVNET_ENDPOINT: &str = "https://api.devnet.solana.com";
const CARRY_FORWARD_SCHEMA: &str = "dclutch-carry-forward-rpc-snapshot-v2";
const PREPARE_BUNDLE_SCHEMA: &str = "dclutch-prepare-programdata-capture-v1";
const PREPARE_BUNDLE_DOMAIN: &[u8] = b"dclutch/prepare-programdata-capture/v1\n";
const PREPARE_MANIFEST_FILE: &str = "manifest.json";
const PERMANENT_SUBSTRATE_SCHEMA: &str = "dclutch-devnet-permanent-substrate-snapshot-v1";
const PERMANENT_SUBSTRATE_DOMAIN: &[u8] = b"dclutch/devnet-permanent-substrate-snapshot/v1\n";

const INFRASTRUCTURE_LABELS: [&str; 9] = [
    "registry_program",
    "registry_programdata",
    "rent_program",
    "rent_programdata",
    "registry_raw",
    "registry_staging",
    "rent_raw",
    "rent_staging",
    "infrastructure_profile",
];

const PREPARE_ROLES: [&str; 5] = ["custody", "resolution", "claims", "trading", "core"];

#[derive(Clone, Debug)]
struct CarryForwardArgsV1 {
    origin: ClusterOriginV1,
    registry_program: Pubkey,
    rent_program: Pubkey,
    core_program: Pubkey,
    expected_upgrade_authority: Pubkey,
    minimum_context_slot: u64,
    output_path: PathBuf,
}

#[derive(Clone, Debug)]
struct PrepareCaptureArgsV1 {
    origin: ClusterOriginV1,
    programs: [Pubkey; 5],
    expected_upgrade_authority: Pubkey,
    minimum_context_slot: u64,
    output_dir: PathBuf,
}

#[derive(Clone, Debug)]
struct PermanentSubstrateArgsV1 {
    origin: ClusterOriginV1,
    /// Decision 0012, amended 2026-09-02: the seven roles are a caller-declared
    /// AUTHENTICATED input, not a fixed table. Every ProgramData coordinate is
    /// Loader-derived from its Program and every account is then read back from
    /// a finalized cluster context, which is where the identity claim is
    /// actually settled.
    targets: DevnetUpgradeTargetsV1,
    expected_upgrade_authority: Pubkey,
    fee_payer: Pubkey,
    minimum_context_slot: u64,
    output_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CarryForwardAccountV1 {
    lamports: u64,
    owner: String,
    executable: bool,
    rent_epoch: u64,
    data_encoding: String,
    data_len: usize,
    data_base64: String,
    data_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CarryForwardSnapshotAccountV1 {
    role: String,
    address: String,
    account: Option<CarryForwardAccountV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CarryForwardSnapshotV1 {
    schema: String,
    endpoint: String,
    commitment: String,
    rpc_method: String,
    context_slot: u64,
    /// The Rent sysvar THIS context quoted, so a later offline re-validation
    /// judges these balances against the rate the cluster actually charged them
    /// rather than against `Rent::default()`. The snapshot is the only carrier
    /// that can: the re-validator has no cluster in reach.
    rent: CarryForwardRentV1,
    accounts: Vec<CarryForwardSnapshotAccountV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CarryForwardRentV1 {
    lamports_per_byte_year: u64,
    exemption_threshold: String,
    burn_percent: u8,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PrepareProgramdataRoleV1 {
    ordinal: u8,
    role: String,
    program_id: String,
    programdata_id: String,
    deployment_slot: u64,
    program_account_data_sha256: String,
    programdata_account_bytes: usize,
    programdata_account_sha256: String,
    live_elf_bytes: usize,
    live_elf_sha256: String,
    body_file: String,
    body_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PrepareProgramdataManifestV1 {
    schema: String,
    endpoint: String,
    commitment: String,
    rpc_method: String,
    context_slot: u64,
    expected_upgrade_authority: String,
    canonical_role_order: Vec<String>,
    roles: Vec<PrepareProgramdataRoleV1>,
    bundle_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PermanentSubstrateRoleV1 {
    ordinal: u8,
    role: String,
    program_id: String,
    programdata_id: String,
    program_lamports: u64,
    program_data_sha256: String,
    programdata_lamports: u64,
    programdata_account_bytes: usize,
    programdata_account_sha256: String,
    deployment_slot: u64,
    live_elf_bytes: usize,
    live_elf_sha256: String,
}

/// One finalized-context pre-write fact set for every permanent Loader pair
/// and the exclusive fee payer. This is deliberately a compact digest
/// projection; the carry-forward and prepare captures remain the owners of the
/// complete account bodies their downstream consumers need.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PermanentSubstrateSnapshotV1 {
    schema: String,
    endpoint: String,
    commitment: String,
    rpc_method: String,
    context_slot: u64,
    expected_upgrade_authority: String,
    fee_payer: String,
    fee_payer_lamports: u64,
    canonical_role_order: Vec<String>,
    roles: Vec<PermanentSubstrateRoleV1>,
    program_lamports_total: u64,
    programdata_lamports_total: u64,
    snapshot_sha256: String,
}

#[derive(Clone, Debug)]
struct LoaderFactsV1 {
    deployment_slot: u64,
    live_elf_bytes: usize,
    live_elf_sha256: String,
}

#[derive(Clone, Debug)]
struct InfrastructureCoordinatesV1 {
    addresses: [Pubkey; 9],
    registry_artifact_id: [u8; 32],
    rent_artifact_id: [u8; 32],
    discovery_profile_body: Vec<u8>,
}

/// Capture the exact nine-account CarryForward document consumed by mixed v2.
pub(crate) fn run_carry_forward(arguments: Vec<String>) -> Result<()> {
    let args = parse_carry_forward_args(arguments)?;
    require_public_devnet(&args.origin)?;
    require_new_file_path(&args.output_path, "carry-forward output")?;

    let registry_programdata = programdata(args.registry_program);
    let rent_programdata = programdata(args.rent_program);
    let profile_address = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &args.core_program,
    )
    .0;
    let discovery_addresses = [
        args.registry_program,
        registry_programdata,
        args.rent_program,
        rent_programdata,
        profile_address,
    ];
    let mut rpc = Rpc::connect_cluster(&args.origin, WritePolicyV1::ReadsOnly)?;
    // The Rent sysvar rides in the SAME finalized context as the balances it
    // will judge, and is sliced off before the nine-account closure is built --
    // so the snapshot schema is unchanged and the rate is not read at some other
    // slot than the accounts.
    let (discovery_slot, discovery_accounts, _discovery_rent) =
        finalized_accounts_with_rent(&mut rpc, &discovery_addresses, args.minimum_context_slot)?;
    let coordinates =
        discover_infrastructure_coordinates(&args, &discovery_addresses, &discovery_accounts)?;
    let minimum_final_slot = discovery_slot.max(args.minimum_context_slot);
    let (context_slot, accounts, rent) =
        finalized_accounts_with_rent(&mut rpc, &coordinates.addresses, minimum_final_slot)?;
    let snapshot =
        authenticate_carry_forward_snapshot(&args, context_slot, accounts, &coordinates, &rent)?;
    write_json_atomic_new(&args.output_path, &snapshot)?;

    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &snapshot)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Capture the five current ProgramData bodies checked prepare consumes.
pub(crate) fn run_prepare_programdata(arguments: Vec<String>) -> Result<()> {
    let args = parse_prepare_capture_args(arguments)?;
    require_public_devnet(&args.origin)?;
    require_new_directory_path(&args.output_dir, "ProgramData output directory")?;

    let mut addresses = Vec::with_capacity(10);
    for program in args.programs {
        addresses.push(program);
        addresses.push(programdata(program));
    }
    let mut rpc = Rpc::connect_cluster(&args.origin, WritePolicyV1::ReadsOnly)?;
    let (context_slot, accounts, _rent) =
        finalized_accounts_with_rent(&mut rpc, &addresses, args.minimum_context_slot)?;
    let (manifest, bodies) =
        authenticate_prepare_programdata(&args, context_slot, &addresses, accounts)?;
    write_prepare_bundle_atomic(&args.output_dir, &manifest, &bodies)?;

    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &manifest)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Capture all seven declared Loader pairs and the explicit fee payer in one
/// finalized `getMultipleAccounts` context. The command is key-free and
/// read-only; the seven roles are declared by the caller and authenticated
/// before a single account is read.
pub(crate) fn run_permanent_substrate(arguments: Vec<String>) -> Result<()> {
    let args = parse_permanent_substrate_args(arguments)?;
    require_public_devnet(&args.origin)?;
    require_new_file_path(&args.output_path, "permanent substrate output")?;

    let programs = target_programs(&args.targets);
    let mut addresses = Vec::with_capacity(programs.len() * 2 + 1);
    for (_, program, programdata) in &programs {
        addresses.extend([*program, *programdata]);
    }
    addresses.push(args.fee_payer);
    let mut rpc = Rpc::connect_cluster(&args.origin, WritePolicyV1::ReadsOnly)?;
    let (context_slot, accounts, rent) =
        finalized_accounts_with_rent(&mut rpc, &addresses, args.minimum_context_slot)?;
    let snapshot = authenticate_permanent_substrate(
        &args,
        context_slot,
        &addresses,
        accounts,
        &programs,
        &rent,
    )?;
    write_json_atomic_new(&args.output_path, &snapshot)?;

    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &snapshot)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    concat!(
        "  dclutch-local-successor-bootstrap devnet-carry-forward-capture-v1 \\\n",
        "    --rpc-url https://api.devnet.solana.com \\\n",
        "    --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\\n",
        "    --expected-registry-program PUBKEY --expected-rent-program PUBKEY \\\n",
        "    --expected-core-program PUBKEY --expected-upgrade-authority PUBKEY \\\n",
        "    --minimum-context-slot U64 --output ABSOLUTE_NEW_JSON\n",
        "    Discovers the immutable infrastructure profile first, then captures and ",
        "reauthenticates its complete canonical nine-account closure in one later ",
        "finalized getMultipleAccounts context. It is read-only and key-free.\n\n",
        "  dclutch-local-successor-bootstrap devnet-prepare-programdata-capture-v1 \\\n",
        "    --rpc-url https://api.devnet.solana.com \\\n",
        "    --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\\n",
        "    --expected-custody-program PUBKEY --expected-resolution-program PUBKEY \\\n",
        "    --expected-claims-program PUBKEY --expected-trading-program PUBKEY \\\n",
        "    --expected-core-program PUBKEY --expected-upgrade-authority PUBKEY \\\n",
        "    --minimum-context-slot U64 --output-dir ABSOLUTE_NEW_DIR\n",
        "    Captures the five exact full ProgramData account bodies in one finalized ",
        "ten-account Program/ProgramData observation. It writes the five bodies ",
        "before a manifest-last fsync commit and never overwrites an existing path.\n\n",
        "  dclutch-local-successor-bootstrap devnet-permanent-substrate-capture-v1 \\\n",
        "    --rpc-url https://api.devnet.solana.com \\\n",
        "    --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \\\n",
        "    --expected-registry-program PUBKEY --expected-rent-program PUBKEY \\\n",
        "    --expected-custody-program PUBKEY --expected-resolution-program PUBKEY \\\n",
        "    --expected-claims-program PUBKEY --expected-trading-program PUBKEY \\\n",
        "    --expected-core-program PUBKEY \\\n",
        "    --expected-upgrade-authority PUBKEY --fee-payer PUBKEY \\\n",
        "    --minimum-context-slot U64 --output ABSOLUTE_NEW_JSON\n",
        "    Captures the seven declared Program/ProgramData pairs and the explicit fee ",
        "payer in one finalized account context. It authenticates every Loader link, ",
        "authority, owner, privilege, slot, account digest, payload digest, parked-rent ",
        "total, and payer balance. It is read-only and key-free; decision 0012's target ",
        "set is a caller-declared authenticated input, and every ProgramData coordinate ",
        "is Loader-derived rather than accepted.",
    )
}

fn parse_carry_forward_args(arguments: Vec<String>) -> Result<CarryForwardArgsV1> {
    let mut fields = parse_pairs(
        arguments,
        &[
            "--rpc-url",
            DEVNET_ACKNOWLEDGMENT_FLAG,
            "--expected-registry-program",
            "--expected-rent-program",
            "--expected-core-program",
            "--expected-upgrade-authority",
            "--minimum-context-slot",
            "--output",
        ],
    )?;
    let rpc_url = take(&mut fields, "--rpc-url")?;
    let acknowledgment = take(&mut fields, DEVNET_ACKNOWLEDGMENT_FLAG)?;
    let registry_program = parse_pubkey(
        &take(&mut fields, "--expected-registry-program")?,
        "expected Registry Program",
    )?;
    let rent_program = parse_pubkey(
        &take(&mut fields, "--expected-rent-program")?,
        "expected Rent Program",
    )?;
    let core_program = parse_pubkey(
        &take(&mut fields, "--expected-core-program")?,
        "expected Core Program",
    )?;
    require_distinct_programs(&[registry_program, rent_program, core_program])?;
    let expected_upgrade_authority = parse_pubkey(
        &take(&mut fields, "--expected-upgrade-authority")?,
        "expected retained upgrade authority",
    )?;
    require_signing_authority(
        expected_upgrade_authority,
        &[registry_program, rent_program, core_program],
    )?;
    Ok(CarryForwardArgsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, Some(&acknowledgment))?,
        registry_program,
        rent_program,
        core_program,
        expected_upgrade_authority,
        minimum_context_slot: parse_nonzero_u64(
            &take(&mut fields, "--minimum-context-slot")?,
            "--minimum-context-slot",
        )?,
        output_path: absolute_path(&take(&mut fields, "--output")?, "--output")?,
    })
}

fn parse_prepare_capture_args(arguments: Vec<String>) -> Result<PrepareCaptureArgsV1> {
    let mut fields = parse_pairs(
        arguments,
        &[
            "--rpc-url",
            DEVNET_ACKNOWLEDGMENT_FLAG,
            "--expected-custody-program",
            "--expected-resolution-program",
            "--expected-claims-program",
            "--expected-trading-program",
            "--expected-core-program",
            "--expected-upgrade-authority",
            "--minimum-context-slot",
            "--output-dir",
        ],
    )?;
    let rpc_url = take(&mut fields, "--rpc-url")?;
    let acknowledgment = take(&mut fields, DEVNET_ACKNOWLEDGMENT_FLAG)?;
    let programs = [
        parse_pubkey(
            &take(&mut fields, "--expected-custody-program")?,
            "expected Custody Program",
        )?,
        parse_pubkey(
            &take(&mut fields, "--expected-resolution-program")?,
            "expected Resolution Program",
        )?,
        parse_pubkey(
            &take(&mut fields, "--expected-claims-program")?,
            "expected Claims Program",
        )?,
        parse_pubkey(
            &take(&mut fields, "--expected-trading-program")?,
            "expected Trading Program",
        )?,
        parse_pubkey(
            &take(&mut fields, "--expected-core-program")?,
            "expected Core Program",
        )?,
    ];
    require_distinct_programs(&programs)?;
    let expected_upgrade_authority = parse_pubkey(
        &take(&mut fields, "--expected-upgrade-authority")?,
        "expected retained upgrade authority",
    )?;
    require_signing_authority(expected_upgrade_authority, &programs)?;
    Ok(PrepareCaptureArgsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, Some(&acknowledgment))?,
        programs,
        expected_upgrade_authority,
        minimum_context_slot: parse_nonzero_u64(
            &take(&mut fields, "--minimum-context-slot")?,
            "--minimum-context-slot",
        )?,
        output_dir: absolute_path(&take(&mut fields, "--output-dir")?, "--output-dir")?,
    })
}

fn parse_permanent_substrate_args(arguments: Vec<String>) -> Result<PermanentSubstrateArgsV1> {
    let role_flags = CHECKED_ROLE_ORDER_V1.map(|role| format!("--expected-{role}-program"));
    let mut allowed = vec![
        "--rpc-url".to_owned(),
        DEVNET_ACKNOWLEDGMENT_FLAG.to_owned(),
        "--expected-upgrade-authority".to_owned(),
        "--fee-payer".to_owned(),
        "--minimum-context-slot".to_owned(),
        "--output".to_owned(),
    ];
    allowed.extend(role_flags.iter().cloned());
    let allowed_refs = allowed.iter().map(String::as_str).collect::<Vec<_>>();
    let mut fields = parse_pairs(arguments, &allowed_refs)?;
    let rpc_url = take(&mut fields, "--rpc-url")?;
    let acknowledgment = take(&mut fields, DEVNET_ACKNOWLEDGMENT_FLAG)?;
    let expected_upgrade_authority = parse_pubkey(
        &take(&mut fields, "--expected-upgrade-authority")?,
        "expected retained upgrade authority",
    )?;
    let fee_payer = parse_pubkey(&take(&mut fields, "--fee-payer")?, "fee payer")?;
    let mut declared = Vec::with_capacity(CHECKED_ROLE_ORDER_V1.len());
    for (role, flag) in CHECKED_ROLE_ORDER_V1.iter().zip(role_flags.iter()) {
        let program = parse_pubkey(&take(&mut fields, flag)?, "declared Program")?;
        declared.push((*role, program.to_string(), programdata(program).to_string()));
    }
    let targets = DevnetUpgradeTargetsV1::authenticate(
        declared
            .iter()
            .map(|(role, program, programdata)| (*role, program.as_str(), programdata.as_str())),
    )?;
    let programs = declared
        .iter()
        .map(|(_, program, _)| parse_pubkey(program, "declared Program"))
        .collect::<Result<Vec<_>>>()?;
    require_signing_authority(expected_upgrade_authority, &programs)?;
    require_signing_authority(fee_payer, &programs)?;
    Ok(PermanentSubstrateArgsV1 {
        origin: ClusterOriginV1::parse(&rpc_url, Some(&acknowledgment))?,
        targets,
        expected_upgrade_authority,
        fee_payer,
        minimum_context_slot: parse_nonzero_u64(
            &take(&mut fields, "--minimum-context-slot")?,
            "--minimum-context-slot",
        )?,
        output_path: absolute_path(&take(&mut fields, "--output")?, "--output")?,
    })
}

fn parse_pairs(
    arguments: Vec<String>,
    allowed: &[&str],
) -> Result<std::collections::BTreeMap<String, String>> {
    let mut fields = std::collections::BTreeMap::new();
    let mut iterator = arguments.into_iter();
    while let Some(flag) = iterator.next() {
        if !allowed.contains(&flag.as_str()) {
            return Err(Error::new(format!(
                "unknown release capture argument: {flag}"
            )));
        }
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{flag} requires a value")))?;
        if fields.insert(flag.clone(), value).is_some() {
            return Err(Error::new(format!("{flag} may be supplied only once")));
        }
    }
    Ok(fields)
}

fn take(fields: &mut std::collections::BTreeMap<String, String>, flag: &str) -> Result<String> {
    fields
        .remove(flag)
        .ok_or_else(|| Error::new(format!("{flag} is required")))
}

fn discover_infrastructure_coordinates(
    args: &CarryForwardArgsV1,
    addresses: &[Pubkey; 5],
    accounts: &[Option<RpcAccount>],
) -> Result<InfrastructureCoordinatesV1> {
    if accounts.len() != addresses.len() {
        return Err(Error::new(
            "infrastructure discovery returned the wrong account count",
        ));
    }
    let registry_programdata = programdata(args.registry_program);
    let rent_programdata = programdata(args.rent_program);
    let registry_program = required(accounts, 0, "Registry Program")?;
    let registry_pd = required(accounts, 1, "Registry ProgramData")?;
    let rent_program = required(accounts, 2, "Rent Program")?;
    let rent_pd = required(accounts, 3, "Rent ProgramData")?;
    let profile_account = required(accounts, 4, "infrastructure profile")?;
    let _ = authenticate_loader_pair(
        "Registry",
        args.registry_program,
        registry_programdata,
        args.expected_upgrade_authority,
        registry_program,
        registry_pd,
    )?;
    let _ = authenticate_loader_pair(
        "Rent",
        args.rent_program,
        rent_programdata,
        args.expected_upgrade_authority,
        rent_program,
        rent_pd,
    )?;
    require_account_shape(
        "infrastructure profile",
        profile_account,
        args.core_program,
        false,
        Some(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1),
    )?;
    require_funded_rent_persists("infrastructure profile", profile_account)?;
    let profile = ProtocolInfrastructureProfileV1::decode(&profile_account.data)
        .map_err(|error| Error::new(format!("infrastructure profile decode: {error:?}")))?;
    if profile.registry().program().to_bytes() != args.registry_program.to_bytes()
        || profile.rent().program().to_bytes() != args.rent_program.to_bytes()
    {
        return Err(Error::new(
            "infrastructure profile does not bind the acknowledged Registry and Rent Programs",
        ));
    }
    let registry_artifact_id = profile.registry().artifact_release().to_bytes();
    let rent_artifact_id = profile.rent().artifact_release().to_bytes();
    let registry_raw = raw_record(args.registry_program, registry_artifact_id);
    let registry_staging = staging_record(args.registry_program, registry_artifact_id);
    let rent_raw = raw_record(args.registry_program, rent_artifact_id);
    let rent_staging = staging_record(args.registry_program, rent_artifact_id);
    Ok(InfrastructureCoordinatesV1 {
        addresses: [
            args.registry_program,
            registry_programdata,
            args.rent_program,
            rent_programdata,
            registry_raw,
            registry_staging,
            rent_raw,
            rent_staging,
            *addresses
                .get(4)
                .ok_or_else(|| Error::new("infrastructure discovery omitted profile address"))?,
        ],
        registry_artifact_id,
        rent_artifact_id,
        discovery_profile_body: profile_account.data.clone(),
    })
}

fn authenticate_carry_forward_snapshot(
    args: &CarryForwardArgsV1,
    context_slot: u64,
    accounts: Vec<Option<RpcAccount>>,
    coordinates: &InfrastructureCoordinatesV1,
    rent: &Rent,
) -> Result<CarryForwardSnapshotV1> {
    if context_slot < args.minimum_context_slot || accounts.len() != coordinates.addresses.len() {
        return Err(Error::new(
            "final carry-forward observation context or width is invalid",
        ));
    }
    if accounts.get(5).is_some_and(Option::is_some) || accounts.get(7).is_some_and(Option::is_some)
    {
        return Err(Error::new(
            "Registry and Rent staging accounts must be finalized RPC null",
        ));
    }
    for (index, label) in [
        (0, "Registry Program"),
        (1, "Registry ProgramData"),
        (2, "Rent Program"),
        (3, "Rent ProgramData"),
        (4, "Registry artifact raw"),
        (6, "Rent artifact raw"),
        (8, "infrastructure profile"),
    ] {
        let _ = required(&accounts, index, label)?;
    }
    let registry_facts = authenticate_loader_pair(
        "Registry",
        args.registry_program,
        programdata(args.registry_program),
        args.expected_upgrade_authority,
        required(&accounts, 0, "Registry Program")?,
        required(&accounts, 1, "Registry ProgramData")?,
    )?;
    let rent_facts = authenticate_loader_pair(
        "Rent",
        args.rent_program,
        programdata(args.rent_program),
        args.expected_upgrade_authority,
        required(&accounts, 2, "Rent Program")?,
        required(&accounts, 3, "Rent ProgramData")?,
    )?;
    let profile_account = required(&accounts, 8, "infrastructure profile")?;
    require_account_shape(
        "infrastructure profile",
        profile_account,
        args.core_program,
        false,
        Some(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1),
    )?;
    require_funded_rent_persists("infrastructure profile", profile_account)?;
    if profile_account.data != coordinates.discovery_profile_body {
        return Err(Error::new(
            "infrastructure profile moved between discovery and the final one-context observation",
        ));
    }
    let profile = ProtocolInfrastructureProfileV1::decode(&profile_account.data)
        .map_err(|error| Error::new(format!("final infrastructure profile decode: {error:?}")))?;
    if profile.registry().program().to_bytes() != args.registry_program.to_bytes()
        || profile.rent().program().to_bytes() != args.rent_program.to_bytes()
        || profile.registry().artifact_release().to_bytes() != coordinates.registry_artifact_id
        || profile.rent().artifact_release().to_bytes() != coordinates.rent_artifact_id
    {
        return Err(Error::new(
            "final infrastructure profile bindings differ from discovery",
        ));
    }
    authenticate_artifact_release(
        "Registry",
        required(&accounts, 4, "Registry artifact raw")?,
        args.registry_program,
        args.registry_program,
        programdata(args.registry_program),
        args.expected_upgrade_authority,
        coordinates.registry_artifact_id,
        required(&accounts, 0, "Registry Program")?,
        required(&accounts, 1, "Registry ProgramData")?,
        &registry_facts,
    )?;
    authenticate_artifact_release(
        "Rent",
        required(&accounts, 6, "Rent artifact raw")?,
        args.registry_program,
        args.rent_program,
        programdata(args.rent_program),
        args.expected_upgrade_authority,
        coordinates.rent_artifact_id,
        required(&accounts, 2, "Rent Program")?,
        required(&accounts, 3, "Rent ProgramData")?,
        &rent_facts,
    )?;

    let rows = coordinates
        .addresses
        .iter()
        .copied()
        .zip(accounts)
        .zip(INFRASTRUCTURE_LABELS)
        .map(|((address, account), role)| CarryForwardSnapshotAccountV1 {
            role: role.into(),
            address: address.to_string(),
            account: account.map(snapshot_account),
        })
        .collect();
    Ok(CarryForwardSnapshotV1 {
        schema: CARRY_FORWARD_SCHEMA.into(),
        endpoint: PUBLIC_DEVNET_ENDPOINT.into(),
        commitment: "finalized".into(),
        rpc_method: "getMultipleAccounts".into(),
        context_slot,
        rent: carry_forward_rent_v1(rent),
        accounts: rows,
    })
}

/// The live rate, recorded exactly. `exemption_threshold` is an `f64` in the
/// sysvar and is written as its shortest round-tripping decimal rather than a
/// float: the offline re-validator must reproduce this number, not approximate
/// it, and JSON floats are not a shape to bet an identity on.
fn carry_forward_rent_v1(rent: &Rent) -> CarryForwardRentV1 {
    CarryForwardRentV1 {
        lamports_per_byte_year: rent.lamports_per_byte_year,
        exemption_threshold: format!("{:?}", rent.exemption_threshold),
        burn_percent: rent.burn_percent,
    }
}

fn authenticate_prepare_programdata(
    args: &PrepareCaptureArgsV1,
    context_slot: u64,
    addresses: &[Pubkey],
    accounts: Vec<Option<RpcAccount>>,
) -> Result<(PrepareProgramdataManifestV1, Vec<Vec<u8>>)> {
    if context_slot < args.minimum_context_slot || addresses.len() != 10 || accounts.len() != 10 {
        return Err(Error::new(
            "prepare ProgramData observation context or width is invalid",
        ));
    }
    let mut roles = Vec::with_capacity(PREPARE_ROLES.len());
    let mut bodies = Vec::with_capacity(PREPARE_ROLES.len());
    for (ordinal, (((role, program), address_pair), account_pair)) in PREPARE_ROLES
        .iter()
        .zip(args.programs)
        .zip(addresses.chunks_exact(2))
        .zip(accounts.chunks_exact(2))
        .enumerate()
    {
        let programdata = programdata(program);
        if address_pair != [program, programdata] {
            return Err(Error::new(format!(
                "{role} Program/ProgramData address order is not canonical"
            )));
        }
        let program_account = account_pair
            .first()
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("missing {role} Program account")))?;
        let programdata_account = account_pair
            .get(1)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("missing {role} ProgramData account")))?;
        let facts = authenticate_loader_pair(
            role,
            program,
            programdata,
            args.expected_upgrade_authority,
            program_account,
            programdata_account,
        )?;
        let ordinal_u8 = u8::try_from(ordinal)
            .map_err(|_| Error::new("prepare ProgramData role ordinal overflow"))?;
        let body_file = format!("{ordinal:02}-{role}-programdata.bin");
        roles.push(PrepareProgramdataRoleV1 {
            ordinal: ordinal_u8,
            role: (*role).into(),
            program_id: program.to_string(),
            programdata_id: programdata.to_string(),
            deployment_slot: facts.deployment_slot,
            program_account_data_sha256: digest(&program_account.data),
            programdata_account_bytes: programdata_account.data.len(),
            programdata_account_sha256: digest(&programdata_account.data),
            live_elf_bytes: facts.live_elf_bytes,
            live_elf_sha256: facts.live_elf_sha256,
            body_file: body_file.clone(),
            body_path: args.output_dir.join(&body_file).display().to_string(),
        });
        bodies.push(programdata_account.data.clone());
    }
    let mut manifest = PrepareProgramdataManifestV1 {
        schema: PREPARE_BUNDLE_SCHEMA.into(),
        endpoint: PUBLIC_DEVNET_ENDPOINT.into(),
        commitment: "finalized".into(),
        rpc_method: "getMultipleAccounts".into(),
        context_slot,
        expected_upgrade_authority: args.expected_upgrade_authority.to_string(),
        canonical_role_order: PREPARE_ROLES.iter().map(|role| (*role).into()).collect(),
        roles,
        bundle_sha256: String::new(),
    };
    manifest.bundle_sha256 = prepare_bundle_digest(&manifest, &bodies)?;
    Ok((manifest, bodies))
}

/// The ordered rows an authenticated target set names. `DevnetUpgradeTargetsV1`
/// has already checked the canonical role order, the Loader-derived ProgramData
/// coordinate of every Program, and fourteen distinct non-native accounts.
fn target_programs(targets: &DevnetUpgradeTargetsV1) -> Vec<(&'static str, Pubkey, Pubkey)> {
    targets
        .iter()
        .map(|target| (target.role, target.program, target.programdata))
        .collect()
}

fn authenticate_permanent_substrate(
    args: &PermanentSubstrateArgsV1,
    context_slot: u64,
    addresses: &[Pubkey],
    accounts: Vec<Option<RpcAccount>>,
    programs: &[(&'static str, Pubkey, Pubkey)],
    rent: &Rent,
) -> Result<PermanentSubstrateSnapshotV1> {
    let expected_width = programs
        .len()
        .checked_mul(2)
        .and_then(|width| width.checked_add(1))
        .ok_or_else(|| Error::new("permanent substrate observation width overflow"))?;
    if context_slot < args.minimum_context_slot
        || addresses.len() != expected_width
        || accounts.len() != expected_width
        || addresses.last() != Some(&args.fee_payer)
    {
        return Err(Error::new(
            "permanent substrate observation context, width, or payer coordinate is invalid",
        ));
    }

    let mut roles = Vec::with_capacity(programs.len());
    let mut program_lamports_total = 0_u64;
    let mut programdata_lamports_total = 0_u64;
    for (ordinal, ((role, program, programdata), (address_pair, account_pair))) in programs
        .iter()
        .zip(
            addresses[..addresses.len() - 1]
                .chunks_exact(2)
                .zip(accounts[..accounts.len() - 1].chunks_exact(2)),
        )
        .enumerate()
    {
        if address_pair != [*program, *programdata] {
            return Err(Error::new(format!(
                "permanent {role} Program/ProgramData address order is not canonical"
            )));
        }
        let program_account = account_pair
            .first()
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("missing permanent {role} Program account")))?;
        let programdata_account = account_pair
            .get(1)
            .and_then(Option::as_ref)
            .ok_or_else(|| Error::new(format!("missing permanent {role} ProgramData account")))?;
        let facts = authenticate_loader_pair(
            role,
            *program,
            *programdata,
            args.expected_upgrade_authority,
            program_account,
            programdata_account,
        )?;
        program_lamports_total = program_lamports_total
            .checked_add(program_account.lamports)
            .ok_or_else(|| Error::new("permanent Program lamport total overflow"))?;
        programdata_lamports_total = programdata_lamports_total
            .checked_add(programdata_account.lamports)
            .ok_or_else(|| Error::new("permanent ProgramData lamport total overflow"))?;
        roles.push(PermanentSubstrateRoleV1 {
            ordinal: u8::try_from(ordinal)
                .map_err(|_| Error::new("permanent role ordinal overflow"))?,
            role: (*role).into(),
            program_id: program.to_string(),
            programdata_id: programdata.to_string(),
            program_lamports: program_account.lamports,
            program_data_sha256: digest(&program_account.data),
            programdata_lamports: programdata_account.lamports,
            programdata_account_bytes: programdata_account.data.len(),
            programdata_account_sha256: digest(&programdata_account.data),
            deployment_slot: facts.deployment_slot,
            live_elf_bytes: facts.live_elf_bytes,
            live_elf_sha256: facts.live_elf_sha256,
        });
    }

    let payer = accounts
        .last()
        .and_then(Option::as_ref)
        .ok_or_else(|| Error::new("explicit permanent-substrate fee payer is absent"))?;
    require_account_shape("fee payer", payer, system_program::ID, false, Some(0))?;
    require_fee_payer_rent_floor_v1("fee payer", payer, rent)?;

    let mut snapshot = PermanentSubstrateSnapshotV1 {
        schema: PERMANENT_SUBSTRATE_SCHEMA.into(),
        endpoint: PUBLIC_DEVNET_ENDPOINT.into(),
        commitment: "finalized".into(),
        rpc_method: "getMultipleAccounts".into(),
        context_slot,
        expected_upgrade_authority: args.expected_upgrade_authority.to_string(),
        fee_payer: args.fee_payer.to_string(),
        fee_payer_lamports: payer.lamports,
        canonical_role_order: CHECKED_ROLE_ORDER_V1
            .iter()
            .map(|role| (*role).into())
            .collect(),
        roles,
        program_lamports_total,
        programdata_lamports_total,
        snapshot_sha256: String::new(),
    };
    snapshot.snapshot_sha256 = permanent_substrate_digest(&snapshot)?;
    Ok(snapshot)
}

fn permanent_substrate_digest(snapshot: &PermanentSubstrateSnapshotV1) -> Result<String> {
    let mut canonical = snapshot.clone();
    canonical.snapshot_sha256.clear();
    let bytes = serde_json::to_vec(&canonical)?;
    let mut hasher = Sha256::new();
    hasher.update(PERMANENT_SUBSTRATE_DOMAIN);
    hash_field(&mut hasher, &bytes)?;
    Ok(hex(&hasher.finalize()))
}

#[allow(clippy::too_many_arguments)]
fn authenticate_artifact_release(
    label: &str,
    raw_account: &RpcAccount,
    record_owner: Pubkey,
    program: Pubkey,
    programdata: Pubkey,
    authority: Pubkey,
    expected_artifact_id: [u8; 32],
    program_account: &RpcAccount,
    programdata_account: &RpcAccount,
    facts: &LoaderFactsV1,
) -> Result<()> {
    require_account_shape(
        &format!("{label} artifact raw"),
        raw_account,
        // Both artifact records live in Registry, including Rent's record.
        record_owner,
        false,
        Some(ARTIFACT_RELEASE_BYTES_V1),
    )?;
    if digest_bytes(&raw_account.data) != expected_artifact_id {
        return Err(Error::new(format!(
            "{label} artifact body does not match the profile-selected content ID"
        )));
    }
    require_funded_rent_persists(&format!("{label} artifact raw"), raw_account)?;
    let release = ArtifactReleaseV1::decode(&raw_account.data)
        .map_err(|error| Error::new(format!("{label} artifact release decode: {error:?}")))?;
    require_slot_pinned_release_v1(release)
        .map_err(|error| Error::new(format!("{label} artifact release policy: {error:?}")))?;
    let program_view = ProgramV3View::parse(&program_account.data)
        .map_err(|error| Error::new(format!("{label} Program decode: {error:?}")))?;
    let observation = DeploymentObservationV1::new(
        program.to_bytes(),
        program_account.owner.to_bytes(),
        program_account.executable,
        programdata.to_bytes(),
        programdata_account.owner.to_bytes(),
        programdata_account.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        facts.deployment_slot,
        hex32(&facts.live_elf_sha256)?,
        Some(authority.to_bytes()),
    )
    .map_err(|error| Error::new(format!("{label} deployment observation: {error:?}")))?;
    release
        .authenticate_deployment(observation)
        .map_err(|error| Error::new(format!("{label} artifact deployment: {error:?}")))
}

fn authenticate_loader_pair(
    label: &str,
    program: Pubkey,
    expected_programdata: Pubkey,
    expected_authority: Pubkey,
    program_account: &RpcAccount,
    programdata_account: &RpcAccount,
) -> Result<LoaderFactsV1> {
    if programdata(program) != expected_programdata {
        return Err(Error::new(format!(
            "{label} ProgramData coordinate is not Loader-derived"
        )));
    }
    require_account_shape(
        &format!("{label} Program"),
        program_account,
        bpf_loader_upgradeable::ID,
        true,
        Some(36),
    )?;
    require_account_shape(
        &format!("{label} ProgramData"),
        programdata_account,
        bpf_loader_upgradeable::ID,
        false,
        None,
    )?;
    require_funded_rent_persists(&format!("{label} Program"), program_account)?;
    require_funded_rent_persists(&format!("{label} ProgramData"), programdata_account)?;
    let program_view = ProgramV3View::parse(&program_account.data)
        .map_err(|error| Error::new(format!("{label} Program decode: {error:?}")))?;
    let programdata_view = ProgramDataV3View::parse(&programdata_account.data)
        .map_err(|error| Error::new(format!("{label} ProgramData decode: {error:?}")))?;
    if program_view.programdata() != expected_programdata.to_bytes() {
        return Err(Error::new(format!(
            "{label} Program links another ProgramData account"
        )));
    }
    if programdata_view.upgrade_authority() != Some(expected_authority.to_bytes()) {
        return Err(Error::new(format!(
            "{label} retained upgrade authority differs"
        )));
    }
    if programdata_view.deployment_slot() == 0 {
        return Err(Error::new(format!(
            "{label} ProgramData deployment slot is zero"
        )));
    }
    Ok(LoaderFactsV1 {
        deployment_slot: programdata_view.deployment_slot(),
        live_elf_bytes: programdata_view.elf().len(),
        live_elf_sha256: digest(programdata_view.elf()),
    })
}

fn require_account_shape(
    label: &str,
    account: &RpcAccount,
    owner: Pubkey,
    executable: bool,
    data_len: Option<usize>,
) -> Result<()> {
    if account.owner != owner
        || account.executable != executable
        || data_len.is_some_and(|expected| account.data.len() != expected)
    {
        return Err(Error::new(format!(
            "{label} owner/executable/length shape is invalid"
        )));
    }
    Ok(())
}

/// Read one finalized account context WITH the Rent sysvar appended, then hand
/// back the caller's accounts and the live rate that context quoted. The sysvar
/// is sliced off, so no snapshot width or schema changes.
fn finalized_accounts_with_rent(
    rpc: &mut Rpc,
    addresses: &[Pubkey],
    minimum_context_slot: u64,
) -> Result<(u64, Vec<Option<RpcAccount>>, Rent)> {
    let mut query = addresses.to_vec();
    query.push(sysvar::rent::ID);
    let (context_slot, mut accounts) = rpc.finalized_accounts(&query, minimum_context_slot)?;
    if accounts.len() != query.len() {
        return Err(Error::new(
            "finalized observation returned the wrong account count",
        ));
    }
    let rent_account = accounts
        .pop()
        .flatten()
        .ok_or_else(|| Error::new("the finalized Rent sysvar account is absent"))?;
    Ok((context_slot, accounts, live_rent(&rent_account)?))
}

/// Decode the Rent sysvar account read in this observation's own finalized
/// context, and refuse anything but its exact canonical body.
fn live_rent(account: &RpcAccount) -> Result<Rent> {
    if account.owner != sysvar::ID || account.executable {
        return Err(Error::new(
            "the Rent sysvar account is not an owned non-executable sysvar",
        ));
    }
    let rent: Rent = bincode::deserialize(&account.data)
        .map_err(|error| Error::new(format!("finalized Rent sysvar: {error}")))?;
    if bincode::serialize(&rent)
        .map_err(|error| Error::new(format!("re-encode finalized Rent sysvar: {error}")))?
        != account.data
    {
        return Err(Error::new(
            "finalized Rent sysvar was not its exact canonical body",
        ));
    }
    if rent.lamports_per_byte_year == 0 || rent.exemption_threshold <= 0.0 {
        return Err(Error::new(
            "finalized Rent sysvar quotes a zero rate or a non-positive exemption threshold",
        ));
    }
    Ok(rent)
}

/// An ALREADY-DEPLOYED account still holds the rent its deployment funded.
///
/// This preflight reads accounts some earlier cohort's transactions created and
/// funded, and it used to price them at the rate of the moment. That is the
/// wrong question in a direction this tool cannot afford, because it gates a
/// redeploy: a cluster whose rate RISES after a cohort was funded would leave
/// every one of that cohort's accounts below today's minimum, and the preflight
/// would refuse a redeploy over a substrate the cluster itself considers alive.
/// Measured, not supposed -- cohort-15's seven Programs each hold 1,038,612
/// lamports over 36 bytes, which is 164 x 6,333, and devnet quoted 5,080 at
/// finalized slot 493,000,156. `Rent::default()` prices those same 36 bytes at
/// 1,141,440. One deployed cohort, three rates, and only one of them funded it.
///
/// `solana-svm 4.3.0-beta.2` says the funded rate is the one that counts:
/// `src/rent_calculator.rs` carries no rent-collection path at all, so nothing
/// debits a funded account for the passage of time or a change of rate;
/// `get_pre_exec_account_rent_state` reads an account a raised rate left behind
/// as `RentExempt` under SIMD-0392; and `transition_allowed` refuses
/// `RentExempt -> RentPaying`, so no transaction can leave a live account
/// partially rented. The one case the runtime has not already decided is
/// `lamports == 0` -- a drained account whose data is residue -- and
/// [`funded_rent_persists_v1`] is exactly that case, rate-free.
///
/// The rate the deployment paid is still captured, in
/// [`carry_forward_rent_v1`], because a re-validator reproducing this evidence
/// offline needs the number. It is recorded, not used as a threshold.
fn require_funded_rent_persists(label: &str, account: &RpcAccount) -> Result<()> {
    if !funded_rent_persists_v1(account.lamports) {
        return Err(Error::new(format!(
            "{label} account is DRAINED: it holds 0 lamports over {} bytes, so its data is residue \
             the runtime reaps rather than state a redeploy may build on",
            account.data.len()
        )));
    }
    Ok(())
}

/// The exclusive fee payer, floored at the LIVE rate -- which is the runtime's
/// own question for this one account and no other in this file.
///
/// Every other account here is read and left alone, and the runtime
/// grandfathers what it was funded at. The fee payer's balance FALLS, by
/// construction, and a falling balance is the exact case SIMD-0392 does not
/// grandfather: `validate_fee_payer` charges the fee and then calls
/// `check_static_account_rent_state_transition`, whose post-execution state is
/// computed against `rent.minimum_balance(len)` at TODAY's rate. A payer that
/// ends below it is `RentPaying`, the transition from `RentExempt` is refused,
/// and the deploy fails with `InsufficientFundsForRent` no matter what the
/// payer was funded at. So this floor is not a statement about the past; it is
/// the runtime's precondition read one step early.
///
/// It is a floor and not the whole precondition: the runtime wants
/// `minimum_balance(len) + fee`, and the fees of a seven-role redeploy are not
/// known here. `Rent::default()` is still never the rate to ask -- measured
/// 2026-09-02, devnet's live rate is LOWER than the genesis constant, and a
/// default-derived demand called all seven of cohort-12's genuinely exempt
/// accounts not exempt and cost a night and 0.2373 SOL of unnecessary top-ups.
/// The error names both numbers, because a threshold refusal that does not say
/// what it wanted is a search rather than a finding.
fn require_fee_payer_rent_floor_v1(label: &str, account: &RpcAccount, rent: &Rent) -> Result<()> {
    let minimum = rent.minimum_balance(account.data.len());
    if account.lamports < minimum {
        return Err(Error::new(format!(
            "{label} account holds {} lamports over {} bytes; the live Rent sysvar requires {minimum}",
            account.lamports,
            account.data.len()
        )));
    }
    Ok(())
}

fn required<'a>(
    accounts: &'a [Option<RpcAccount>],
    index: usize,
    label: &str,
) -> Result<&'a RpcAccount> {
    accounts
        .get(index)
        .and_then(Option::as_ref)
        .ok_or_else(|| Error::new(format!("missing {label} account")))
}

fn snapshot_account(account: RpcAccount) -> CarryForwardAccountV1 {
    CarryForwardAccountV1 {
        lamports: account.lamports,
        owner: account.owner.to_string(),
        executable: account.executable,
        rent_epoch: account.rent_epoch,
        data_encoding: "base64".into(),
        data_len: account.data.len(),
        data_sha256: digest(&account.data),
        data_base64: BASE64.encode(account.data),
    }
}

fn prepare_bundle_digest(
    manifest: &PrepareProgramdataManifestV1,
    bodies: &[Vec<u8>],
) -> Result<String> {
    if manifest.roles.len() != bodies.len() {
        return Err(Error::new(
            "prepare ProgramData manifest/body count differs",
        ));
    }
    let mut hasher = Sha256::new();
    hasher.update(PREPARE_BUNDLE_DOMAIN);
    hash_field(&mut hasher, manifest.schema.as_bytes())?;
    hash_field(&mut hasher, manifest.endpoint.as_bytes())?;
    hash_field(&mut hasher, manifest.commitment.as_bytes())?;
    hash_field(&mut hasher, manifest.rpc_method.as_bytes())?;
    hasher.update(manifest.context_slot.to_le_bytes());
    hash_field(&mut hasher, manifest.expected_upgrade_authority.as_bytes())?;
    hash_field(
        &mut hasher,
        &u64::try_from(manifest.canonical_role_order.len())
            .map_err(|_| Error::new("canonical role order is too wide"))?
            .to_le_bytes(),
    )?;
    for role in &manifest.canonical_role_order {
        hash_field(&mut hasher, role.as_bytes())?;
    }
    hash_field(
        &mut hasher,
        &u64::try_from(manifest.roles.len())
            .map_err(|_| Error::new("prepare role list is too wide"))?
            .to_le_bytes(),
    )?;
    for (role, body) in manifest.roles.iter().zip(bodies) {
        hasher.update([role.ordinal]);
        hash_field(&mut hasher, role.role.as_bytes())?;
        hash_field(&mut hasher, role.program_id.as_bytes())?;
        hash_field(&mut hasher, role.programdata_id.as_bytes())?;
        hasher.update(role.deployment_slot.to_le_bytes());
        hash_field(&mut hasher, role.program_account_data_sha256.as_bytes())?;
        hasher.update(
            u64::try_from(role.programdata_account_bytes)
                .map_err(|_| Error::new("ProgramData account body is too wide"))?
                .to_le_bytes(),
        );
        hash_field(&mut hasher, role.programdata_account_sha256.as_bytes())?;
        hasher.update(
            u64::try_from(role.live_elf_bytes)
                .map_err(|_| Error::new("live ELF is too wide"))?
                .to_le_bytes(),
        );
        hash_field(&mut hasher, role.live_elf_sha256.as_bytes())?;
        hash_field(&mut hasher, role.body_file.as_bytes())?;
        hash_field(&mut hasher, role.body_path.as_bytes())?;
        hash_field(&mut hasher, body)?;
    }
    Ok(hex(&hasher.finalize()))
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) -> Result<()> {
    let width = u64::try_from(value.len()).map_err(|_| Error::new("capture field is too wide"))?;
    hasher.update(width.to_le_bytes());
    hasher.update(value);
    Ok(())
}

fn write_prepare_bundle_atomic(
    output_dir: &Path,
    manifest: &PrepareProgramdataManifestV1,
    bodies: &[Vec<u8>],
) -> Result<()> {
    if manifest.roles.len() != bodies.len() {
        return Err(Error::new("prepare ProgramData bundle width differs"));
    }
    fs::create_dir(output_dir).map_err(|error| {
        Error::new(format!(
            "could not create no-clobber output directory {}: {error}",
            output_dir.display()
        ))
    })?;
    for (role, body) in manifest.roles.iter().zip(bodies) {
        write_new_sync(&output_dir.join(&role.body_file), body)?;
    }
    // The manifest is the commit marker. A crash before this write leaves an
    // incomplete directory that no consumer can mistake for a complete bundle.
    let mut bytes = serde_json::to_vec_pretty(manifest)?;
    bytes.push(b'\n');
    write_new_sync(&output_dir.join(PREPARE_MANIFEST_FILE), &bytes)?;
    sync_directory(output_dir)?;
    if let Some(parent) = output_dir.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

pub(crate) fn write_json_atomic_new<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("capture output omitted a parent"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::new("capture output file name is not UTF-8"))?;
    let temporary = parent.join(format!(".{file_name}.{}.pending", std::process::id()));
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_new_sync(&temporary, &bytes)?;
    if let Err(error) = fs::hard_link(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(Error::new(format!(
            "could not publish no-clobber output {}: {error}",
            path.display()
        )));
    }
    sync_directory(parent)?;
    fs::remove_file(&temporary)?;
    sync_directory(parent)?;
    Ok(())
}

fn write_new_sync(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| Error::new(format!("could not create {}: {error}", path.display())))?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

fn require_public_devnet(origin: &ClusterOriginV1) -> Result<()> {
    if origin.url().trim_end_matches('/') != PUBLIC_DEVNET_ENDPOINT {
        return Err(Error::new(format!(
            "release capture requires the canonical public devnet endpoint {PUBLIC_DEVNET_ENDPOINT}"
        )));
    }
    Ok(())
}

fn require_new_file_path(path: &Path, label: &str) -> Result<()> {
    require_absolute_normalized(path, label)?;
    if fs::symlink_metadata(path).is_ok() {
        return Err(Error::new(format!(
            "{label} {} already exists",
            path.display()
        )));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(format!("{label} omitted a parent")))?;
    require_canonical_directory(parent, label)
}

fn require_new_directory_path(path: &Path, label: &str) -> Result<()> {
    require_new_file_path(path, label)
}

fn require_absolute_normalized(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(Error::new(format!(
            "{label} must be an absolute normalized path with no escape"
        )));
    }
    Ok(())
}

fn require_canonical_directory(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        Error::new(format!(
            "{label} parent {} cannot be inspected: {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(Error::new(format!(
            "{label} parent must be one real directory"
        )));
    }
    if fs::canonicalize(path)? != path {
        return Err(Error::new(format!("{label} parent is not canonical")));
    }
    Ok(())
}

fn absolute_path(value: &str, label: &str) -> Result<PathBuf> {
    let path = PathBuf::from(value);
    require_absolute_normalized(&path, label)?;
    Ok(path)
}

fn require_distinct_programs(programs: &[Pubkey]) -> Result<()> {
    let mut identities = std::collections::BTreeSet::new();
    for program in programs {
        if *program == Pubkey::default()
            || *program == bpf_loader_upgradeable::ID
            || !identities.insert(*program)
        {
            return Err(Error::new(
                "expected Programs must be distinct non-System, non-Loader identities",
            ));
        }
    }
    Ok(())
}

fn require_signing_authority(authority: Pubkey, programs: &[Pubkey]) -> Result<()> {
    if authority == Pubkey::default()
        || authority == bpf_loader_upgradeable::ID
        || programs
            .iter()
            .any(|program| authority == *program || authority == programdata(*program))
    {
        return Err(Error::new(
            "expected retained upgrade authority must be a distinct non-System signer identity",
        ));
    }
    Ok(())
}

fn parse_nonzero_u64(value: &str, label: &str) -> Result<u64> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| Error::new(format!("{label} must be a u64")))?;
    if parsed == 0 {
        return Err(Error::new(format!("{label} must be nonzero")));
    }
    Ok(parsed)
}

fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    Pubkey::from_str(value).map_err(|_| Error::new(format!("{label} is not a Solana pubkey")))
}

fn programdata(program: Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn raw_record(registry: Pubkey, artifact_id: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &artifact_id,
        ],
        &registry,
    )
    .0
}

fn staging_record(registry: Pubkey, artifact_id: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V1,
            &artifact_id,
        ],
        &registry,
    )
    .0
}

fn digest(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        use core::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn hex32(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(Error::new(
            "digest is not 64 lowercase hexadecimal characters",
        ));
    }
    let mut output = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = core::str::from_utf8(pair).map_err(|_| Error::new("digest is not UTF-8"))?;
        let byte = output
            .get_mut(index)
            .ok_or_else(|| Error::new("digest output index overflow"))?;
        *byte =
            u8::from_str_radix(text, 16).map_err(|_| Error::new("digest is not hexadecimal"))?;
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tests judge rent against the genesis-default rate, which is what
    /// their synthetic balances were minted at. Production never uses it: the
    /// live Rent sysvar of the observation's own finalized context does.
    fn fixture_rent() -> Rent {
        Rent::default()
    }

    /// Every account cohort-15 deployed, read off devnet, judged at a rate it
    /// was NOT funded at -- and admitted.
    ///
    /// This preflight gates a redeploy over accounts an earlier cohort created,
    /// so the direction that matters for safety is a rate that ROSE since the
    /// funding. The rate is a live chain fact and there is no direction a
    /// cluster is forbidden to move; the rate-of-the-moment floor this check
    /// used to be would refuse cohort-16's redeploy over a substrate the
    /// cluster itself considers alive, and nothing about the substrate would
    /// have changed. `solana-svm 4.3.0-beta.2` is the authority and it
    /// grandfathers (no rent-collection path; `get_pre_exec_account_rent_state`
    /// reads a rate-stranded account as `RentExempt` under SIMD-0392;
    /// `transition_allowed` refuses `RentExempt -> RentPaying`). The one case
    /// left is a DRAINED account, which is this test's hostile.
    ///
    /// THE POSITIVE CONTROL IS TWO-SIDED, over all fourteen accounts: each must
    /// be exempt at the rate it was funded at AND below the risen minimum. A
    /// rate that did not really move for these widths could not let this pass.
    #[test]
    fn a_risen_rate_admits_the_cohort_it_did_not_fund_and_a_drained_account_refuses() {
        // Read off devnet at finalized slot 493,000,156: cohort-15's seven
        // Program accounts and the seven ProgramData accounts the Loader
        // derived for them, exactly as `getMultipleAccounts` returned them.
        // `tools/cohort/cohorts/15.json` names the program ids.
        const COHORT_15_PROGRAMS_V1: [(&str, u64, usize); 7] = [
            ("registry", 1_038_612, 36),
            ("rent", 1_038_612, 36),
            ("custody", 1_038_612, 36),
            ("resolution", 1_038_612, 36),
            ("claims", 1_038_612, 36),
            ("trading", 1_038_612, 36),
            ("core", 1_038_612, 36),
        ];
        const COHORT_15_PROGRAMDATA_V1: [(&str, u64, usize); 7] = [
            ("registry", 1_523_802_129, 240_485),
            ("rent", 911_375_697, 143_781),
            ("custody", 3_638_365_497, 574_381),
            ("resolution", 5_256_776_313, 829_933),
            ("claims", 8_788_107_777, 1_387_541),
            ("trading", 14_828_523_177, 2_341_341),
            ("core", 7_558_897_809, 1_193_445),
        ];

        // The three rates one deployed cohort has now been priced at, and only
        // the first funded it.
        let funded = Rent {
            lamports_per_byte_year: 6_333,
            exemption_threshold: 1.0,
            burn_percent: 50,
        };
        // Devnet's live rate at slot 493,000,156. It FELL after the funding --
        // which is the direction `c0a1586b1` repaired for the exactness checks.
        let live_devnet = Rent {
            lamports_per_byte_year: 5_080,
            exemption_threshold: 1.0,
            burn_percent: 50,
        };
        // A RISEN rate, which is this test's subject. The genesis constant is
        // one: 3,480 over a 2.0-year threshold prices a byte at 6,960, above
        // the 6,333 this cohort was funded at.
        let risen = Rent::default();
        assert_eq!(funded.minimum_balance(36), 1_038_612);
        assert_eq!(live_devnet.minimum_balance(36), 833_120);
        assert_eq!(risen.minimum_balance(36), 1_141_440);

        let account = |bytes: usize, lamports: u64| RpcAccount {
            lamports,
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
            data: vec![0; bytes],
        };

        for (label, lamports, bytes) in COHORT_15_PROGRAMS_V1
            .iter()
            .chain(COHORT_15_PROGRAMDATA_V1.iter())
        {
            // Side one: the cluster really did fund it, at 6,333, exactly.
            assert_eq!(
                *lamports,
                funded.minimum_balance(*bytes),
                "{label}: cohort-15 was funded at one rate, at every width"
            );
            // Side two: the risen rate really did move for THIS width, so an
            // admission below cannot be a rate that failed to rise.
            assert!(
                *lamports < risen.minimum_balance(*bytes),
                "{label}: the risen rate must actually strand this account"
            );
            // And the account the runtime calls alive is admitted.
            require_funded_rent_persists(label, &account(*bytes, *lamports))
                .expect("a deployed account the cluster funded is alive at any later rate");
        }

        // THE HOSTILE, and the only case the runtime has not already decided: a
        // DRAINED account. Its data is residue an earlier instruction of this
        // transaction left behind and the runtime reaps at the end, and a
        // redeploy may not build on it.
        let drained = account(36, 0);
        let refusal = require_funded_rent_persists("Registry Program", &drained)
            .expect_err("a drained account is not a substrate to redeploy over")
            .to_string();
        assert!(
            refusal.contains("Registry Program") && refusal.contains("DRAINED"),
            "the refusal must name the account and what it found: {refusal}"
        );
        // One lamport is the whole difference, at any rate and any width.
        require_funded_rent_persists("Registry Program", &account(36, 1))
            .expect("one lamport is not residue");
        require_funded_rent_persists("Trading ProgramData", &account(2_341_341, 1))
            .expect("width is not the question either");

        // The FEE PAYER keeps a live-rate floor, because its balance falls:
        // `validate_fee_payer` charges the fee and then prices the result at
        // today's minimum, so a payer that ends below it is refused by the
        // runtime no matter what it was funded at.
        let payer = |lamports: u64| RpcAccount {
            lamports,
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        };
        require_fee_payer_rent_floor_v1(
            "fee payer",
            &payer(live_devnet.minimum_balance(0)),
            &live_devnet,
        )
        .expect("a payer at the live minimum clears the runtime's own floor");
        let thin = require_fee_payer_rent_floor_v1(
            "fee payer",
            &payer(live_devnet.minimum_balance(0) - 1),
            &live_devnet,
        )
        .expect_err("one lamport below the live minimum cannot pay a fee and stay exempt")
        .to_string();
        assert!(
            thin.contains("fee payer")
                && thin.contains(&live_devnet.minimum_balance(0).to_string()),
            "a threshold refusal must state what it wanted: {thin}"
        );
        // And the cohort-12 regression stays fixed: the genesis default is not
        // the rate to ask. A payer exempt on devnet today would be refused by
        // it, which cost a night and 0.2373 SOL of unnecessary top-ups.
        assert!(
            live_devnet.minimum_balance(0) < risen.minimum_balance(0),
            "the genesis default is not devnet's live rate"
        );
        require_fee_payer_rent_floor_v1(
            "fee payer",
            &payer(live_devnet.minimum_balance(0)),
            &risen,
        )
        .expect_err("reading the wrong rate is what this refuses to do in production");
    }

    /// The Rent sysvar body is authenticated, not trusted. The rate it quotes
    /// is still recorded in every carry-forward capture, so an offline
    /// re-validator can reproduce the deployment's own arithmetic.
    #[test]
    fn the_finalized_rent_sysvar_body_is_authenticated_not_trusted() {
        let live_devnet = Rent {
            lamports_per_byte_year: 5_080,
            exemption_threshold: 1.0,
            burn_percent: 50,
        };
        let canonical = bincode::serialize(&live_devnet).expect("Rent body");
        let sysvar_account = |owner: Pubkey, executable: bool, data: Vec<u8>| RpcAccount {
            lamports: 1,
            owner,
            executable,
            rent_epoch: 0,
            data,
        };
        assert_eq!(
            live_rent(&sysvar_account(sysvar::ID, false, canonical.clone())).expect("live rent"),
            live_devnet
        );
        assert!(
            live_rent(&sysvar_account(
                system_program::ID,
                false,
                canonical.clone()
            ))
            .expect_err("a foreign owner")
            .to_string()
            .contains("not an owned non-executable sysvar")
        );
        let mut trailing = canonical.clone();
        trailing.push(0);
        assert!(
            live_rent(&sysvar_account(sysvar::ID, false, trailing))
                .expect_err("a padded body")
                .to_string()
                .contains("exact canonical body")
        );
        let zero_rate = bincode::serialize(&Rent {
            lamports_per_byte_year: 0,
            exemption_threshold: 1.0,
            burn_percent: 50,
        })
        .expect("zero-rate body");
        assert!(
            live_rent(&sysvar_account(sysvar::ID, false, zero_rate))
                .expect_err("a zero rate would make everything exempt")
                .to_string()
                .contains("zero rate or a non-positive exemption threshold")
        );
        assert_eq!(
            carry_forward_rent_v1(&live_devnet).lamports_per_byte_year,
            5_080
        );
    }

    use crate::{cluster::DEVNET_GENESIS_HASH, rpc::parse_json_without_duplicate_keys_v1};

    /// One real seven-role Loader set, used ONLY as a fixture. See the identical
    /// note in `upgrade::tests`: decision 0012's amendment retired the
    /// production constant, and these ids survive because their ProgramData
    /// coordinates really are Loader-derived.
    const FIXTURE_TARGETS_V1: &[(&str, &str, &str)] = &[
        (
            "registry",
            "Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj",
            "ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz",
        ),
        (
            "rent",
            "DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3",
            "78MW6W4iPzBVLceAwTL51CtyLcpcFM2iGVMDbzZtUFmy",
        ),
        (
            "custody",
            "34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH",
            "EhB7hHJ7vsCW3nCeqbxbJrn5Jsi6gbqwpVhoLMPZ8ENf",
        ),
        (
            "resolution",
            "2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd",
            "2QFBQJdLBXAnJWTVK8KeeUtWZEFhQqqN2CbkrWjMjY6f",
        ),
        (
            "claims",
            "85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN",
            "4La2511ddSxUcAQfdhKvEeGEasih3TStbQWVFEQKd34j",
        ),
        (
            "trading",
            "5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk",
            "AE1cWbCvXedE23XH3otSxvDQ7xVx7WLNMYDc8y8rqkrn",
        ),
        (
            "core",
            "HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N",
            "AD6mb5SP6yqc5GFexf3xhpr1wKaZQhS7Hrt41iZhKxaN",
        ),
    ];
    use dclutch_core_contract::ContentId;
    use dclutch_registry::ArtifactUpgradePolicyV1;
    use dclutch_registry::release_set::{
        ArtifactReleaseIdV1, ExecutionRoleBindingV1, ProgramIdentityV1,
    };

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "dclutch-release-capture-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("time")
                    .as_nanos()
            ));
            fs::create_dir(&path).expect("temp directory");
            Self(fs::canonicalize(path).expect("canonical temp directory"))
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn acknowledged_args(
        extra: &[(&str, Pubkey)],
        output_flag: &str,
        output: &Path,
    ) -> Vec<String> {
        let mut arguments = vec![
            "--rpc-url".into(),
            format!("{PUBLIC_DEVNET_ENDPOINT}/"),
            DEVNET_ACKNOWLEDGMENT_FLAG.into(),
            DEVNET_GENESIS_HASH.into(),
        ];
        for (flag, value) in extra {
            arguments.push((*flag).into());
            arguments.push(value.to_string());
        }
        arguments.extend([
            "--expected-upgrade-authority".into(),
            Pubkey::new_unique().to_string(),
            "--minimum-context-slot".into(),
            "1".into(),
            output_flag.into(),
            output.display().to_string(),
        ]);
        arguments
    }

    fn devnet_origin() -> ClusterOriginV1 {
        ClusterOriginV1::parse(PUBLIC_DEVNET_ENDPOINT, Some(DEVNET_GENESIS_HASH))
            .expect("acknowledged devnet origin")
    }

    fn rpc_account(owner: Pubkey, executable: bool, data: Vec<u8>) -> RpcAccount {
        RpcAccount {
            lamports: Rent::default().minimum_balance(data.len()),
            owner,
            executable,
            rent_epoch: u64::MAX,
            data,
        }
    }

    fn loader_accounts(
        program: Pubkey,
        authority: Pubkey,
        deployment_slot: u64,
        elf: &[u8],
    ) -> (RpcAccount, RpcAccount) {
        let programdata = programdata(program);
        let mut program_body = vec![0_u8; 36];
        program_body
            .get_mut(..4)
            .expect("Program variant")
            .copy_from_slice(&2_u32.to_le_bytes());
        program_body
            .get_mut(4..)
            .expect("ProgramData link")
            .copy_from_slice(programdata.as_ref());

        let mut programdata_body = vec![0_u8; 45 + elf.len()];
        programdata_body
            .get_mut(..4)
            .expect("ProgramData variant")
            .copy_from_slice(&3_u32.to_le_bytes());
        programdata_body
            .get_mut(4..12)
            .expect("deployment slot")
            .copy_from_slice(&deployment_slot.to_le_bytes());
        *programdata_body.get_mut(12).expect("upgrade authority tag") = 1;
        programdata_body
            .get_mut(13..45)
            .expect("upgrade authority")
            .copy_from_slice(authority.as_ref());
        programdata_body
            .get_mut(45..)
            .expect("ELF tail")
            .copy_from_slice(elf);
        (
            rpc_account(bpf_loader_upgradeable::ID, true, program_body),
            rpc_account(bpf_loader_upgradeable::ID, false, programdata_body),
        )
    }

    fn prepare_fixture() -> (PrepareCaptureArgsV1, Vec<Pubkey>, Vec<Option<RpcAccount>>) {
        let programs = std::array::from_fn(|_| Pubkey::new_unique());
        let authority = Pubkey::new_unique();
        let output_dir = std::env::temp_dir().join("unused-release-capture-fixture");
        let args = PrepareCaptureArgsV1 {
            origin: devnet_origin(),
            programs,
            expected_upgrade_authority: authority,
            minimum_context_slot: 1,
            output_dir,
        };
        let mut addresses = Vec::with_capacity(10);
        let mut accounts = Vec::with_capacity(10);
        for (ordinal, program) in programs.into_iter().enumerate() {
            let elf = vec![u8::try_from(ordinal + 1).expect("ELF fill"); 64 + ordinal];
            let (program_account, programdata_account) = loader_accounts(
                program,
                authority,
                100 + u64::try_from(ordinal).expect("slot"),
                &elf,
            );
            addresses.extend([program, programdata(program)]);
            accounts.extend([Some(program_account), Some(programdata_account)]);
        }
        (args, addresses, accounts)
    }

    #[allow(clippy::type_complexity)]
    fn permanent_fixture() -> (
        PermanentSubstrateArgsV1,
        Vec<Pubkey>,
        Vec<Option<RpcAccount>>,
        Vec<(&'static str, Pubkey, Pubkey)>,
    ) {
        let targets = DevnetUpgradeTargetsV1::authenticate(
            FIXTURE_TARGETS_V1
                .iter()
                .map(|(role, program, programdata)| (*role, *program, *programdata)),
        )
        .expect("fixture targets");
        let programs = target_programs(&targets);
        let authority = Pubkey::new_unique();
        let payer = Pubkey::new_unique();
        let args = PermanentSubstrateArgsV1 {
            targets: targets.clone(),
            origin: devnet_origin(),
            expected_upgrade_authority: authority,
            fee_payer: payer,
            minimum_context_slot: 1,
            output_path: std::env::temp_dir().join("unused-permanent-substrate-fixture.json"),
        };
        let mut addresses = Vec::with_capacity(programs.len() * 2 + 1);
        let mut accounts = Vec::with_capacity(programs.len() * 2 + 1);
        for (ordinal, (_, program, expected_programdata)) in programs.iter().enumerate() {
            let elf = vec![u8::try_from(ordinal + 1).expect("ELF fill"); 80 + ordinal];
            let (program_account, programdata_account) = loader_accounts(
                *program,
                authority,
                900 + u64::try_from(ordinal).expect("slot"),
                &elf,
            );
            addresses.extend([*program, *expected_programdata]);
            accounts.extend([Some(program_account), Some(programdata_account)]);
        }
        addresses.push(payer);
        let mut payer_account = rpc_account(system_program::ID, false, Vec::new());
        payer_account.lamports = 42_185_584_146;
        accounts.push(Some(payer_account));
        (args, addresses, accounts, programs)
    }

    fn artifact_fixture(
        program: Pubkey,
        authority: Pubkey,
        deployment_slot: u64,
        semantic_fill: u8,
        elf_fill: u8,
    ) -> (RpcAccount, RpcAccount, RpcAccount, [u8; 32]) {
        let elf = vec![elf_fill; 96];
        let (program_account, programdata_account) =
            loader_accounts(program, authority, deployment_slot, &elf);
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program.to_bytes()).expect("Program identity"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("Loader identity"),
            programdata(program).to_bytes(),
            ContentId::new([semantic_fill; 32]).expect("semantic release"),
            digest_bytes(&elf),
            deployment_slot,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(authority.to_bytes()),
        )
        .expect("artifact release");
        let body = release.to_bytes().to_vec();
        let artifact_id = digest_bytes(&body);
        (
            program_account,
            programdata_account,
            rpc_account(program, false, body),
            artifact_id,
        )
    }

    fn carry_fixture() -> (
        CarryForwardArgsV1,
        [Pubkey; 5],
        Vec<Option<RpcAccount>>,
        InfrastructureCoordinatesV1,
        Vec<Option<RpcAccount>>,
    ) {
        let registry = Pubkey::new_unique();
        let rent = Pubkey::new_unique();
        let core = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let (registry_program, registry_programdata, registry_raw, registry_artifact_id) =
            artifact_fixture(registry, authority, 200, 31, 41);
        let (rent_program, rent_programdata, mut rent_raw, rent_artifact_id) =
            artifact_fixture(rent, authority, 201, 32, 42);
        // Registry is the record owner for both infrastructure artifacts.
        rent_raw.owner = registry;
        let profile = ProtocolInfrastructureProfileV1::new(
            ExecutionRoleBindingV1::new(
                ProgramIdentityV1::new(registry.to_bytes()).expect("Registry identity"),
                ArtifactReleaseIdV1::new(registry_artifact_id).expect("Registry artifact ID"),
            ),
            ExecutionRoleBindingV1::new(
                ProgramIdentityV1::new(rent.to_bytes()).expect("Rent identity"),
                ArtifactReleaseIdV1::new(rent_artifact_id).expect("Rent artifact ID"),
            ),
        )
        .expect("infrastructure profile")
        .to_bytes()
        .to_vec();
        let profile_address =
            Pubkey::find_program_address(&[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1], &core).0;
        let profile_account = rpc_account(core, false, profile);
        let discovery_addresses = [
            registry,
            programdata(registry),
            rent,
            programdata(rent),
            profile_address,
        ];
        let discovery_accounts = vec![
            Some(registry_program.clone()),
            Some(registry_programdata.clone()),
            Some(rent_program.clone()),
            Some(rent_programdata.clone()),
            Some(profile_account.clone()),
        ];
        let args = CarryForwardArgsV1 {
            origin: devnet_origin(),
            registry_program: registry,
            rent_program: rent,
            core_program: core,
            expected_upgrade_authority: authority,
            minimum_context_slot: 1,
            output_path: std::env::temp_dir().join("unused-carry-forward-fixture.json"),
        };
        let coordinates =
            discover_infrastructure_coordinates(&args, &discovery_addresses, &discovery_accounts)
                .expect("authenticated infrastructure discovery");
        let final_accounts = vec![
            Some(registry_program),
            Some(registry_programdata),
            Some(rent_program),
            Some(rent_programdata),
            Some(registry_raw),
            None,
            Some(rent_raw),
            None,
            Some(profile_account),
        ];
        (
            args,
            discovery_addresses,
            discovery_accounts,
            coordinates,
            final_accounts,
        )
    }

    #[test]
    fn cli_requires_exact_pairs_nonzero_floor_and_distinct_programs() {
        let temp = TempDir::new("cli");
        let registry = Pubkey::new_unique();
        let args = acknowledged_args(
            &[
                ("--expected-registry-program", registry),
                ("--expected-rent-program", registry),
                ("--expected-core-program", Pubkey::new_unique()),
            ],
            "--output",
            &temp.0.join("snapshot.json"),
        );
        assert!(
            parse_carry_forward_args(args).is_err(),
            "aliased Programs refuse"
        );

        let mut args = acknowledged_args(
            &[
                ("--expected-registry-program", Pubkey::new_unique()),
                ("--expected-rent-program", Pubkey::new_unique()),
                ("--expected-core-program", Pubkey::new_unique()),
            ],
            "--output",
            &temp.0.join("snapshot.json"),
        );
        let floor = args
            .iter()
            .position(|value| value == "--minimum-context-slot")
            .expect("floor");
        let value = args
            .get_mut(floor + 1)
            .expect("floor flag must carry a value");
        *value = "0".into();
        assert!(
            parse_carry_forward_args(args).is_err(),
            "zero floor refuses"
        );

        let mut args = acknowledged_args(
            &[
                ("--expected-registry-program", Pubkey::new_unique()),
                ("--expected-rent-program", Pubkey::new_unique()),
                ("--expected-core-program", Pubkey::new_unique()),
            ],
            "--output",
            &temp.0.join("snapshot.json"),
        );
        let authority = args
            .iter()
            .position(|value| value == "--expected-upgrade-authority")
            .expect("authority");
        *args
            .get_mut(authority + 1)
            .expect("authority flag must carry a value") = Pubkey::default().to_string();
        assert!(
            parse_carry_forward_args(args).is_err(),
            "System cannot be a retained signing authority"
        );
    }

    #[test]
    fn original_json_boundary_refuses_duplicate_and_trailing_values() {
        for hostile in [
            br#"{"slot":1,"slot":2}"#.as_slice(),
            br#"{"slot":1} {}"#.as_slice(),
        ] {
            assert!(parse_json_without_duplicate_keys_v1(hostile).is_err());
        }
    }

    #[test]
    fn prepare_capture_authenticates_all_five_loader_pairs_in_canonical_order() {
        let (args, addresses, accounts) = prepare_fixture();
        let expected_bodies: Vec<Vec<u8>> = accounts
            .chunks_exact(2)
            .map(|pair| {
                pair.get(1)
                    .and_then(Option::as_ref)
                    .expect("ProgramData account")
                    .data
                    .clone()
            })
            .collect();
        let (manifest, bodies) = authenticate_prepare_programdata(&args, 777, &addresses, accounts)
            .expect("authenticated capture");
        assert_eq!(manifest.context_slot, 777);
        assert_eq!(manifest.roles.len(), PREPARE_ROLES.len());
        assert_eq!(bodies, expected_bodies);
        for ((row, role), program) in manifest.roles.iter().zip(PREPARE_ROLES).zip(args.programs) {
            assert_eq!(row.role, role);
            assert_eq!(row.program_id, program.to_string());
            assert_eq!(row.programdata_id, programdata(program).to_string());
            assert_eq!(
                row.programdata_account_sha256,
                digest(
                    bodies
                        .get(usize::from(row.ordinal))
                        .expect("body for canonical ordinal")
                )
            );
        }
    }

    #[test]
    fn prepare_capture_refuses_missing_substituted_or_reordered_loader_evidence() {
        let (args, addresses, accounts) = prepare_fixture();

        let mut missing = accounts.clone();
        *missing.get_mut(3).expect("second ProgramData") = None;
        assert!(authenticate_prepare_programdata(&args, 1, &addresses, missing).is_err());

        let mut wrong_authority = accounts.clone();
        *wrong_authority
            .get_mut(1)
            .and_then(Option::as_mut)
            .and_then(|account| account.data.get_mut(13))
            .expect("first ProgramData authority") ^= 1;
        assert!(authenticate_prepare_programdata(&args, 1, &addresses, wrong_authority).is_err());

        let mut wrong_owner = accounts.clone();
        wrong_owner
            .get_mut(4)
            .and_then(Option::as_mut)
            .expect("third Program")
            .owner = Pubkey::new_unique();
        assert!(authenticate_prepare_programdata(&args, 1, &addresses, wrong_owner).is_err());

        let mut wrong_link = accounts.clone();
        wrong_link
            .get_mut(6)
            .and_then(Option::as_mut)
            .expect("fourth Program")
            .data
            .get_mut(4..)
            .expect("ProgramData link")
            .copy_from_slice(Pubkey::new_unique().as_ref());
        assert!(authenticate_prepare_programdata(&args, 1, &addresses, wrong_link).is_err());

        let mut reordered = addresses.clone();
        reordered.swap(0, 2);
        assert!(authenticate_prepare_programdata(&args, 1, &reordered, accounts).is_err());
    }

    #[test]
    fn permanent_snapshot_binds_seven_fixed_pairs_and_payer_in_one_context() {
        let (args, addresses, accounts, programs) = permanent_fixture();
        let expected_program_lamports = accounts
            .iter()
            .take(14)
            .step_by(2)
            .map(|account| account.as_ref().expect("Program").lamports)
            .sum::<u64>();
        let expected_programdata_lamports = accounts
            .iter()
            .skip(1)
            .take(14)
            .step_by(2)
            .map(|account| account.as_ref().expect("ProgramData").lamports)
            .sum::<u64>();
        let snapshot = authenticate_permanent_substrate(
            &args,
            1_001,
            &addresses,
            accounts,
            &programs,
            &fixture_rent(),
        )
        .expect("authenticated permanent snapshot");
        assert_eq!(snapshot.schema, PERMANENT_SUBSTRATE_SCHEMA);
        assert_eq!(snapshot.context_slot, 1_001);
        assert_eq!(snapshot.roles.len(), CHECKED_ROLE_ORDER_V1.len());
        assert_eq!(snapshot.fee_payer_lamports, 42_185_584_146);
        assert_eq!(snapshot.program_lamports_total, expected_program_lamports);
        assert_eq!(
            snapshot.programdata_lamports_total,
            expected_programdata_lamports
        );
        assert_eq!(
            snapshot.snapshot_sha256,
            permanent_substrate_digest(&snapshot).expect("canonical snapshot digest")
        );
        for ((row, expected_role), (_, expected_program, expected_programdata)) in snapshot
            .roles
            .iter()
            .zip(CHECKED_ROLE_ORDER_V1)
            .zip(programs)
        {
            assert_eq!(row.role, expected_role);
            assert_eq!(row.program_id, expected_program.to_string());
            assert_eq!(row.programdata_id, expected_programdata.to_string());
        }
    }

    #[test]
    fn permanent_snapshot_refuses_every_identity_authority_and_payer_substitution() {
        let (args, addresses, accounts, programs) = permanent_fixture();

        let mut reordered = addresses.clone();
        reordered.swap(0, 2);
        assert!(
            authenticate_permanent_substrate(
                &args,
                10,
                &reordered,
                accounts.clone(),
                &programs,
                &fixture_rent()
            )
            .is_err()
        );

        let mut wrong_authority = accounts.clone();
        *wrong_authority
            .get_mut(3)
            .and_then(Option::as_mut)
            .and_then(|account| account.data.get_mut(13))
            .expect("second ProgramData authority") ^= 1;
        assert!(
            authenticate_permanent_substrate(
                &args,
                10,
                &addresses,
                wrong_authority,
                &programs,
                &fixture_rent()
            )
            .is_err()
        );

        let mut wrong_program_owner = accounts.clone();
        wrong_program_owner
            .get_mut(8)
            .and_then(Option::as_mut)
            .expect("fifth Program")
            .owner = system_program::ID;
        assert!(
            authenticate_permanent_substrate(
                &args,
                10,
                &addresses,
                wrong_program_owner,
                &programs,
                &fixture_rent()
            )
            .is_err()
        );

        let mut absent_payer = accounts.clone();
        *absent_payer.last_mut().expect("payer row") = None;
        assert!(
            authenticate_permanent_substrate(
                &args,
                10,
                &addresses,
                absent_payer,
                &programs,
                &fixture_rent()
            )
            .is_err()
        );

        let mut executable_payer = accounts;
        executable_payer
            .last_mut()
            .and_then(Option::as_mut)
            .expect("payer")
            .executable = true;
        assert!(
            authenticate_permanent_substrate(
                &args,
                10,
                &addresses,
                executable_payer,
                &programs,
                &fixture_rent()
            )
            .is_err()
        );
    }

    /// Decision 0012's amendment at the CLI: the capture stays key-free, and the
    /// seven roles become a REQUIRED, authenticated, caller-declared input. The
    /// test this replaces asserted the opposite -- that a `--expected-core-program`
    /// flag must be rejected -- which is precisely the property that made the
    /// whole checked-upgrade lineage reachable by one closed substrate.
    #[test]
    fn permanent_capture_cli_requires_and_authenticates_a_declared_seven_role_set() {
        let temp = TempDir::new("permanent-cli");
        let authority = Pubkey::new_unique();
        let base = |programs: &[(&str, String)]| {
            let mut args = vec![
                "--rpc-url".to_owned(),
                PUBLIC_DEVNET_ENDPOINT.to_owned(),
                DEVNET_ACKNOWLEDGMENT_FLAG.to_owned(),
                DEVNET_GENESIS_HASH.to_owned(),
                "--expected-upgrade-authority".to_owned(),
                authority.to_string(),
                "--fee-payer".to_owned(),
                authority.to_string(),
                "--minimum-context-slot".to_owned(),
                "1".to_owned(),
                "--output".to_owned(),
                temp.0.join("snapshot.json").display().to_string(),
            ];
            for (role, program) in programs {
                args.push(format!("--expected-{role}-program"));
                args.push(program.clone());
            }
            args
        };
        let declared = CHECKED_ROLE_ORDER_V1
            .iter()
            .map(|role| (*role, Pubkey::new_unique().to_string()))
            .collect::<Vec<_>>();

        let parsed =
            parse_permanent_substrate_args(base(&declared)).expect("declared seven-role set");
        assert_eq!(parsed.expected_upgrade_authority, authority);
        assert_eq!(parsed.fee_payer, authority);
        assert_eq!(parsed.targets.len(), CHECKED_ROLE_ORDER_V1.len());
        for (index, target) in parsed.targets.iter().enumerate() {
            assert_eq!(target.role, CHECKED_ROLE_ORDER_V1[index]);
            assert_eq!(target.program.to_string(), declared[index].1);
            // Never accepted from the caller: derived.
            assert_eq!(target.programdata, programdata(target.program));
        }

        // A missing role is a missing set, not a defaulted one.
        let short = base(&declared[..6]);
        assert!(
            parse_permanent_substrate_args(short)
                .expect_err("six roles")
                .to_string()
                .contains("--expected-core-program"),
            "the refusal must name the role flag that was absent"
        );

        // Two roles may not name one Program.
        let mut aliased = declared.clone();
        aliased[5].1 = aliased[4].1.clone();
        assert!(
            parse_permanent_substrate_args(base(&aliased))
                .expect_err("an aliased Program")
                .to_string()
                .contains("repeats an earlier target account")
        );

        // A signing identity may not also be an observed Program.
        let mut payer_is_a_program = declared.clone();
        payer_is_a_program[3].1 = authority.to_string();
        assert!(parse_permanent_substrate_args(base(&payer_is_a_program)).is_err());

        // Still key-free: no keypair flag exists on this command at all.
        let mut keypair = base(&declared);
        keypair.extend(["--authority-keypair".to_owned(), "/dev/null".to_owned()]);
        assert!(
            parse_permanent_substrate_args(keypair)
                .expect_err("a keypair flag")
                .to_string()
                .contains("--authority-keypair")
        );
    }

    #[test]
    fn carry_capture_discovers_profile_coordinates_then_authenticates_exact_final_context() {
        let (args, _, _, coordinates, accounts) = carry_fixture();
        let snapshot = authenticate_carry_forward_snapshot(
            &args,
            900,
            accounts,
            &coordinates,
            &fixture_rent(),
        )
        .expect("authenticated final context");
        assert_eq!(snapshot.context_slot, 900);
        assert_eq!(snapshot.accounts.len(), INFRASTRUCTURE_LABELS.len());
        for ((row, label), address) in snapshot
            .accounts
            .iter()
            .zip(INFRASTRUCTURE_LABELS)
            .zip(coordinates.addresses)
        {
            assert_eq!(row.role, label);
            assert_eq!(row.address, address.to_string());
        }
        assert!(
            snapshot
                .accounts
                .get(5)
                .expect("Registry staging")
                .account
                .is_none()
        );
        assert!(
            snapshot
                .accounts
                .get(7)
                .expect("Rent staging")
                .account
                .is_none()
        );
    }

    #[test]
    fn carry_capture_refuses_changed_profile_present_staging_and_substituted_release() {
        let (args, _, _, coordinates, accounts) = carry_fixture();

        let mut moved_profile = accounts.clone();
        *moved_profile
            .get_mut(8)
            .and_then(Option::as_mut)
            .and_then(|account| account.data.last_mut())
            .expect("profile tail") ^= 1;
        assert!(
            authenticate_carry_forward_snapshot(
                &args,
                1,
                moved_profile,
                &coordinates,
                &fixture_rent()
            )
            .is_err()
        );

        let mut live_staging = accounts.clone();
        *live_staging.get_mut(5).expect("Registry staging") =
            Some(rpc_account(args.registry_program, false, vec![1]));
        assert!(
            authenticate_carry_forward_snapshot(
                &args,
                1,
                live_staging,
                &coordinates,
                &fixture_rent()
            )
            .is_err()
        );

        let mut substituted_release = accounts;
        *substituted_release
            .get_mut(4)
            .and_then(Option::as_mut)
            .and_then(|account| account.data.last_mut())
            .expect("Registry release tail") ^= 1;
        assert!(
            authenticate_carry_forward_snapshot(
                &args,
                1,
                substituted_release,
                &coordinates,
                &fixture_rent()
            )
            .is_err()
        );
    }

    #[test]
    fn atomic_json_publication_never_clobbers() {
        let temp = TempDir::new("json");
        let output = temp.0.join("snapshot.json");
        let value = serde_json::json!({"schema":"first"});
        write_json_atomic_new(&output, &value).expect("first publication");
        let before = fs::read(&output).expect("first bytes");
        assert!(write_json_atomic_new(&output, &serde_json::json!({"schema":"second"})).is_err());
        assert_eq!(fs::read(output).expect("preserved bytes"), before);
    }

    #[test]
    fn prepare_bundle_commits_manifest_last_and_refuses_existing_directory() {
        let temp = TempDir::new("bundle");
        let output = temp.0.join("capture");
        let authority = Pubkey::new_unique();
        let mut roles = Vec::new();
        let mut bodies = Vec::new();
        for (ordinal, role) in PREPARE_ROLES.iter().enumerate() {
            let body = vec![u8::try_from(ordinal + 1).expect("byte"); 48];
            let program = Pubkey::new_unique();
            let programdata = programdata(program);
            let body_file = format!("{ordinal:02}-{role}-programdata.bin");
            roles.push(PrepareProgramdataRoleV1 {
                ordinal: u8::try_from(ordinal).expect("ordinal"),
                role: (*role).into(),
                program_id: program.to_string(),
                programdata_id: programdata.to_string(),
                deployment_slot: 10 + u64::try_from(ordinal).expect("slot"),
                program_account_data_sha256: digest(&[0; 36]),
                programdata_account_bytes: body.len(),
                programdata_account_sha256: digest(&body),
                live_elf_bytes: 3,
                live_elf_sha256: digest(body.get(45..).expect("ProgramData ELF tail")),
                body_file: body_file.clone(),
                body_path: output.join(&body_file).display().to_string(),
            });
            bodies.push(body);
        }
        let mut manifest = PrepareProgramdataManifestV1 {
            schema: PREPARE_BUNDLE_SCHEMA.into(),
            endpoint: PUBLIC_DEVNET_ENDPOINT.into(),
            commitment: "finalized".into(),
            rpc_method: "getMultipleAccounts".into(),
            context_slot: 99,
            expected_upgrade_authority: authority.to_string(),
            canonical_role_order: PREPARE_ROLES.iter().map(|role| (*role).into()).collect(),
            roles,
            bundle_sha256: String::new(),
        };
        let bundle_digest = prepare_bundle_digest(&manifest, &bodies).expect("digest");
        let mut substituted_manifest = manifest.clone();
        substituted_manifest
            .roles
            .first_mut()
            .expect("first role")
            .program_account_data_sha256 = digest(b"substituted Program body");
        assert_ne!(
            prepare_bundle_digest(&substituted_manifest, &bodies).expect("substituted digest"),
            bundle_digest
        );
        let mut substituted_bodies = bodies.clone();
        substituted_bodies
            .first_mut()
            .and_then(|body| body.last_mut())
            .map(|byte| *byte ^= 1)
            .expect("first body tail");
        assert_ne!(
            prepare_bundle_digest(&manifest, &substituted_bodies).expect("body digest"),
            bundle_digest
        );
        manifest.bundle_sha256 = bundle_digest;
        write_prepare_bundle_atomic(&output, &manifest, &bodies).expect("bundle");
        assert!(output.join(PREPARE_MANIFEST_FILE).is_file());
        assert!(write_prepare_bundle_atomic(&output, &manifest, &bodies).is_err());
        for (role, body) in manifest.roles.iter().zip(bodies) {
            assert_eq!(fs::read(output.join(&role.body_file)).expect("body"), body);
        }
    }
}
