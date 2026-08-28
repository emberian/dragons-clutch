//! Command-line interface for deterministic devnet economic scenario fixtures.

use std::path::PathBuf;

use dclutch_devnet_scenarios::{
    canonical_manifest_bytes, check_fixture_directory, write_fixture_directory,
};

fn main() {
    if let Err(error) = run(std::env::args().skip(1).collect()) {
        eprintln!("devnet-scenarios: {error}");
        std::process::exit(1);
    }
}

fn run(arguments: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    match arguments.as_slice() {
        [command, scenario] if command == "print" => {
            std::io::Write::write_all(
                &mut std::io::stdout(),
                &canonical_manifest_bytes(scenario)?,
            )?;
            Ok(())
        }
        [command, output] if command == "generate" => {
            write_fixture_directory(&absolute(output, "output directory")?)?;
            Ok(())
        }
        [command, fixtures] if command == "check" => {
            check_fixture_directory(&absolute(fixtures, "fixture directory")?)?;
            println!("devnet scenario fixtures: exact");
            Ok(())
        }
        _ => Err(usage().into()),
    }
}

fn absolute(value: &str, label: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute").into());
    }
    Ok(path)
}

fn usage() -> &'static str {
    "usage:\n  dclutch-devnet-scenarios print SCENARIO_ID\n  dclutch-devnet-scenarios generate ABSOLUTE_FRESH_DIRECTORY\n  dclutch-devnet-scenarios check ABSOLUTE_FIXTURE_DIRECTORY"
}
