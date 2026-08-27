//! Select one generator-emitted Series Shadow include at compile time.

use std::{env, fs, path::PathBuf};

const GENERATED_INCLUDE_ENV: &str = "DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE";
const OUTPUT_NAME: &str = "series_shadow_generated.rs";

fn main() {
    println!("cargo:rerun-if-env-changed={GENERATED_INCLUDE_ENV}");
    let output =
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR")).join(OUTPUT_NAME);
    match env::var_os(GENERATED_INCLUDE_ENV) {
        Some(path) => {
            let source = fs::read(path).expect("read checked Series Shadow generated include");
            assert!(
                !source.is_empty(),
                "generated Series Shadow include is empty"
            );
            let mut selected =
                b"pub const SERIES_SHADOW_RELEASE_SELECTED_V1: bool = true;\n".to_vec();
            selected.extend_from_slice(&source);
            fs::write(output, selected).expect("write selected Series Shadow include");
        }
        None => fs::write(output, fallback()).expect("write fail-closed Series Shadow include"),
    }
}

fn fallback() -> &'static [u8] {
    br#"pub const SERIES_SHADOW_RELEASE_SELECTED_V1: bool = false;
pub const SERIES_SHADOW_SOURCE_MANIFEST_DIGEST_V1: [u8; 32] = [0; 32];
pub const SERIES_SHADOW_BUNDLE_DIGEST_V4: [u8; 32] = [0; 32];
pub const SERIES_SHADOW_SEMANTIC_SOURCE_ID_V1: [u8; 32] = [0; 32];
pub const SERIES_SHADOW_COMPILER_SOURCE_ID_V1: [u8; 32] = [0; 32];
pub const SERIES_SHADOW_TOOLCHAIN_ID_V1: [u8; 32] = [0; 32];
pub const SERIES_SHADOW_CERTIFICATE_ID_V1: [u8; 32] = [0; 32];
pub const SERIES_SHADOW_CAPABILITY_PROGRAM_V4: &[u8] = &[];
pub const SERIES_SHADOW_ACCOUNT_PROFILE_V4: &[u8] = &[];
pub const SERIES_SHADOW_REQUEST_PROFILE_V4: &[u8] = &[];
pub const SERIES_SHADOW_LIFECYCLE_V5: &[u8] = &[];
pub const SERIES_SHADOW_TRANSITION_V4: &[u8] = &[];
pub const SERIES_SHADOW_EFFECT_V4: &[u8] = &[];
pub const SERIES_SHADOW_STRATEGY_V4: &[u8] = &[];
"#
}
