#![forbid(unsafe_code)]

//! Command-line entry point for the offline checked-release verifier.

use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use dclutch_vm::capability_seal::CAPABILITY_SEAL_BYTES_V1;
use dclutch_registry::release_set::{
    ExecutionReleaseSetV1, ProtocolInfrastructureProfileV1, ProtocolInfrastructureProfileV2,
};
use dclutch_release_tool::{
    BuildMetadataV1, CHECKED_TRANSLATION_VALIDATION_INPUT_COUNT_V1,
    CHECKED_TRANSLATION_VALIDATION_LABELS_V1, CheckedCapabilityExecutionV1,
    CheckedExecutionReleaseSetV1, CheckedGenesisInfrastructureV1, CheckedInfrastructureV1,
    CheckedReleaseV1, CheckedTranslationValidationV1, LoaderV3AuthorityStateV1, ReleaseEvidenceV1,
    SealAccountDumpV1, TranslationValidationEvidenceV1,
    build_checked_capability_execution_from_bytes_v1, build_checked_execution_release_set,
    build_checked_genesis_infrastructure_v1, build_checked_infrastructure_v1,
    build_checked_release, build_checked_translation_validation, derive_execution_release_set,
    derive_protocol_infrastructure_profile_v1, derive_protocol_infrastructure_profile_v2,
    loader_v3_program_account_data_v1, loader_v3_programdata_account_data_v1,
    loader_v3_programdata_address_v1, probe_defunct_seal_v1,
    verify_checked_capability_execution_v1, verify_checked_execution_release_set,
    verify_checked_genesis_infrastructure_v1, verify_checked_infrastructure_v1,
    verify_checked_release, verify_checked_translation_validation,
};
use solana_program::pubkey::Pubkey;

const USAGE: &str = "usage:\n  dclutch-release-tool create --elf PATH --semantic-preimage PATH --metadata PATH --program-account-data PATH --programdata-account-data PATH --out PATH [--text-out PATH]\n  dclutch-release-tool verify --manifest PATH --elf PATH --semantic-preimage PATH --metadata PATH --program-account-data PATH --programdata-account-data PATH [--text-out PATH]\n  dclutch-release-tool inspect --manifest PATH [--text-out PATH]\n  dclutch-release-tool loader-accounts --program-id HEX32 --loader-program-id HEX32 --elf PATH --deployment-slot U64 [--upgrade-authority HEX32 | --revoked-authority HEX32] --program-out PATH --programdata-out PATH [--text-out PATH]\n  dclutch-release-tool derive-set --core PATH --claims PATH --trading PATH --resolution PATH --custody PATH --out PATH\n  dclutch-release-tool derive-infrastructure-profile --registry PATH --rent PATH --predecessor-profile PATH --out PATH\n  dclutch-release-tool derive-genesis-infrastructure-profile --registry PATH --rent PATH --out PATH --v2-out PATH\n  dclutch-release-tool create-set --release-set PATH --core PATH --claims PATH --trading PATH --resolution PATH --custody PATH --out PATH [--text-out PATH]\n  dclutch-release-tool verify-set --manifest PATH --core PATH --claims PATH --trading PATH --resolution PATH --custody PATH [--text-out PATH]\n  dclutch-release-tool inspect-set --manifest PATH [--text-out PATH]\n  dclutch-release-tool create-infrastructure [--genesis] --execution PATH --profile PATH --core PATH --claims PATH --trading PATH --resolution PATH --custody PATH --registry PATH --rent PATH --out PATH [--text-out PATH]\n  dclutch-release-tool verify-infrastructure --manifest PATH --execution PATH --core PATH --claims PATH --trading PATH --resolution PATH --custody PATH --registry PATH --rent PATH [--text-out PATH]\n  dclutch-release-tool inspect-infrastructure --manifest PATH [--text-out PATH]\n  dclutch-release-tool create-capability-execution --descriptor PATH --strategy PATH --certificate PATH [--admission PATH] --accelerator PATH --out PATH [--text-out PATH]\n  dclutch-release-tool verify-capability-execution --manifest PATH --accelerator PATH [--text-out PATH]\n  dclutch-release-tool inspect-capability-execution --manifest PATH [--text-out PATH]\n  dclutch-release-tool create-translation --evidence-dir PATH --out PATH [--text-out PATH]\n  dclutch-release-tool verify-translation --manifest PATH --evidence-dir PATH [--text-out PATH]\n  dclutch-release-tool inspect-translation --manifest PATH [--text-out PATH]\n  dclutch-release-tool seal-probe --account PATH --live-release ID32 [--address ID32] [--program-id ID32] [--text-out PATH]\n\nID32 is 64 hexadecimal digits or a base58 32-byte identity.";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "dclutch-release-tool: {error}\n{USAGE}"
            );
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args_os();
    let _program = arguments.next();
    let command = arguments
        .next()
        .ok_or_else(|| "missing command".to_owned())?
        .into_string()
        .map_err(|_| "command must be UTF-8".to_owned())?;
    let mut flags = parse_flags(arguments)?;
    match command.as_str() {
        "create" => create(&mut flags),
        "verify" => verify(&mut flags),
        "inspect" => inspect(&mut flags),
        "loader-accounts" => loader_accounts(&mut flags),
        "derive-set" => derive_set(&mut flags),
        "derive-infrastructure-profile" => derive_infrastructure_profile(&mut flags),
        "derive-genesis-infrastructure-profile" => {
            derive_genesis_infrastructure_profile(&mut flags)
        }
        "create-set" => create_set(&mut flags),
        "verify-set" => verify_set(&mut flags),
        "inspect-set" => inspect_set(&mut flags),
        "create-infrastructure" => create_infrastructure(&mut flags),
        "verify-infrastructure" => verify_infrastructure(&mut flags),
        "inspect-infrastructure" => inspect_infrastructure(&mut flags),
        "create-capability-execution" => create_capability_execution(&mut flags),
        "verify-capability-execution" => verify_capability_execution(&mut flags),
        "inspect-capability-execution" => inspect_capability_execution(&mut flags),
        "create-translation" => create_translation(&mut flags),
        "verify-translation" => verify_translation(&mut flags),
        "inspect-translation" => inspect_translation(&mut flags),
        "seal-probe" => seal_probe(&mut flags),
        _ => Err(format!("unknown command: {command}")),
    }
}

fn create_translation(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let output = required(flags, "--out")?;
    let text_output = flags.remove("--text-out");
    let evidence = TranslationEvidenceFiles::load(required(flags, "--evidence-dir")?)?;
    require_no_flags(flags)?;
    let result =
        build_checked_translation_validation(evidence.evidence()).map_err(format_release_error)?;
    fs::write(&output, result.encode())
        .map_err(|error| format!("failed writing {}: {error}", output.display()))?;
    emit_translation_text(result, text_output)
}

fn verify_translation(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest = read_bytes(required(flags, "--manifest")?)?;
    let text_output = flags.remove("--text-out");
    let evidence = TranslationEvidenceFiles::load(required(flags, "--evidence-dir")?)?;
    require_no_flags(flags)?;
    let result = verify_checked_translation_validation(&manifest, evidence.evidence())
        .map_err(format_release_error)?;
    emit_translation_text(result, text_output)
}

fn inspect_translation(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest = read_bytes(required(flags, "--manifest")?)?;
    let text_output = flags.remove("--text-out");
    require_no_flags(flags)?;
    let result = CheckedTranslationValidationV1::decode(&manifest).map_err(format_release_error)?;
    emit_translation_text(result, text_output)
}

fn emit_translation_text(
    translation: CheckedTranslationValidationV1,
    output: Option<PathBuf>,
) -> Result<(), String> {
    let text = translation.render_text().map_err(format_release_error)?;
    if let Some(path) = output {
        fs::write(&path, text)
            .map_err(|error| format!("failed writing {}: {error}", path.display()))
    } else {
        io::stdout()
            .lock()
            .write_all(text.as_bytes())
            .map_err(|error| format!("failed writing stdout: {error}"))
    }
}

fn create_infrastructure(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let output = required(flags, "--out")?;
    let text_output = flags.remove("--text-out");
    let execution_manifest = read_bytes(required(flags, "--execution")?)?;
    let profile_bytes = read_bytes(required(flags, "--profile")?)?;
    // Which act this manifest describes is a STATED choice, exactly as it is
    // for the profile derivation, and for the same reason: the two commit
    // different profile versions to different chain acts. The flag and the
    // profile width must agree, and disagreeing either way refuses by name --
    // so a mistyped pipeline cannot quietly emit the wrong manifest kind.
    let genesis = flags.remove("--genesis").is_some();
    let manifests = CheckedManifestFiles::load(flags)?;
    let registry = load_checked_release(required(flags, "--registry")?)?;
    let rent = load_checked_release(required(flags, "--rent")?)?;
    require_no_flags(flags)?;

    let execution = verify_checked_execution_release_set(&execution_manifest, manifests.refs())
        .map_err(format_release_error)?;
    let checked = manifests.decode()?;
    if genesis {
        let profile = ProtocolInfrastructureProfileV1::decode(&profile_bytes).map_err(|error| {
            format!(
                "genesis infrastructure profile refused: {error:?}. --genesis expects the \
                 write-once V1 profile a cohort that succeeds nothing commits; a succession \
                 profile belongs to this command without --genesis"
            )
        })?;
        let result = build_checked_genesis_infrastructure_v1(
            execution,
            profile,
            &checked[0],
            &registry,
            &rent,
        )
        .map_err(format_release_error)?;
        fs::write(&output, result.encode())
            .map_err(|error| format!("failed writing {}: {error}", output.display()))?;
        return emit_genesis_infrastructure_text(result, text_output);
    }
    let profile = ProtocolInfrastructureProfileV2::decode(&profile_bytes).map_err(|error| {
        format!(
            "infrastructure profile refused: {error:?}. This command builds a SUCCESSION \
             manifest and expects the 224-byte V2 profile; for a cohort that succeeds nothing, \
             pass --genesis and the 144-byte V1 profile"
        )
    })?;
    let result = build_checked_infrastructure_v1(execution, profile, &checked[0], &registry, &rent)
        .map_err(format_release_error)?;
    fs::write(&output, result.encode())
        .map_err(|error| format!("failed writing {}: {error}", output.display()))?;
    emit_infrastructure_text(result, text_output)
}

fn emit_genesis_infrastructure_text(
    infrastructure: CheckedGenesisInfrastructureV1,
    output: Option<PathBuf>,
) -> Result<(), String> {
    let text = infrastructure.render_text().map_err(format_release_error)?;
    if let Some(path) = output {
        fs::write(&path, text)
            .map_err(|error| format!("failed writing {}: {error}", path.display()))
    } else {
        io::stdout()
            .lock()
            .write_all(text.as_bytes())
            .map_err(|error| format!("failed writing stdout: {error}"))
    }
}

fn verify_infrastructure(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest = read_bytes(required(flags, "--manifest")?)?;
    let execution = read_bytes(required(flags, "--execution")?)?;
    let text_output = flags.remove("--text-out");
    let manifests = CheckedManifestFiles::load(flags)?;
    let registry = read_bytes(required(flags, "--registry")?)?;
    let rent = read_bytes(required(flags, "--rent")?)?;
    require_no_flags(flags)?;

    // A manifest declares its own shape: the header's schema field and the
    // exact width distinguish a genesis from a succession, and each decoder
    // refuses the other by name rather than misreading it. So this reads the
    // bytes instead of asking the caller to restate what they already say.
    refuse_retired_genesis_manifest(manifest.len())?;
    if manifest.len() == dclutch_release_tool::CHECKED_GENESIS_INFRASTRUCTURE_BYTES_V1 {
        let result = verify_checked_genesis_infrastructure_v1(
            &manifest,
            &execution,
            manifests.refs(),
            &registry,
            &rent,
        )
        .map_err(format_release_error)?;
        return emit_genesis_infrastructure_text(result, text_output);
    }
    let result =
        verify_checked_infrastructure_v1(&manifest, &execution, manifests.refs(), &registry, &rent)
            .map_err(format_release_error)?;
    emit_infrastructure_text(result, text_output)
}

fn inspect_infrastructure(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest = read_bytes(required(flags, "--manifest")?)?;
    let text_output = flags.remove("--text-out");
    require_no_flags(flags)?;
    refuse_retired_genesis_manifest(manifest.len())?;
    if manifest.len() == dclutch_release_tool::CHECKED_GENESIS_INFRASTRUCTURE_BYTES_V1 {
        let result =
            CheckedGenesisInfrastructureV1::decode(&manifest).map_err(format_release_error)?;
        return emit_genesis_infrastructure_text(result, text_output);
    }
    let result = CheckedInfrastructureV1::decode(&manifest).map_err(format_release_error)?;
    emit_infrastructure_text(result, text_output)
}

/// Refuse a schema-3 genesis manifest by name rather than by misreading it.
///
/// Schema 3 pinned only the 144-byte V1. Since `c60b25e8` initialization
/// commits both profiles in one instruction, so those bytes describe half a
/// chain act; without this the retired width falls through to the succession
/// decoder and dies on `InvalidLength`, which names nothing.
fn refuse_retired_genesis_manifest(len: usize) -> Result<(), String> {
    if len == dclutch_release_tool::CHECKED_GENESIS_INFRASTRUCTURE_BYTES_RETIRED_V3 {
        return Err(
            "this is a retired schema-3 genesis infrastructure manifest: it pins only the              144-byte V1 profile, and initialization now commits the genesis V2 in the same              instruction. Rebuild the candidate to emit a schema-4 manifest carrying both              profiles."
                .into(),
        );
    }
    Ok(())
}

fn emit_infrastructure_text(
    infrastructure: CheckedInfrastructureV1,
    output: Option<PathBuf>,
) -> Result<(), String> {
    let text = infrastructure.render_text().map_err(format_release_error)?;
    if let Some(path) = output {
        fs::write(&path, text)
            .map_err(|error| format!("failed writing {}: {error}", path.display()))
    } else {
        io::stdout()
            .lock()
            .write_all(text.as_bytes())
            .map_err(|error| format!("failed writing stdout: {error}"))
    }
}

fn create_capability_execution(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let output = required(flags, "--out")?;
    let text_output = flags.remove("--text-out");
    let descriptor = read_bytes(required(flags, "--descriptor")?)?;
    let strategy = read_bytes(required(flags, "--strategy")?)?;
    let certificate = read_bytes(required(flags, "--certificate")?)?;
    let admission = flags.remove("--admission").map(read_bytes).transpose()?;
    let accelerator = read_bytes(required(flags, "--accelerator")?)?;
    require_no_flags(flags)?;
    let result = build_checked_capability_execution_from_bytes_v1(
        &descriptor,
        &strategy,
        &certificate,
        admission.as_deref(),
        &accelerator,
    )
    .map_err(format_release_error)?;
    fs::write(&output, result.encode())
        .map_err(|error| format!("failed writing {}: {error}", output.display()))?;
    emit_capability_execution_text(result, text_output)
}

fn verify_capability_execution(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest = read_bytes(required(flags, "--manifest")?)?;
    let accelerator = read_bytes(required(flags, "--accelerator")?)?;
    let text_output = flags.remove("--text-out");
    require_no_flags(flags)?;
    let result = verify_checked_capability_execution_v1(&manifest, &accelerator)
        .map_err(format_release_error)?;
    emit_capability_execution_text(result, text_output)
}

fn inspect_capability_execution(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest = read_bytes(required(flags, "--manifest")?)?;
    let text_output = flags.remove("--text-out");
    require_no_flags(flags)?;
    let result = CheckedCapabilityExecutionV1::decode(&manifest).map_err(format_release_error)?;
    emit_capability_execution_text(result, text_output)
}

fn emit_capability_execution_text(
    execution: CheckedCapabilityExecutionV1,
    output: Option<PathBuf>,
) -> Result<(), String> {
    let text = execution.render_text().map_err(format_release_error)?;
    if let Some(path) = output {
        fs::write(&path, text)
            .map_err(|error| format!("failed writing {}: {error}", path.display()))
    } else {
        io::stdout()
            .lock()
            .write_all(text.as_bytes())
            .map_err(|error| format!("failed writing stdout: {error}"))
    }
}

fn loader_accounts(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let program_id = required_hex32(flags, "--program-id")?;
    let loader_program_id = required_hex32(flags, "--loader-program-id")?;
    let elf = read_bytes(required(flags, "--elf")?)?;
    let deployment_slot = required_u64(flags, "--deployment-slot")?;
    let upgrade_authority = optional_hex32(flags, "--upgrade-authority")?;
    // The third state. A program that was deployed mutable and then revoked to
    // `--final` keeps its former authority at [13..45] behind a zero tag
    // forever, and that is the ONLY shape a real devnet role can be in. An
    // offline construction could not express it, so every checked manifest
    // built for a real deployment carried a programdata_account_sha256 no
    // account could match: DEVNET_DEMO_DEPLOY.md section 7 blocker B.
    //
    // It is a separate flag rather than a mode on --upgrade-authority because
    // the two say opposite things about who can upgrade the program, and a
    // caller must not be able to reach the wrong one by forgetting a boolean.
    let revoked_authority = optional_hex32(flags, "--revoked-authority")?;
    let program_out = required(flags, "--program-out")?;
    let programdata_out = required(flags, "--programdata-out")?;
    let text_output = flags.remove("--text-out");
    require_no_flags(flags)?;
    let authority = match (upgrade_authority, revoked_authority) {
        (Some(_), Some(_)) => {
            return Err(
                "--upgrade-authority and --revoked-authority are mutually exclusive: a program \
                 either can be upgraded by that key or used to be and no longer can"
                    .to_owned(),
            );
        }
        (Some(key), None) => LoaderV3AuthorityStateV1::Upgradeable(key),
        (None, Some(key)) => LoaderV3AuthorityStateV1::Revoked(key),
        (None, None) => LoaderV3AuthorityStateV1::NeverAuthorized,
    };

    let programdata_id = loader_v3_programdata_address_v1(&program_id, &loader_program_id);
    let program =
        loader_v3_program_account_data_v1(&programdata_id).map_err(format_release_error)?;
    let programdata = loader_v3_programdata_account_data_v1(&elf, deployment_slot, authority)
        .map_err(format_release_error)?;
    fs::write(&program_out, program)
        .map_err(|error| format!("failed writing {}: {error}", program_out.display()))?;
    fs::write(&programdata_out, &programdata)
        .map_err(|error| format!("failed writing {}: {error}", programdata_out.display()))?;

    let mut text = String::new();
    push_line(&mut text, "format", "dclutch-loader-v3-accounts-v1");
    push_line(&mut text, "program_id", &hex(&program_id));
    push_line(&mut text, "programdata_id", &hex(&programdata_id));
    push_line(&mut text, "loader_program_id", &hex(&loader_program_id));
    push_line(&mut text, "deployment_slot", &deployment_slot.to_string());
    push_line(
        &mut text,
        "upgrade_authority",
        &upgrade_authority.map_or_else(|| "none".to_owned(), |value| hex(&value)),
    );
    // APPENDED, never inserted: tools/release/checked-release-candidate.sh
    // scrapes this projection line by line with `sed -n 's/^key=//p'`.
    push_line(
        &mut text,
        "retained_revoked_authority",
        &revoked_authority.map_or_else(|| "none".to_owned(), |value| hex(&value)),
    );
    push_line(
        &mut text,
        "program_account_bytes",
        &program.len().to_string(),
    );
    push_line(
        &mut text,
        "programdata_account_bytes",
        &programdata.len().to_string(),
    );
    push_line(&mut text, "elf_bytes", &elf.len().to_string());
    push_line(
        &mut text,
        "evidence_class",
        // A retained revoked authority cannot be predicted -- nothing offline
        // knows which key a program used to have -- so a run that carries one
        // is quoting an observation and must not claim otherwise.
        if revoked_authority.is_some() {
            "loader-state-carrying-an-observed-retained-authority"
        } else {
            "predicted-loader-state-not-observed"
        },
    );
    write_text(text, text_output)
}

fn derive_set(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let output = required(flags, "--out")?;
    let manifests = CheckedManifestFiles::load(flags)?;
    require_no_flags(flags)?;
    let checked = manifests.decode()?;
    let release_set = derive_execution_release_set([
        &checked[0],
        &checked[1],
        &checked[2],
        &checked[3],
        &checked[4],
    ])
    .map_err(format_release_error)?;
    fs::write(&output, release_set.to_bytes())
        .map_err(|error| format!("failed writing {}: {error}", output.display()))
}

fn derive_infrastructure_profile(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let output = required(flags, "--out")?;
    let registry = load_checked_release(required(flags, "--registry")?)?;
    let rent = load_checked_release(required(flags, "--rent")?)?;
    // The predecessor account is a chain fact, not a function of the successor
    // manifests, so it is supplied rather than derived. It stays required on
    // THIS command because a succession profile is the only one a succeeding
    // cohort's consumers read: a succession command that could silently fall
    // back to the predecessor's own shape would hand the pipeline bytes the
    // chain has stopped answering to. A cohort that succeeds nothing is a
    // different act with a different name -- see
    // `derive-genesis-infrastructure-profile` -- so that danger is avoided by
    // separating the two commands, never by relaxing this one.
    let predecessor_bytes = read_bytes(required(flags, "--predecessor-profile")?)?;
    require_no_flags(flags)?;
    let predecessor = ProtocolInfrastructureProfileV1::decode(&predecessor_bytes)
        .map_err(|error| format!("predecessor infrastructure profile refused: {error:?}"))?;
    let profile = derive_protocol_infrastructure_profile_v2(&registry, &rent, predecessor)
        .map_err(format_release_error)?;
    fs::write(&output, profile.to_bytes())
        .map_err(|error| format!("failed writing {}: {error}", output.display()))
}

/// Derive the write-once V1 profile a cohort that succeeds NOTHING commits.
///
/// This is the profile `InitializeProtocolInfrastructureV1` writes at
/// `dclutch:infrastructure:v1` when infrastructure is founded rather than
/// succeeded. It takes no predecessor because there is none to read: a genesis
/// cohort's two binding ids are wholly a function of its own Registry and Rent
/// manifests, which is exactly why this derivation is offline and complete
/// while the succession one is not.
///
/// `derive_protocol_infrastructure_profile_v1` has existed in this crate's
/// library since the succession work landed, reachable only from its own unit
/// tests. Ember's 2026-09-01 ruling that devnet is disposable -- redeploy a
/// fresh cohort from exact current sources, abandon the previous one in place
/// rather than migrating it -- makes the founding act the next one the project
/// actually performs, so the derivation needs a route a cold operator can
/// reach. Naming the command `genesis` rather than adding a mode to the
/// succession command is deliberate: the two emit different profile versions
/// for different chain acts, and a reader of either invocation can see which
/// one they got without inspecting the output bytes.
fn derive_genesis_infrastructure_profile(
    flags: &mut BTreeMap<String, PathBuf>,
) -> Result<(), String> {
    // A predecessor is not merely unnecessary here, it is meaningless: if the
    // caller has one, the act they intend is a succession and belongs to the
    // other command. Refuse rather than silently discard the flag, so a
    // mistyped command cannot quietly produce the wrong profile version. This
    // is checked BEFORE any input is read, so an operator who typed the wrong
    // command is told that, rather than being sent to debug whichever release
    // manifest happened to fail to load first.
    if flags.contains_key("--predecessor-profile") {
        return Err(
            "--predecessor-profile is not accepted by derive-genesis-infrastructure-profile: a \
             genesis cohort succeeds nothing. Use derive-infrastructure-profile for a succession."
                .to_owned(),
        );
    }
    let output = required(flags, "--out")?;
    // Both bodies, because `InitializeProtocolInfrastructureV1` commits both
    // in one instruction. Required rather than optional: a pack that carried
    // only the V1 would describe half the chain act, and every consumer reads
    // the V2.
    let v2_output = required(flags, "--v2-out")?;
    let registry = load_checked_release(required(flags, "--registry")?)?;
    let rent = load_checked_release(required(flags, "--rent")?)?;
    require_no_flags(flags)?;
    let profile = derive_protocol_infrastructure_profile_v1(&registry, &rent)
        .map_err(format_release_error)?;
    let genesis = ProtocolInfrastructureProfileV2::genesis(profile.registry(), profile.rent())
        .map_err(|error| format!("genesis infrastructure V2 profile refused: {error:?}"))?;
    fs::write(&output, profile.to_bytes())
        .map_err(|error| format!("failed writing {}: {error}", output.display()))?;
    fs::write(&v2_output, genesis.to_bytes())
        .map_err(|error| format!("failed writing {}: {error}", v2_output.display()))
}

/// Probe one dumped account against the ZeroBump seal-close arm, offline.
///
/// The cohort-9 plan review gates that arm on this check: the one stranded seal
/// must be shown, before the cut is built, to be exactly the shape the arm
/// reads. Everything the probe decides is decided by the real seal contract and
/// by `create_program_address`, so a PASS here is a statement about the code the
/// chain will run rather than about this tool's reading of it.
///
/// It opens no socket. Fetch the account once with
/// `solana account <ADDRESS> --output json --output-file <PATH>` and hand the
/// file over; `--address` is only needed for a dump that carries no `pubkey`,
/// and `--program-id` only to check the dump against a Program named
/// independently of what the dump itself claims owns it.
///
/// The projection ends in `verdict=PASS` or `verdict=DOA`, and a DOA also exits
/// non-zero with the failing conjunct named, so a gate script can use either.
fn seal_probe(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let account_path = required(flags, "--account")?;
    let live_release = required_identity32(flags, "--live-release")?;
    let address = optional_identity32(flags, "--address")?;
    let named_program = optional_identity32(flags, "--program-id")?;
    let text_output = flags.remove("--text-out");
    require_no_flags(flags)?;

    let json = fs::read_to_string(&account_path)
        .map_err(|error| format!("failed reading {}: {error}", account_path.display()))?;
    let account = SealAccountDumpV1::parse(&json, address)?;
    // A seal is derived under the Program that owns it, so the dump's owner is
    // the derivation's program unless the caller names one to check against.
    let program = named_program.unwrap_or_else(|| account.owner());
    let verdict = probe_defunct_seal_v1(&account, program, live_release);

    let mut text = String::new();
    push_line(&mut text, "format", "dclutch-zerobump-seal-probe-v1");
    push_line(&mut text, "account", &base58(account.address()));
    push_line(&mut text, "owner", &base58(account.owner()));
    push_line(&mut text, "program_id", &base58(program));
    push_line(
        &mut text,
        "owner_is_program",
        yes_no(verdict.owner_is_program),
    );
    push_line(&mut text, "lamports", &account.lamports().to_string());
    push_line(&mut text, "data_bytes", &account.data().len().to_string());
    push_line(
        &mut text,
        "seal_bytes",
        &CAPABILITY_SEAL_BYTES_V1.to_string(),
    );
    push_line(
        &mut text,
        "funded_rent_persists",
        yes_no(verdict.funded_rent_persists),
    );
    push_line(
        &mut text,
        "decode",
        &conjunct(verdict.canonical.map(|_| ())),
    );
    push_line(
        &mut text,
        "persisted_bump",
        &verdict
            .canonical
            .map_or_else(|_| "unwritten".to_owned(), |bump| bump.to_string()),
    );
    push_line(&mut text, "decode_defunct", &conjunct(verdict.defunct));
    if let Some(key) = verdict.key {
        push_line(
            &mut text,
            "descriptor_schema",
            &hex(&key.descriptor_schema()),
        );
        push_line(
            &mut text,
            "descriptor_digest",
            &hex(&key.descriptor_digest()),
        );
        push_line(&mut text, "action", &key.action().to_string());
        push_line(
            &mut text,
            "sealed_trading_release",
            &hex(&key.trading_semantic_release()),
        );
        push_line(&mut text, "registry_program", &hex(&key.registry_program()));
    }
    push_line(&mut text, "live_trading_release", &hex(&live_release));
    push_line(
        &mut text,
        "release_is_live",
        verdict.release_is_live.map_or("unknown", yes_no),
    );
    push_line(
        &mut text,
        "bump_candidate",
        &verdict
            .bump_candidate
            .map_or_else(|| "none".to_owned(), |bump| bump.to_string()),
    );
    push_line(
        &mut text,
        "verdict",
        if verdict.closable() { "PASS" } else { "DOA" },
    );
    let refusal = verdict.refusal();
    if let Some(reason) = refusal.as_deref() {
        push_line(&mut text, "reason", reason);
    }
    write_text(text, text_output)?;
    match refusal {
        None => Ok(()),
        Some(reason) => Err(format!("this seal is not closable at the cut: {reason}")),
    }
}

fn base58(identity: [u8; 32]) -> String {
    Pubkey::new_from_array(identity).to_string()
}

const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// Render one seal-contract conjunct, naming its refusal when it refused.
fn conjunct(result: Result<(), dclutch_vm::capability_seal::Error>) -> String {
    match result {
        Ok(()) => "ok".to_owned(),
        Err(error) => format!("refused:{error:?}"),
    }
}

fn write_text(text: String, output: Option<PathBuf>) -> Result<(), String> {
    if let Some(path) = output {
        fs::write(&path, text)
            .map_err(|error| format!("failed writing {}: {error}", path.display()))
    } else {
        io::stdout()
            .lock()
            .write_all(text.as_bytes())
            .map_err(|error| format!("failed writing stdout: {error}"))
    }
}

fn push_line(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push('=');
    output.push_str(value);
    output.push('\n');
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push(hex_digit(byte >> 4));
        output.push(hex_digit(byte & 0x0f));
    }
    output
}

fn hex_digit(value: u8) -> char {
    match value {
        10..=15 => char::from(b'a'.saturating_add(value.saturating_sub(10))),
        _ => char::from(b'0'.saturating_add(value)),
    }
}

fn required_hex32(flags: &mut BTreeMap<String, PathBuf>, name: &str) -> Result<[u8; 32], String> {
    let value = required(flags, name)?;
    decode_hex32(&value.to_string_lossy(), name)
}

fn optional_hex32(
    flags: &mut BTreeMap<String, PathBuf>,
    name: &str,
) -> Result<Option<[u8; 32]>, String> {
    flags
        .remove(name)
        .map(|value| decode_hex32(&value.to_string_lossy(), name))
        .transpose()
}

fn required_u64(flags: &mut BTreeMap<String, PathBuf>, name: &str) -> Result<u64, String> {
    let value = required(flags, name)?;
    value
        .to_string_lossy()
        .parse::<u64>()
        .map_err(|error| format!("{name} must be a decimal u64: {error}"))
}

fn required_identity32(
    flags: &mut BTreeMap<String, PathBuf>,
    name: &str,
) -> Result<[u8; 32], String> {
    let value = required(flags, name)?;
    decode_identity32(&value.to_string_lossy(), name)
}

fn optional_identity32(
    flags: &mut BTreeMap<String, PathBuf>,
    name: &str,
) -> Result<Option<[u8; 32]>, String> {
    flags
        .remove(name)
        .map(|value| decode_identity32(&value.to_string_lossy(), name))
        .transpose()
}

/// Read one 32-byte identity in either form an operator has it to hand.
///
/// Manifests here print hex and a cluster prints base58, and the probe below is
/// fed from both at once — an account address off an explorer beside a semantic
/// release out of a manifest. The two encodings cannot be confused: a base58
/// 32-byte identity is 32 to 44 characters, so 64 hexadecimal digits is
/// unambiguous.
fn decode_identity32(value: &str, name: &str) -> Result<[u8; 32], String> {
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return decode_hex32(&value.to_ascii_lowercase(), name);
    }
    value
        .parse::<Pubkey>()
        .map(|identity| identity.to_bytes())
        .map_err(|_| {
            format!("{name} must be 64 hexadecimal digits or a base58 32-byte identity: `{value}`")
        })
}

fn decode_hex32(value: &str, name: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err(format!("{name} must be 64 lowercase hexadecimal digits"));
    }
    let mut output = [0_u8; 32];
    for (destination, pair) in output.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let high = nibble(pair.first().copied(), name)?;
        let low = nibble(pair.get(1).copied(), name)?;
        *destination = high.saturating_mul(16).saturating_add(low);
    }
    Ok(output)
}

fn nibble(value: Option<u8>, name: &str) -> Result<u8, String> {
    match value {
        Some(byte @ b'0'..=b'9') => Ok(byte.saturating_sub(b'0')),
        Some(byte @ b'a'..=b'f') => Ok(byte.saturating_sub(b'a').saturating_add(10)),
        _ => Err(format!("{name} must be 64 lowercase hexadecimal digits")),
    }
}

fn create_set(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let output = required(flags, "--out")?;
    let text_output = flags.remove("--text-out");
    let release_set = load_release_set(required(flags, "--release-set")?)?;
    let manifests = CheckedManifestFiles::load(flags)?;
    require_no_flags(flags)?;
    let checked = manifests.decode()?;
    let result = build_checked_execution_release_set(
        release_set,
        [
            &checked[0],
            &checked[1],
            &checked[2],
            &checked[3],
            &checked[4],
        ],
    )
    .map_err(format_release_error)?;
    fs::write(&output, result.encode())
        .map_err(|error| format!("failed writing {}: {error}", output.display()))?;
    emit_set_text(result, text_output)
}

fn verify_set(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest_path = required(flags, "--manifest")?;
    let text_output = flags.remove("--text-out");
    let manifests = CheckedManifestFiles::load(flags)?;
    require_no_flags(flags)?;
    let manifest = read_bytes(manifest_path)?;
    let result = verify_checked_execution_release_set(&manifest, manifests.refs())
        .map_err(format_release_error)?;
    emit_set_text(result, text_output)
}

fn inspect_set(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest_path = required(flags, "--manifest")?;
    let text_output = flags.remove("--text-out");
    require_no_flags(flags)?;
    let manifest = read_bytes(manifest_path)?;
    let result = CheckedExecutionReleaseSetV1::decode(&manifest).map_err(format_release_error)?;
    emit_set_text(result, text_output)
}

fn create(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let output = required(flags, "--out")?;
    let text_output = flags.remove("--text-out");
    let files = EvidenceFiles::load(flags)?;
    require_no_flags(flags)?;
    let release = build_checked_release(files.evidence()).map_err(format_release_error)?;
    let bytes = release.encode().map_err(format_release_error)?;
    fs::write(&output, bytes)
        .map_err(|error| format!("failed writing {}: {error}", output.display()))?;
    emit_text(&release, text_output)
}

fn verify(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest_path = required(flags, "--manifest")?;
    let text_output = flags.remove("--text-out");
    let files = EvidenceFiles::load(flags)?;
    require_no_flags(flags)?;
    let manifest = fs::read(&manifest_path)
        .map_err(|error| format!("failed reading {}: {error}", manifest_path.display()))?;
    let release =
        verify_checked_release(&manifest, files.evidence()).map_err(format_release_error)?;
    emit_text(&release, text_output)
}

fn inspect(flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let manifest_path = required(flags, "--manifest")?;
    let text_output = flags.remove("--text-out");
    require_no_flags(flags)?;
    let manifest = fs::read(&manifest_path)
        .map_err(|error| format!("failed reading {}: {error}", manifest_path.display()))?;
    let release = CheckedReleaseV1::decode(&manifest).map_err(format_release_error)?;
    emit_text(&release, text_output)
}

fn emit_text(release: &CheckedReleaseV1, output: Option<PathBuf>) -> Result<(), String> {
    let text = release.render_text().map_err(format_release_error)?;
    if let Some(path) = output {
        fs::write(&path, text)
            .map_err(|error| format!("failed writing {}: {error}", path.display()))
    } else {
        io::stdout()
            .lock()
            .write_all(text.as_bytes())
            .map_err(|error| format!("failed writing stdout: {error}"))
    }
}

fn emit_set_text(
    release_set: CheckedExecutionReleaseSetV1,
    output: Option<PathBuf>,
) -> Result<(), String> {
    let text = release_set.render_text().map_err(format_release_error)?;
    if let Some(path) = output {
        fs::write(&path, text)
            .map_err(|error| format!("failed writing {}: {error}", path.display()))
    } else {
        io::stdout()
            .lock()
            .write_all(text.as_bytes())
            .map_err(|error| format!("failed writing stdout: {error}"))
    }
}

/// Flags that are switches rather than name/value pairs.
const VALUELESS_FLAGS_V1: &[&str] = &["--genesis"];

fn parse_flags(
    mut arguments: impl Iterator<Item = OsString>,
) -> Result<BTreeMap<String, PathBuf>, String> {
    let mut flags = BTreeMap::new();
    while let Some(flag) = arguments.next() {
        let flag = flag
            .into_string()
            .map_err(|_| "flag names must be UTF-8".to_owned())?;
        if !flag.starts_with("--") {
            return Err(format!("expected flag, found: {flag}"));
        }
        // Switches, named one by one and never inferred. Every other flag
        // takes a value, and a switch that consumed one would silently eat the
        // NEXT flag's name -- which is exactly how `--genesis` first swallowed
        // `--execution` and reported the error one argument too late.
        let value = if VALUELESS_FLAGS_V1.contains(&flag.as_str()) {
            OsString::from("")
        } else {
            arguments
                .next()
                .ok_or_else(|| format!("missing value for {flag}"))?
        };
        if flags.insert(flag.clone(), PathBuf::from(value)).is_some() {
            return Err(format!("duplicate flag: {flag}"));
        }
    }
    Ok(flags)
}

fn required(flags: &mut BTreeMap<String, PathBuf>, name: &str) -> Result<PathBuf, String> {
    flags
        .remove(name)
        .ok_or_else(|| format!("missing required flag: {name}"))
}

fn require_no_flags(flags: &BTreeMap<String, PathBuf>) -> Result<(), String> {
    if let Some((name, _)) = flags.first_key_value() {
        return Err(format!("unknown flag: {name}"));
    }
    Ok(())
}

fn format_release_error(error: dclutch_release_tool::Error) -> String {
    format!("release evidence refused: {error}")
}

struct EvidenceFiles {
    elf: Vec<u8>,
    semantic_preimage: Vec<u8>,
    program_account_data: Vec<u8>,
    programdata_account_data: Vec<u8>,
    metadata: BuildMetadataV1,
}

struct CheckedManifestFiles {
    manifests: [Vec<u8>; 5],
}

struct TranslationEvidenceFiles {
    inputs: [Vec<u8>; CHECKED_TRANSLATION_VALIDATION_INPUT_COUNT_V1],
}

impl TranslationEvidenceFiles {
    fn load(directory: PathBuf) -> Result<Self, String> {
        let mut inputs = Vec::with_capacity(CHECKED_TRANSLATION_VALIDATION_INPUT_COUNT_V1);
        for label in CHECKED_TRANSLATION_VALIDATION_LABELS_V1 {
            inputs.push(read_bytes(directory.join(format!("{label}.bin")))?);
        }
        let inputs = inputs.try_into().map_err(|values: Vec<Vec<u8>>| {
            format!(
                "translation evidence directory produced {} inputs, expected {}",
                values.len(),
                CHECKED_TRANSLATION_VALIDATION_INPUT_COUNT_V1,
            )
        })?;
        Ok(Self { inputs })
    }

    fn evidence(&self) -> TranslationValidationEvidenceV1<'_> {
        TranslationValidationEvidenceV1 {
            corpus: &self.inputs[0],
            lean_sources: [
                &self.inputs[1],
                &self.inputs[2],
                &self.inputs[3],
                &self.inputs[4],
                &self.inputs[5],
                &self.inputs[6],
                &self.inputs[7],
                &self.inputs[8],
            ],
            rust_sources: [
                &self.inputs[9],
                &self.inputs[10],
                &self.inputs[11],
                &self.inputs[12],
                &self.inputs[13],
                &self.inputs[14],
                &self.inputs[15],
                &self.inputs[16],
            ],
            validator_result: &self.inputs[17],
            rustc_verbose: &self.inputs[18],
            lake_version: &self.inputs[19],
            validator_cargo_lock: &self.inputs[20],
        }
    }
}

impl CheckedManifestFiles {
    fn load(flags: &mut BTreeMap<String, PathBuf>) -> Result<Self, String> {
        Ok(Self {
            manifests: [
                read_bytes(required(flags, "--core")?)?,
                read_bytes(required(flags, "--claims")?)?,
                read_bytes(required(flags, "--trading")?)?,
                read_bytes(required(flags, "--resolution")?)?,
                read_bytes(required(flags, "--custody")?)?,
            ],
        })
    }

    fn decode(&self) -> Result<[CheckedReleaseV1; 5], String> {
        Ok([
            CheckedReleaseV1::decode(&self.manifests[0]).map_err(format_release_error)?,
            CheckedReleaseV1::decode(&self.manifests[1]).map_err(format_release_error)?,
            CheckedReleaseV1::decode(&self.manifests[2]).map_err(format_release_error)?,
            CheckedReleaseV1::decode(&self.manifests[3]).map_err(format_release_error)?,
            CheckedReleaseV1::decode(&self.manifests[4]).map_err(format_release_error)?,
        ])
    }

    fn refs(&self) -> [&[u8]; 5] {
        [
            &self.manifests[0],
            &self.manifests[1],
            &self.manifests[2],
            &self.manifests[3],
            &self.manifests[4],
        ]
    }
}

fn load_release_set(path: PathBuf) -> Result<ExecutionReleaseSetV1, String> {
    let bytes = read_bytes(path)?;
    ExecutionReleaseSetV1::decode(&bytes)
        .map_err(|error| format!("execution release set refused: {error:?}"))
}

fn load_checked_release(path: PathBuf) -> Result<CheckedReleaseV1, String> {
    CheckedReleaseV1::decode(&read_bytes(path)?).map_err(format_release_error)
}

impl EvidenceFiles {
    fn load(flags: &mut BTreeMap<String, PathBuf>) -> Result<Self, String> {
        let elf = read_bytes(required(flags, "--elf")?)?;
        let semantic_preimage = read_bytes(required(flags, "--semantic-preimage")?)?;
        let program_account_data = read_bytes(required(flags, "--program-account-data")?)?;
        let programdata_account_data = read_bytes(required(flags, "--programdata-account-data")?)?;
        let metadata_path = required(flags, "--metadata")?;
        let metadata_text = fs::read_to_string(&metadata_path).map_err(|error| {
            format!(
                "failed reading {} as UTF-8: {error}",
                metadata_path.display()
            )
        })?;
        let metadata = BuildMetadataV1::parse(&metadata_text).map_err(format_release_error)?;
        Ok(Self {
            elf,
            semantic_preimage,
            program_account_data,
            programdata_account_data,
            metadata,
        })
    }

    fn evidence(&self) -> ReleaseEvidenceV1<'_> {
        ReleaseEvidenceV1 {
            elf: &self.elf,
            semantic_preimage: &self.semantic_preimage,
            program_account_data: &self.program_account_data,
            programdata_account_data: &self.programdata_account_data,
            metadata: &self.metadata,
        }
    }
}

fn read_bytes(path: PathBuf) -> Result<Vec<u8>, String> {
    fs::read(&path).map_err(|error| format!("failed reading {}: {error}", path.display()))
}
