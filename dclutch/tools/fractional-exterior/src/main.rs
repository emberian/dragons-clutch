#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Durable Fractional private-validator exterior.
//!
//! The ProgramTest campaign at
//! `programs/dclutch-claims-sbf/program-test/fractional-atomic/` proves the
//! Fractional actions execute against real ELFs. It proves it inside a harness:
//! no fees, no cluster, no finality, and accounts planted through a door a real
//! validator does not have.
//!
//! This drives the same actions against a real localhost validator -- the same
//! pinned `solana-test-validator` the successor launcher wraps -- with the
//! fixture preloaded at genesis, every action submitted as a real transaction
//! confirmed to `finalized`, and a durable journal that makes a rerun
//! byte-identical. It consumes the validator at the process boundary and shares
//! no code, build, or workspace with any other tool.

mod claim_check;
mod cubic_life;
mod journal;
mod narrow_fixture;
mod stage;
mod validator;

use std::{env, fs, path::PathBuf, process::ExitCode};

/// Stable exterior refusal.
#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl Error {
    /// One refusal with an exact reason.
    pub fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

/// Result alias.
pub type Result<T> = core::result::Result<T, Box<dyn std::error::Error>>;

fn usage() {
    eprintln!(
        "usage:
  dclutch-fractional-exterior prepare --elf-dir ABS --out ABS
      Stage the Fractional fixture as genesis account files and write the
      manifest. Deterministic: the same inputs produce the same digest.

  dclutch-fractional-exterior run --elf-dir ABS --out ABS [--keep]
      Prepare, start a private validator on the staged genesis, submit every
      action to finalized, and journal each one. --keep leaves the validator up.

  dclutch-fractional-exterior verify --out ABS
      Re-read the journal and check it is internally exact.

  dclutch-fractional-exterior prepare-claim-check --elf-dir ABS --out ABS
      Stage the canonical post-compaction claim-check boundary.

  dclutch-fractional-exterior run-claim-check --elf-dir ABS --out ABS [--keep]
      Drive hostile rollback, partial and settling burn/pay, then permissionless
      escrow close through real Claims and Token-2022 ELFs.

  dclutch-fractional-exterior verify-claim-check --out ABS
      Recompute the exact instruction/frame/state contract of that journal.

  dclutch-fractional-exterior run-cubic-life --source-root ABS --elf-dir ABS --out ABS
      Run or resume one propagated cubic wrap/transfer/unwrap, real-ELF
      permissionless compaction, and post-compaction redemption/close life.

  dclutch-fractional-exterior verify-cubic-life --source-root ABS --elf-dir ABS --out ABS
      Recompute every source, ELF, bridge, identity, amount and phase digest."
    );
}

fn absolute(value: Option<String>, flag: &str) -> Result<PathBuf> {
    let raw = value.ok_or_else(|| Error::new(format!("{flag} is required")))?;
    let path = PathBuf::from(&raw);
    if !path.is_absolute() {
        return Err(Error::new(format!("{flag} must be absolute: {raw}")).into());
    }
    Ok(path)
}

struct ParsedArguments {
    elf_dir: Option<PathBuf>,
    out: PathBuf,
    keep: bool,
}

fn parse(arguments: Vec<String>) -> Result<ParsedArguments> {
    let (mut elf_dir, mut out, mut keep) = (None, None, false);
    let mut rest = arguments.into_iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--keep" => keep = true,
            "--elf-dir" => elf_dir = rest.next(),
            "--out" => out = rest.next(),
            other => return Err(Error::new(format!("unknown argument: {other}")).into()),
        }
    }
    let elf_dir = elf_dir
        .map(|value| absolute(Some(value), "--elf-dir"))
        .transpose()?;
    Ok(ParsedArguments {
        elf_dir,
        out: absolute(out, "--out")?,
        keep,
    })
}

fn parse_with_elves(arguments: Vec<String>) -> Result<(PathBuf, PathBuf, bool)> {
    let parsed = parse(arguments)?;
    Ok((
        parsed
            .elf_dir
            .ok_or_else(|| Error::new("--elf-dir is required"))?,
        parsed.out,
        parsed.keep,
    ))
}

fn parse_verify(arguments: Vec<String>) -> Result<PathBuf> {
    let parsed = parse(arguments)?;
    if parsed.elf_dir.is_some() {
        return Err(Error::new("verify does not read ELFs; omit --elf-dir").into());
    }
    if parsed.keep {
        return Err(Error::new("verify does not start a validator; omit --keep").into());
    }
    Ok(parsed.out)
}

fn parse_life(arguments: Vec<String>) -> Result<(PathBuf, PathBuf, PathBuf)> {
    let (mut source, mut elf, mut out) = (None, None, None);
    let mut rest = arguments.into_iter();
    while let Some(argument) = rest.next() {
        let slot = match argument.as_str() {
            "--source-root" => &mut source,
            "--elf-dir" => &mut elf,
            "--out" => &mut out,
            other => return Err(Error::new(format!("unknown argument: {other}")).into()),
        };
        if slot.is_some() {
            return Err(Error::new(format!("duplicate argument: {argument}")).into());
        }
        *slot = Some(
            rest.next()
                .ok_or_else(|| Error::new(format!("{argument} requires a value")))?,
        );
    }
    Ok((
        absolute(source, "--source-root")?,
        absolute(elf, "--elf-dir")?,
        absolute(out, "--out")?,
    ))
}

fn main() -> ExitCode {
    let mut arguments = env::args();
    let _binary = arguments.next();
    let command = arguments.next();
    let rest: Vec<String> = arguments.collect();
    let outcome = match command.as_deref() {
        Some("prepare") => parse_with_elves(rest).and_then(|(elf, out, _)| {
            let staged = validator::prepare(&elf, &out)?;
            println!("staged {} accounts -> {}", staged, out.display());
            Ok(())
        }),
        Some("run") => {
            parse_with_elves(rest).and_then(|(elf, out, keep)| validator::run(&elf, &out, keep))
        }
        Some("verify") => parse_verify(rest).and_then(|out| {
            let (entries, digest) = journal::verify(&out)?;
            println!("journal exact: {entries} entries, sha256 {digest}");
            Ok(())
        }),
        Some("prepare-claim-check") => parse_with_elves(rest).and_then(|(elf, out, _)| {
            let staged = claim_check::prepare(&elf, &out)?;
            println!("staged {staged} claim-check accounts -> {}", out.display());
            Ok(())
        }),
        Some("run-claim-check") => {
            parse_with_elves(rest).and_then(|(elf, out, keep)| claim_check::run(&elf, &out, keep))
        }
        Some("verify-claim-check") => parse_verify(rest).and_then(|out| {
            let (entries, digest) = claim_check::verify(&out)?;
            println!("claim-check journal exact: {entries} entries, sha256 {digest}");
            Ok(())
        }),
        Some("run-cubic-life") => {
            parse_life(rest).and_then(|(source, elf, out)| cubic_life::run(&source, &elf, &out))
        }
        Some("verify-cubic-life") => parse_life(rest).and_then(|(source, elf, out)| {
            let digest = cubic_life::verify(&source, &elf, &out)?;
            println!("fractional cubic life exact: sha256 {digest}");
            Ok(())
        }),
        _ => {
            usage();
            return ExitCode::from(2);
        }
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dclutch-fractional-exterior: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_life, parse_verify, parse_with_elves};

    #[test]
    fn cubic_life_requires_one_absolute_value_per_flag() {
        let parsed = parse_life(vec![
            "--out".into(),
            "/tmp/evidence".into(),
            "--source-root".into(),
            "/tmp/source".into(),
            "--elf-dir".into(),
            "/tmp/elves".into(),
        ])
        .expect("life arguments");
        assert_eq!(parsed.0, std::path::PathBuf::from("/tmp/source"));
        assert_eq!(parsed.1, std::path::PathBuf::from("/tmp/elves"));
        assert_eq!(parsed.2, std::path::PathBuf::from("/tmp/evidence"));

        let duplicate = parse_life(vec![
            "--source-root".into(),
            "/tmp/source".into(),
            "--source-root".into(),
            "/tmp/other".into(),
        ])
        .expect_err("duplicate must refuse")
        .to_string();
        assert_eq!(duplicate, "duplicate argument: --source-root");

        let unknown = parse_life(vec!["--imaginary".into(), "/tmp/value".into()])
            .expect_err("unknown must refuse")
            .to_string();
        assert_eq!(unknown, "unknown argument: --imaginary");

        let relative = parse_life(vec![
            "--source-root".into(),
            "relative".into(),
            "--elf-dir".into(),
            "/tmp/elves".into(),
            "--out".into(),
            "/tmp/evidence".into(),
        ])
        .expect_err("relative source must refuse")
        .to_string();
        assert!(relative.contains("--source-root must be absolute"));
    }

    #[test]
    fn verify_needs_only_an_absolute_output() {
        assert_eq!(
            parse_verify(vec!["--out".into(), "/tmp/evidence".into()]).expect("verify arguments"),
            std::path::PathBuf::from("/tmp/evidence")
        );
    }

    #[test]
    fn verify_refuses_run_only_options() {
        let elf = parse_verify(vec![
            "--out".into(),
            "/tmp/evidence".into(),
            "--elf-dir".into(),
            "/tmp/elves".into(),
        ])
        .expect_err("verify must not pretend to read ELFs")
        .to_string();
        assert!(elf.contains("does not read ELFs"));
        let keep = parse_verify(vec![
            "--out".into(),
            "/tmp/evidence".into(),
            "--keep".into(),
        ])
        .expect_err("verify must not pretend to leave a validator running")
        .to_string();
        assert!(keep.contains("does not start a validator"));
    }

    #[test]
    fn run_requires_an_absolute_elf_directory() {
        let missing = parse_with_elves(vec!["--out".into(), "/tmp/evidence".into()])
            .expect_err("run requires ELFs")
            .to_string();
        assert!(missing.contains("--elf-dir is required"));
        let relative = parse_with_elves(vec![
            "--out".into(),
            "/tmp/evidence".into(),
            "--elf-dir".into(),
            "relative".into(),
        ])
        .expect_err("ELF directory must be absolute")
        .to_string();
        assert!(relative.contains("must be absolute"));
    }
}

/// Read one required ELF.
pub fn read_elf(directory: &std::path::Path, name: &str) -> Result<Vec<u8>> {
    let path = directory.join(name);
    fs::read(&path)
        .map_err(|error| Error::new(format!("missing real ELF {}: {error}", path.display())).into())
}
