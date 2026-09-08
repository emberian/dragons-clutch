//! File boundary for Series comparison evidence. Computation is performed by
//! the named comparison driver; these commands validate and bind its outputs.

use std::{collections::BTreeMap, fs, io::Write, path::PathBuf};

use dclutch_release_tool::{
    CHECKED_SERIES_TRANSLATION_LABELS_V1, CheckedSeriesTranslationV1, SeriesTranslationEvidenceV1,
};

use crate::{format_release_error, read_bytes, require_no_flags, required, write_text};

pub(crate) fn run(command: &str, flags: &mut BTreeMap<String, PathBuf>) -> Result<(), String> {
    let text_output = flags.remove("--text-out");
    let output = if command == "create-series-translation" {
        Some(required(flags, "--out")?)
    } else {
        None
    };
    let manifest = if output.is_none() {
        Some(read_bytes(required(flags, "--manifest")?)?)
    } else {
        None
    };
    let inputs = if command != "inspect-series-translation" {
        let directory = required(flags, "--evidence-dir")?;
        let mut inputs = Vec::new();
        for label in CHECKED_SERIES_TRANSLATION_LABELS_V1 {
            inputs.push(read_bytes(directory.join(format!("{label}.bin")))?);
        }
        Some(inputs)
    } else {
        None
    };
    require_no_flags(flags)?;
    let checked = if let Some(inputs) = inputs.as_ref() {
        let evidence = SeriesTranslationEvidenceV1 {
            semantic_source: &inputs[0],
            compiler_source_manifest: &inputs[1],
            toolchain_manifest: &inputs[2],
            corpus: &inputs[3],
            validator_source: &inputs[4],
            validator_result: &inputs[5],
            rustc_verbose: &inputs[6],
            cargo_lock: &inputs[7],
        };
        let checked = CheckedSeriesTranslationV1::build(evidence).map_err(format_release_error)?;
        if let Some(bytes) = manifest.as_ref() {
            CheckedSeriesTranslationV1::decode(bytes)
                .and_then(|decoded| decoded.verify(evidence))
                .map_err(format_release_error)?;
        }
        checked
    } else {
        CheckedSeriesTranslationV1::decode(manifest.as_deref().ok_or("missing manifest")?)
            .map_err(format_release_error)?
    };
    if let Some(path) = output {
        // The fixed-width hostile decoder has accepted these bytes before the
        // canonical output is touched. A failed write leaves its predecessor.
        let bytes = checked.encode();
        CheckedSeriesTranslationV1::decode(&bytes).map_err(format_release_error)?;
        let parent = path.parent().ok_or("output has no parent")?;
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| format!("output clock: {error}"))?
            .as_nanos();
        let temporary = parent.join(format!(
            ".series-translation-{}-{nonce}.tmp",
            std::process::id()
        ));
        let result = (|| -> std::io::Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, &path)
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(format!("failed writing {}: {error}", path.display()));
        }
    }
    let id = checked
        .translation_validation_id()
        .map_err(format_release_error)?;
    let hex: String = id
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    write_text(
        format!(
            "dclutch-series-translation-v1\ntranslation_validation_id={hex}\nevidence=finite-native-interpreted-shadow-comparison\n"
        ),
        text_output,
    )
}
