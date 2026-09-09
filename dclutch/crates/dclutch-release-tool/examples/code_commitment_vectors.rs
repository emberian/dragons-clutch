//! Native vectors for the browser's projection of Registry code commitments.

use dclutch_registry::artifact_code_commitment_v2::{
    CODE_COMMITMENT_CHUNK_BYTES_V2, code_commitment_v2,
};
use dclutch_release_tool::Error;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut vectors = Vec::new();
    for length in [
        1,
        64,
        CODE_COMMITMENT_CHUNK_BYTES_V2 - 1,
        CODE_COMMITMENT_CHUNK_BYTES_V2,
        CODE_COMMITMENT_CHUNK_BYTES_V2 + 1,
        2_735_824,
        10_485_715,
    ] {
        let bytes = (0..length)
            .map(|index| u8::try_from(index % 251))
            .collect::<Result<Vec<_>, _>>()?;
        vectors.push(serde_json::json!({
            "length": length,
            "code_commitment": code_commitment_v2(&bytes).map_err(Error::CodeCommitment)?.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
        }));
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "dclutch-code-commitment-native-vectors-v2",
            "generator": "crates/dclutch-release-tool/examples/code_commitment_vectors.rs",
            "input_pattern": "byte[i] = i % 251",
            "chunk_bytes": CODE_COMMITMENT_CHUNK_BYTES_V2,
            "vectors": vectors,
        }))?
    );
    Ok(())
}
