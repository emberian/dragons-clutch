//! Fold a directory of per-transaction records into one census document.
//!
//! usage: fold-program-test-evidence DIRECTORY OUTPUT
//!
//! A campaign writes one file per transaction because `cargo test` runs its
//! cases on many threads. This is the join. It is a binary rather than a `jq`
//! pipeline so the exact document shape has one owner, in Rust, next to the
//! emitter that produced the parts.

use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let (Some(directory), Some(output)) = (arguments.next(), arguments.next()) else {
        eprintln!("usage: fold-program-test-evidence DIRECTORY OUTPUT");
        return ExitCode::FAILURE;
    };
    let directory = PathBuf::from(directory);
    let document = match dclutch_program_test_evidence::fold(&directory) {
        Ok(document) => document,
        Err(error) => {
            eprintln!("fold-program-test-evidence: {error}");
            return ExitCode::FAILURE;
        }
    };
    // Emit to a temporary neighbour and rename, so a failed fold leaves the
    // last accepted document byte-for-byte intact.
    let staging = PathBuf::from(format!("{output}.staging"));
    if let Err(error) = std::fs::write(&staging, document) {
        eprintln!("fold-program-test-evidence: {output}.staging: {error}");
        return ExitCode::FAILURE;
    }
    if let Err(error) = std::fs::rename(&staging, &output) {
        eprintln!("fold-program-test-evidence: {output}: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
