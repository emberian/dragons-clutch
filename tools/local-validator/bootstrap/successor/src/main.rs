#![forbid(unsafe_code)]

use std::{env, error::Error as StdError, fmt, fs::OpenOptions, io::Write, path::PathBuf};

use solana_sdk::pubkey::Pubkey;

mod model;
mod plan;
mod rpc;
mod runtime;

type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
struct Error(String);

impl Error {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl StdError for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<std::time::SystemTimeError> for Error {
    fn from(error: std::time::SystemTimeError) -> Self {
        Self::new(error.to_string())
    }
}

fn main() -> core::result::Result<(), Box<dyn StdError>> {
    match run() {
        Ok(()) => Ok(()),
        Err(error) => Err(Box::new(error)),
    }
}

fn run() -> Result<()> {
    let mut arguments = env::args();
    let _program = arguments.next();
    match arguments.next().as_deref() {
        Some("prepare") => run_prepare(arguments.collect()),
        Some("run") => run_runtime(arguments.collect()),
        Some("help" | "-h" | "--help") | None => {
            usage();
            Ok(())
        }
        Some(command) => Err(Error::new(format!("unknown command: {command}"))),
    }
}

fn run_runtime(arguments: Vec<String>) -> Result<()> {
    let mut rpc_url = None;
    let mut plan = None;
    let mut provider_evidence = None;
    let mut output = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            "--plan" => &mut plan,
            "--provider-evidence" => &mut provider_evidence,
            "--output" => &mut output,
            _ => return Err(Error::new(format!("unknown run argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let output = absolute(output, "--output")?;
    let evidence = runtime::execute(&runtime::RunArgs {
        rpc_url: required(rpc_url, "--rpc-url")?,
        plan_path: absolute(plan, "--plan")?,
        provider_evidence_path: absolute(provider_evidence, "--provider-evidence")?,
        output: output.clone(),
    })?;
    create_new_json(&output, &evidence)?;
    let summary = serde_json::json!({
        "schema": evidence.schema,
        "evidence": output,
        "primary_resolution_executed": evidence.primary_resolution_executed,
        "sequential_recovery_exhaustion_failure_executed": evidence.sequential_recovery_exhaustion_failure_executed,
        "rollback_proved": evidence.rollback_proved,
    });
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&summary)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn run_prepare(arguments: Vec<String>) -> Result<()> {
    let mut account_dir = None;
    let mut output = None;
    let mut registry_program = None;
    let mut registry_elf = None;
    let mut registry_sha256 = None;
    let mut resolution_program = None;
    let mut resolution_elf = None;
    let mut resolution_sha256 = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--account-dir" => &mut account_dir,
            "--output" => &mut output,
            "--registry-program-id" => &mut registry_program,
            "--registry-elf" => &mut registry_elf,
            "--registry-sha256" => &mut registry_sha256,
            "--resolution-program-id" => &mut resolution_program,
            "--resolution-elf" => &mut resolution_elf,
            "--resolution-sha256" => &mut resolution_sha256,
            _ => return Err(Error::new(format!("unknown prepare argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let args = plan::PrepareArgs {
        account_dir: absolute(account_dir, "--account-dir")?,
        plan_path: absolute(output, "--output")?,
        registry_program: parse_pubkey(registry_program, "--registry-program-id")?,
        registry_elf: absolute(registry_elf, "--registry-elf")?,
        registry_sha256: required(registry_sha256, "--registry-sha256")?,
        resolution_program: parse_pubkey(resolution_program, "--resolution-program-id")?,
        resolution_elf: absolute(resolution_elf, "--resolution-elf")?,
        resolution_sha256: required(resolution_sha256, "--resolution-sha256")?,
    };
    let path = args.plan_path.clone();
    let prepared = plan::prepare(args)?;
    let summary = serde_json::json!({
        "schema": "dclutch-local-successor-prepare-result-v1",
        "plan": path,
        "account_dir": prepared.account_dir,
        "registry_program_id": prepared.registry.program_id,
        "resolution_program_id": prepared.resolution.program_id,
        "genesis_account_count": prepared.genesis_accounts.len(),
    });
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&summary)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
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

fn parse_pubkey(value: Option<String>, label: &str) -> Result<Pubkey> {
    plan::pubkey(&required(value, label)?)
}

fn usage() {
    println!(
        "Usage:\n  dclutch-local-successor-bootstrap prepare --account-dir ABSOLUTE_NEW_DIR --output ABSOLUTE_NEW_JSON --registry-program-id PUBKEY --registry-elf ABSOLUTE_ELF --registry-sha256 SHA256 --resolution-program-id PUBKEY --resolution-elf ABSOLUTE_ELF --resolution-sha256 SHA256\n  dclutch-local-successor-bootstrap run --rpc-url LOOPBACK_HTTP_ORIGIN --plan ABSOLUTE_JSON --provider-evidence ABSOLUTE_JSON --output ABSOLUTE_NEW_JSON"
    );
}

fn create_new_json(path: &PathBuf, value: &impl serde::Serialize) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    Ok(())
}
