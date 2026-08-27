#![forbid(unsafe_code)]

use std::{env, error::Error as StdError, fmt, io::Write, path::PathBuf};

use solana_sdk::pubkey::Pubkey;

mod market;
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
        Some("demo-market") => run_demo_market(arguments.collect()),
        Some("run") => run_runtime(arguments.collect()),
        Some("help" | "-h" | "--help") | None => {
            usage();
            Ok(())
        }
        Some(command) => Err(Error::new(format!("unknown command: {command}"))),
    }
}

fn run_runtime(arguments: Vec<String>) -> Result<()> {
    let mut spec = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--spec" => &mut spec,
            _ => return Err(Error::new(format!("unknown run argument: {argument}"))),
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    runtime::execute(&absolute(spec, "--spec")?)
}

fn run_demo_market(arguments: Vec<String>) -> Result<()> {
    let mut registry = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--registry-program-id" => &mut registry,
            _ => {
                return Err(Error::new(format!(
                    "unknown demo-market argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let registry = parse_pubkey(registry, "--registry-program-id")?;
    let input = market::demo_market_input(registry)?;
    let mut stdout = std::io::stdout();
    stdout.write_all(&serde_json::to_vec_pretty(&input)?)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

fn run_prepare(arguments: Vec<String>) -> Result<()> {
    let mut account_dir = None;
    let mut output = None;
    let mut registry_program = None;
    let mut registry_elf = None;
    let mut registry_sha256 = None;
    let mut registry_semantic_release = None;
    let mut core_program = None;
    let mut core_elf = None;
    let mut core_sha256 = None;
    let mut core_semantic_release = None;
    let mut core_bootstrap_upgrade_authority = None;
    let mut claims_program = None;
    let mut claims_elf = None;
    let mut claims_sha256 = None;
    let mut claims_semantic_release = None;
    let mut trading_program = None;
    let mut trading_elf = None;
    let mut trading_sha256 = None;
    let mut trading_semantic_release = None;
    let mut resolution_program = None;
    let mut resolution_elf = None;
    let mut resolution_sha256 = None;
    let mut resolution_semantic_release = None;
    let mut custody_program = None;
    let mut custody_elf = None;
    let mut custody_sha256 = None;
    let mut custody_semantic_release = None;
    let mut rent_credit_program = None;
    let mut rent_credit_elf = None;
    let mut rent_credit_sha256 = None;
    let mut rent_credit_semantic_release = None;
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
            "--registry-semantic-release-id" => &mut registry_semantic_release,
            "--core-program-id" => &mut core_program,
            "--core-elf" => &mut core_elf,
            "--core-sha256" => &mut core_sha256,
            "--core-semantic-release-id" => &mut core_semantic_release,
            "--core-bootstrap-upgrade-authority" => &mut core_bootstrap_upgrade_authority,
            "--claims-program-id" => &mut claims_program,
            "--claims-elf" => &mut claims_elf,
            "--claims-sha256" => &mut claims_sha256,
            "--claims-semantic-release-id" => &mut claims_semantic_release,
            "--trading-program-id" => &mut trading_program,
            "--trading-elf" => &mut trading_elf,
            "--trading-sha256" => &mut trading_sha256,
            "--trading-semantic-release-id" => &mut trading_semantic_release,
            "--resolution-program-id" => &mut resolution_program,
            "--resolution-elf" => &mut resolution_elf,
            "--resolution-sha256" => &mut resolution_sha256,
            "--resolution-semantic-release-id" => &mut resolution_semantic_release,
            "--custody-program-id" => &mut custody_program,
            "--custody-elf" => &mut custody_elf,
            "--custody-sha256" => &mut custody_sha256,
            "--custody-semantic-release-id" => &mut custody_semantic_release,
            "--rent-credit-program-id" => &mut rent_credit_program,
            "--rent-credit-elf" => &mut rent_credit_elf,
            "--rent-credit-sha256" => &mut rent_credit_sha256,
            "--rent-credit-semantic-release-id" => &mut rent_credit_semantic_release,
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
        registry_semantic_release_id: required(
            registry_semantic_release,
            "--registry-semantic-release-id",
        )?,
        core_program: parse_pubkey(core_program, "--core-program-id")?,
        core_elf: absolute(core_elf, "--core-elf")?,
        core_sha256: required(core_sha256, "--core-sha256")?,
        core_semantic_release_id: required(core_semantic_release, "--core-semantic-release-id")?,
        core_bootstrap_upgrade_authority: parse_pubkey(
            core_bootstrap_upgrade_authority,
            "--core-bootstrap-upgrade-authority",
        )?,
        claims_program: parse_pubkey(claims_program, "--claims-program-id")?,
        claims_elf: absolute(claims_elf, "--claims-elf")?,
        claims_sha256: required(claims_sha256, "--claims-sha256")?,
        claims_semantic_release_id: required(
            claims_semantic_release,
            "--claims-semantic-release-id",
        )?,
        trading_program: parse_pubkey(trading_program, "--trading-program-id")?,
        trading_elf: absolute(trading_elf, "--trading-elf")?,
        trading_sha256: required(trading_sha256, "--trading-sha256")?,
        trading_semantic_release_id: required(
            trading_semantic_release,
            "--trading-semantic-release-id",
        )?,
        resolution_program: parse_pubkey(resolution_program, "--resolution-program-id")?,
        resolution_elf: absolute(resolution_elf, "--resolution-elf")?,
        resolution_sha256: required(resolution_sha256, "--resolution-sha256")?,
        resolution_semantic_release_id: required(
            resolution_semantic_release,
            "--resolution-semantic-release-id",
        )?,
        custody_program: parse_pubkey(custody_program, "--custody-program-id")?,
        custody_elf: absolute(custody_elf, "--custody-elf")?,
        custody_sha256: required(custody_sha256, "--custody-sha256")?,
        custody_semantic_release_id: required(
            custody_semantic_release,
            "--custody-semantic-release-id",
        )?,
        rent_credit_program: parse_pubkey(rent_credit_program, "--rent-credit-program-id")?,
        rent_credit_elf: absolute(rent_credit_elf, "--rent-credit-elf")?,
        rent_credit_sha256: required(rent_credit_sha256, "--rent-credit-sha256")?,
        rent_credit_semantic_release_id: required(
            rent_credit_semantic_release,
            "--rent-credit-semantic-release-id",
        )?,
    };
    let path = args.plan_path.clone();
    let prepared = plan::prepare(args)?;
    let summary = serde_json::json!({
        "schema": "dclutch-local-successor-prepare-result-v1",
        "plan": path,
        "account_dir": prepared.account_dir,
        "registry_program_id": prepared.registry.program_id,
        "core_program_id": prepared.core.program_id,
        "claims_program_id": prepared.claims.program_id,
        "trading_program_id": prepared.trading.program_id,
        "resolution_program_id": prepared.resolution.program_id,
        "custody_program_id": prepared.custody.program_id,
        "rent_credit_program_id": prepared.rent_credit.program_id,
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
        "Usage:\n  dclutch-local-successor-bootstrap prepare --account-dir ABSOLUTE_NEW_DIR --output ABSOLUTE_NEW_JSON --registry-program-id PUBKEY --registry-elf ABSOLUTE_ELF --registry-sha256 SHA256 --registry-semantic-release-id SHA256 --core-program-id PUBKEY --core-elf ABSOLUTE_ELF --core-sha256 SHA256 --core-semantic-release-id SHA256 --core-bootstrap-upgrade-authority PUBKEY --claims-program-id PUBKEY --claims-elf ABSOLUTE_ELF --claims-sha256 SHA256 --claims-semantic-release-id SHA256 --trading-program-id PUBKEY --trading-elf ABSOLUTE_ELF --trading-sha256 SHA256 --trading-semantic-release-id SHA256 --resolution-program-id PUBKEY --resolution-elf ABSOLUTE_ELF --resolution-sha256 SHA256 --resolution-semantic-release-id SHA256 --custody-program-id PUBKEY --custody-elf ABSOLUTE_ELF --custody-sha256 SHA256 --custody-semantic-release-id SHA256 --rent-credit-program-id PUBKEY --rent-credit-elf ABSOLUTE_ELF --rent-credit-sha256 SHA256 --rent-credit-semantic-release-id SHA256\n  dclutch-local-successor-bootstrap run --spec ABSOLUTE_JSON\n  dclutch-local-successor-bootstrap demo-market --registry-program-id PUBKEY\n\nThe run command is the canonical same-process supervisor. It creates one ephemeral Core authority only in memory, prepares its public key into fresh genesis inputs, starts a guarded foreground localhost validator, initializes Core infrastructure, proves pre-revocation release refusal, revokes Loader-v3 authority to None, and activates the immutable release set. It never reads a wallet or CLI configuration."
    );
}
