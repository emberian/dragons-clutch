//! Native stdin/stdout witness for generated Rust-to-WASM parity fixtures.

use std::io::{Read as _, Write as _};
use std::process::ExitCode;

use dclutch_rational_open_wasm::plan_rational_open_json_v1;

fn main() -> ExitCode {
    let mut source = Vec::new();
    if let Err(error) = std::io::stdin().read_to_end(&mut source) {
        eprintln!("Rational-open native input: {error}");
        return ExitCode::FAILURE;
    }
    let output = match plan_rational_open_json_v1(&source) {
        Ok(output) => output,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(error) = std::io::stdout().write_all(output.as_bytes()) {
        eprintln!("Rational-open native output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
