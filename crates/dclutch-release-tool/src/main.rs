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

use dclutch_release_set_contract::ExecutionReleaseSetV1;
use dclutch_release_tool::{
    BuildMetadataV1, CheckedCapabilityExecutionV1, CheckedExecutionReleaseSetV1, CheckedReleaseV1,
    ReleaseEvidenceV1, build_checked_capability_execution_from_bytes_v1,
    build_checked_execution_release_set, build_checked_release,
    verify_checked_capability_execution_v1, verify_checked_execution_release_set,
    verify_checked_release,
};

const USAGE: &str = "usage:\n  dclutch-release-tool create --elf PATH --semantic-preimage PATH --metadata PATH --program-account-data PATH --programdata-account-data PATH --out PATH [--text-out PATH]\n  dclutch-release-tool verify --manifest PATH --elf PATH --semantic-preimage PATH --metadata PATH --program-account-data PATH --programdata-account-data PATH [--text-out PATH]\n  dclutch-release-tool inspect --manifest PATH [--text-out PATH]\n  dclutch-release-tool create-set --release-set PATH --core PATH --claims PATH --trading PATH --resolution PATH --custody PATH --out PATH [--text-out PATH]\n  dclutch-release-tool verify-set --manifest PATH --core PATH --claims PATH --trading PATH --resolution PATH --custody PATH [--text-out PATH]\n  dclutch-release-tool inspect-set --manifest PATH [--text-out PATH]\n  dclutch-release-tool create-capability-execution --descriptor PATH --strategy PATH --certificate PATH [--admission PATH] --accelerator PATH --out PATH [--text-out PATH]\n  dclutch-release-tool verify-capability-execution --manifest PATH --accelerator PATH [--text-out PATH]\n  dclutch-release-tool inspect-capability-execution --manifest PATH [--text-out PATH]";

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
        "create-set" => create_set(&mut flags),
        "verify-set" => verify_set(&mut flags),
        "inspect-set" => inspect_set(&mut flags),
        "create-capability-execution" => create_capability_execution(&mut flags),
        "verify-capability-execution" => verify_capability_execution(&mut flags),
        "inspect-capability-execution" => inspect_capability_execution(&mut flags),
        _ => Err(format!("unknown command: {command}")),
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
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
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
