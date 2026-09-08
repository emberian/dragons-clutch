#![forbid(unsafe_code)]

//! Rebuild and authenticate one selected Series Shadow include for a checked
//! accelerator candidate.

use std::{env, fs, path::Path, process::ExitCode};

use dclutch_series_shadow_bundle_generator::{
    SeriesShadowRebuildSourcesV1, require_deterministic_series_shadow_rebuild_v1,
    require_exact_series_shadow_generated_include_v1,
};

const MAX_INPUT_BYTES: usize = 1 << 20;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("series_shadow_checked_include: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let [
        manifest,
        include,
        semantic_source,
        compiler_source,
        toolchain,
    ] = arguments.as_slice()
    else {
        return Err("usage: series_shadow_checked_include <manifest> <include> <semantic-source> <compiler-source> <toolchain-manifest>".into());
    };
    let manifest = read_regular(Path::new(manifest), "source manifest")?;
    let include = read_regular(Path::new(include), "generated include")?;
    let semantic_source = read_regular(Path::new(semantic_source), "semantic source")?;
    let compiler_source = read_regular(Path::new(compiler_source), "compiler source")?;
    let toolchain = read_regular(Path::new(toolchain), "toolchain manifest")?;
    require_deterministic_series_shadow_rebuild_v1(
        &manifest,
        SeriesShadowRebuildSourcesV1 {
            semantic_source: &semantic_source,
            compiler_source: &compiler_source,
            toolchain_manifest: &toolchain,
        },
    )
    .map_err(|error| format!("deterministic source rebuild refused: {error:?}"))?;
    require_exact_series_shadow_generated_include_v1(&manifest, &include).map_err(|error| {
        format!("generated include differs from source-pinned manifest regeneration: {error:?}")
    })?;
    Ok(())
}

fn read_regular(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    if !path.is_absolute() {
        return Err(format!("{label} path is not absolute"));
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("{label} metadata: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(format!("{label} is not a regular file"));
    }
    if fs::canonicalize(path).map_err(|error| format!("{label} canonicalize: {error}"))? != path {
        return Err(format!("{label} path is not canonical"));
    }
    let bytes = fs::read(path).map_err(|error| format!("{label} read: {error}"))?;
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err(format!("{label} width is invalid"));
    }
    Ok(bytes)
}
