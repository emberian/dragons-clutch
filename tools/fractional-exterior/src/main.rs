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
      Re-read the journal and check it is internally exact."
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

fn parse(arguments: Vec<String>) -> Result<(PathBuf, PathBuf, bool)> {
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
    Ok((
        absolute(elf_dir, "--elf-dir")?,
        absolute(out, "--out")?,
        keep,
    ))
}

fn main() -> ExitCode {
    let mut arguments = env::args();
    let _binary = arguments.next();
    let command = arguments.next();
    let rest: Vec<String> = arguments.collect();
    let outcome = match command.as_deref() {
        Some("prepare") => parse(rest).and_then(|(elf, out, _)| {
            let staged = validator::prepare(&elf, &out)?;
            println!("staged {} accounts -> {}", staged, out.display());
            Ok(())
        }),
        Some("run") => parse(rest).and_then(|(elf, out, keep)| validator::run(&elf, &out, keep)),
        Some("verify") => parse(rest).and_then(|(_, out, _)| {
            let entries = journal::verify(&out)?;
            println!("journal exact: {entries} entries");
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

/// Read one required ELF.
pub fn read_elf(directory: &std::path::Path, name: &str) -> Result<Vec<u8>> {
    let path = directory.join(name);
    fs::read(&path)
        .map_err(|error| Error::new(format!("missing real ELF {}: {error}", path.display())).into())
}
