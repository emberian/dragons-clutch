#![forbid(unsafe_code)]

//! Emit one **lab-scoped** Series Shadow generated include for measurement.
//!
//! This binary exists to answer a question by measuring it rather than
//! arguing it: *does the embedded ShadowAot certificate identity actually
//! reach the accelerator ELF's bytes?* If it does, the certificate cannot
//! name the deployment that embeds it, because
//! `ArtifactReleaseV1::elf_digest` is then a function of the certificate that
//! is supposed to be a function of it.
//!
//! What this driver is NOT: an operator. It authenticates no finalized
//! record, joins no chain observation, and selects no release.
//! `build_series_shadow_source_v1` is the operator path. Every include this
//! binary writes carries a lab certificate supplied on the command line and
//! is explicitly not release evidence.
//!
//! What it *is* honest about: the descriptor semantics, the LifecycleV5
//! bytes, the account widths and the child requests are all taken from
//! Trading's own canonical `series_consume_selected_release_v4` for the same
//! certificate, so the generator compiles the bundle for the exact release
//! Trading would publish. The driver then requires the generator's
//! independently rebuilt `CapabilityProgramV4` to equal Trading's descriptor
//! byte for byte — the first time those two compilers have been joined.
//!
//! Usage: `series_shadow_lab_include <certificate-hex-32> <out-dir>`

use std::{env, fs, path::PathBuf, process::ExitCode};

use dclutch_market::capability_program::v4::CapabilityProgramV4;
use dclutch_claims::founding_v5::{ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5};
use dclutch_core_contract::ContentId;
use dclutch_series_shadow_bundle_generator::{
    SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4, SeriesShadowBundleSourceV4,
    SeriesShadowDescriptorSemanticsV4, SeriesShadowReleaseSourcesV4,
    compile_series_shadow_source_manifest_v1, emit_series_shadow_generated_include_v1,
};
use dclutch_trading::series::replay::SERIES_TICKET_STATE_BYTES_V3;
use dclutch_trading_sbf::series::{
    account_profile_v4::stamp_series_release_owned_widths_v4,
    artifacts_v3::{
        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
    lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
    release_v4::{SeriesConsumeSelectedReleaseInputV4, series_consume_selected_release_v4},
};
use sha2::{Digest, Sha256};

/// Exact reviewed semantic source the manifest commits to.
const SEMANTIC_SOURCE: &[u8] =
    include_bytes!("../../../../dclutch-trading-sbf/src/series/consume_artifacts_v4.rs");
/// Exact generator source the manifest commits to.
const COMPILER_SOURCE: &[u8] = concat!(
    include_str!("../lib.rs"),
    include_str!("../manifest.rs"),
    include_str!("../source_operator.rs"),
)
.as_bytes();

/// Lab template identity. Not a chain fact; the measurement does not use one.
const LAB_TEMPLATE: [u8; 32] = [1; 32];
/// Lab Template occurrence count: one occurrence, hence an empty proof.
///
/// `series_proof_count_v3(1) == 0`, so the compiled Effect declares no
/// borrowed proof range. That is the canonical single-occurrence Series, and
/// it is the only shape this lab has ever staged.
const LAB_OCCURRENCE_COUNT: u32 = 1;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("series_shadow_lab_include: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let (certificate_hex, out_dir) = match arguments.as_slice() {
        [certificate, directory] => (certificate.clone(), PathBuf::from(directory)),
        _ => {
            return Err("usage: series_shadow_lab_include <certificate-hex-32> <out-dir>".into());
        }
    };
    let certificate_bytes = decode_hex(&certificate_hex)?;
    let certificate = ContentId::new(certificate_bytes)
        .map_err(|_| "certificate identity is zero".to_string())?;

    // The toolchain manifest is a real measurement of this build host, not a
    // label. It is still lab-scoped: a release toolchain manifest is pinned
    // evidence, and this one is whatever ran.
    let toolchain_manifest = format!(
        "lab-toolchain;rustc={};target={};not-release-evidence",
        env::var("SERIES_SHADOW_LAB_RUSTC").unwrap_or_else(|_| "unrecorded".into()),
        env::var("SERIES_SHADOW_LAB_TARGET").unwrap_or_else(|_| "unrecorded".into()),
    );

    // Trading's own canonical release for this exact certificate.
    //
    // Lock, Core and Realize stay opaque lab filler: the Consume emitter only
    // zeroes their root-dependent windows and copies the rest. The Claims
    // request is not filler-shaped — `5fbe0bd8` made the emitter DECODE it and
    // rebuild it as a root-independent transport template, so filler bytes now
    // refuse the whole release with `Effect`. This driver therefore supplies a
    // real, still lab-scoped, `ClaimsFoundingRequestV5`.
    let lock = [0x11_u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
    let core = [0x22_u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3];
    let realize = [0x33_u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3];
    let claims = lab_claims_request()?;
    let child_requests = SeriesConsumeChildRequestsV4 {
        lock: &lock,
        core: &core,
        realize: &realize,
        claims: &claims,
    };
    let observed = [0_u32; SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4];
    let template = ContentId::new(LAB_TEMPLATE).map_err(|_| "template identity".to_string())?;
    let release = series_consume_selected_release_v4(SeriesConsumeSelectedReleaseInputV4 {
        template,
        template_occurrence_count: LAB_OCCURRENCE_COUNT,
        shadow_certificate_program: certificate,
        child_requests,
        observed_data_lengths: &observed,
    })
    .map_err(|error| format!("Trading selected release refused: {error:?}"))?;

    // The generator consumes the widths Trading stamped, not the raw observed
    // array: the two Trading-owned widths are release constants.
    let mut lengths = observed;
    stamp_series_release_owned_widths_v4(
        &mut lengths,
        u32::try_from(SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5)
            .map_err(|_| "root width".to_string())?,
        u32::try_from(SERIES_TICKET_STATE_BYTES_V3).map_err(|_| "ticket width".to_string())?,
    );

    // Descriptor semantics are read back out of Trading's own descriptor, so
    // the accelerator is compiled for the release Trading actually emits.
    let descriptor = CapabilityProgramV4::decode(&release.descriptor)
        .map_err(|error| format!("Trading descriptor did not decode: {error:?}"))?;

    let source = SeriesShadowBundleSourceV4 {
        occurrence_count: LAB_OCCURRENCE_COUNT,
        descriptor: SeriesShadowDescriptorSemanticsV4 {
            kind: descriptor.kind(),
            config_schema: descriptor.config_schema(),
            request_schema: descriptor.request_schema(),
            root_schema: descriptor.root_schema(),
            derivation_policy: descriptor.derivation_policy(),
            capacity_profile: descriptor.capacity_profile(),
            root_state_bytes: descriptor.root_state_bytes(),
        },
        release_sources: SeriesShadowReleaseSourcesV4 {
            semantic_source: SEMANTIC_SOURCE,
            compiler_source: COMPILER_SOURCE,
            toolchain_manifest: toolchain_manifest.as_bytes(),
            certificate,
        },
        lifecycle: &release.lifecycle,
        fixed_data_lengths: &lengths,
        child_requests,
    };

    let manifest = compile_series_shadow_source_manifest_v1(source)
        .map_err(|error| format!("generator refused: {error:?}"))?;
    let include = emit_series_shadow_generated_include_v1(&manifest)
        .map_err(|error| format!("include emitter refused: {error:?}"))?;

    // THE JOIN: two independent compilers, one descriptor. Trading assembled
    // its descriptor from its emitted artifacts; the generator rebuilt one
    // from the same semantics and its own re-emitted artifacts.
    let rebuilt = compiled_capability_program(&manifest)?;
    if rebuilt != release.descriptor.as_slice() {
        return Err(format!(
            "DESCRIPTOR DIVERGENCE: generator rebuilt {} but Trading emitted {}",
            hex(&Sha256::digest(&rebuilt)),
            hex(&Sha256::digest(release.descriptor)),
        ));
    }

    fs::create_dir_all(&out_dir).map_err(|error| format!("create out dir: {error}"))?;
    let include_path = out_dir.join("series_shadow_generated.rs");
    let manifest_path = out_dir.join("series_shadow_source_manifest.bin");
    fs::write(&include_path, &include).map_err(|error| format!("write include: {error}"))?;
    fs::write(&manifest_path, &manifest).map_err(|error| format!("write manifest: {error}"))?;

    println!("certificate_id       {certificate_hex}");
    println!("descriptor_join      OK (generator == Trading, byte for byte)");
    println!(
        "trading_descriptor   {}",
        hex(&Sha256::digest(release.descriptor))
    );
    println!(
        "trading_strategy     {}",
        hex(&Sha256::digest(release.strategy))
    );
    println!("source_manifest      {}", hex(&Sha256::digest(&manifest)));
    println!("generated_include    {}", hex(&Sha256::digest(&include)));
    println!("include_path         {}", include_path.display());
    println!("manifest_path        {}", manifest_path.display());
    Ok(())
}

/// Re-decode the manifest and return its embedded CapabilityProgramV4 bytes.
fn compiled_capability_program(manifest: &[u8]) -> Result<Vec<u8>, String> {
    use dclutch_series_shadow_bundle_generator::SeriesShadowSourceManifestV1;
    let decoded = SeriesShadowSourceManifestV1::decode(manifest)
        .map_err(|error| format!("manifest did not hostile-decode: {error:?}"))?;
    Ok(decoded.generated_bundle().capability_program.to_vec())
}

fn decode_hex(value: &str) -> Result<[u8; 32], String> {
    let trimmed = value.trim();
    if trimmed.len() != 64 {
        return Err(format!(
            "certificate must be 64 hex characters, got {}",
            trimmed.len()
        ));
    }
    let mut output = [0_u8; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        let start = index
            .checked_mul(2)
            .ok_or_else(|| "hex index overflow".to_string())?;
        let end = start
            .checked_add(2)
            .ok_or_else(|| "hex index overflow".to_string())?;
        let pair = trimmed
            .get(start..end)
            .ok_or_else(|| "hex slice out of range".to_string())?;
        *byte = u8::from_str_radix(pair, 16)
            .map_err(|error| format!("bad hex at byte {index}: {error}"))?;
    }
    Ok(output)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

/// One real, lab-scoped `ClaimsFoundingRequestV5` for the Consume emitter.
///
/// Every field is lab filler in the same sense the toolchain manifest is: a
/// distinct nonzero value, not a chain fact. What matters is that the bytes
/// DECODE, because `encode_series_consume_effect_v4_from_requests_atomic`
/// rebuilds this request as a root-independent transport template rather than
/// copying it through. The template zeroes the root-derived coordinates, so
/// the values chosen here do not reach the emitted Claims route bytes.
fn lab_claims_request() -> Result<[u8; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3], String> {
    Ok(ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
        release_set: [1; 32],
        market: [2; 32],
        product_record_digest: [3; 32],
        product_instance_id: [4; 32],
        linked_basis_record_digest: [5; 32],
        semantic_basis_id: [6; 32],
        founder: [7; 32],
        founding_intent_digest: [40; 32],
        aggregate: [9; 32],
        position: [10; 32],
        admission: [11; 32],
        funding_source: [12; 32],
        hoard: [13; 32],
        custody_replay: [14; 32],
        rent_credit: [15; 32],
        rent_program: [16; 32],
        claims_program: [17; 32],
        trading_program: [18; 32],
        custody_request_digest: [41; 32],
        custody_receipt_digest: [42; 32],
        generation: 8,
        claim_count: 3,
        quantity: 3,
        basis_scale: 3,
        pre_source_amount: 9,
        post_source_amount: 0,
        pre_hoard_amount: 0,
        post_hoard_amount: 9,
        pre_custody_revision: 2,
        post_custody_revision: 3,
        aggregate_rent_principal: 30,
        position_rent_principal: 31,
        admission_rent_principal: 32,
        observed_aggregate_lamports: 33,
        observed_position_lamports: 34,
        observed_admission_lamports: 35,
        pre_aggregate_revision: 0,
        post_aggregate_revision: 1,
        pre_position_revision: 0,
        post_position_revision: 1,
    })
    .map_err(|error| format!("lab Claims founding request refused: {error:?}"))?
    .to_bytes())
}
