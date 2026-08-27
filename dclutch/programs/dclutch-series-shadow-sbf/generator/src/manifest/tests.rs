use dclutch_account_profile_contract::lifecycle_v3::{
    HEADER_BYTES as LIFECYCLE_HEADER_BYTES_V5, encode::encode_lifecycle_policy_v5_atomic,
};
use dclutch_core_contract::ContentId;
use dclutch_trading_sbf::series::{
    artifacts_v3::{
        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
};

use super::*;

const SEMANTIC_SOURCE: &[u8] =
    include_bytes!("../../../../dclutch-trading-sbf/src/series/consume_artifacts_v4.rs");
const EPHEMERAL_COMPILER_SOURCE_MANIFEST: &[u8] =
    b"test-only:lib.rs+manifest.rs;not release evidence";
const EPHEMERAL_TOOLCHAIN_MANIFEST: &[u8] = b"test-only:rustc-1.89.0;not release evidence";

struct Fixture {
    lifecycle: [u8; LIFECYCLE_HEADER_BYTES_V5],
    lengths: [u32; SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4],
    lock: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    core: [u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3],
    realize: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    claims: [u8; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3],
}

impl Fixture {
    fn new() -> Self {
        let mut lifecycle_scratch = [0_u8; LIFECYCLE_HEADER_BYTES_V5];
        let mut lifecycle = [0_u8; LIFECYCLE_HEADER_BYTES_V5];
        encode_lifecycle_policy_v5_atomic(
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &mut lifecycle_scratch,
            &mut lifecycle,
        )
        .expect("canonical empty LifecycleV5 encodes");
        Self {
            lifecycle,
            lengths: [0_u32; SERIES_SHADOW_FIXED_ACCOUNT_COUNT_V4],
            lock: [0x11; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
            core: [0x22; SERIES_CONSUME_CORE_REQUEST_BYTES_V3],
            realize: [0x33; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
            claims: [0x44; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3],
        }
    }

    fn source(&self) -> SeriesShadowBundleSourceV4<'_> {
        SeriesShadowBundleSourceV4 {
            descriptor: SeriesShadowDescriptorSemanticsV4 {
                kind: identity(1),
                config_schema: identity(2),
                request_schema: identity(3),
                root_schema: identity(4),
                derivation_policy: identity(5),
                capacity_profile: identity(6),
                root_state_bytes: 64,
            },
            release_sources: SeriesShadowReleaseSourcesV4 {
                semantic_source: SEMANTIC_SOURCE,
                compiler_source: EPHEMERAL_COMPILER_SOURCE_MANIFEST,
                toolchain_manifest: EPHEMERAL_TOOLCHAIN_MANIFEST,
                certificate: identity(7),
            },
            lifecycle: &self.lifecycle,
            fixed_data_lengths: &self.lengths,
            child_requests: SeriesConsumeChildRequestsV4 {
                lock: &self.lock,
                core: &self.core,
                realize: &self.realize,
                claims: &self.claims,
            },
        }
    }
}

#[test]
fn exact_manifest_rebuilds_byte_for_byte() {
    let fixture = Fixture::new();
    let manifest = compile_series_shadow_source_manifest_v1(fixture.source())
        .expect("test-only source manifest compiles");
    let decoded = SeriesShadowSourceManifestV1::decode(&manifest)
        .expect("generated manifest hostile-decodes");
    assert_eq!(decoded.bytes(), manifest);
    assert_eq!(decoded.generated_bundle().lifecycle, fixture.lifecycle);
    assert_eq!(
        decoded.semantic_source(),
        content(SEMANTIC_SOURCE).expect("source digest")
    );
    assert_eq!(
        require_deterministic_series_shadow_rebuild_v1(
            &manifest,
            SeriesShadowRebuildSourcesV1 {
                semantic_source: SEMANTIC_SOURCE,
                compiler_source: EPHEMERAL_COMPILER_SOURCE_MANIFEST,
                toolchain_manifest: EPHEMERAL_TOOLCHAIN_MANIFEST,
            },
        ),
        Ok(())
    );
}

#[test]
fn source_and_toolchain_substitution_refuse() {
    let fixture = Fixture::new();
    let manifest = compile_series_shadow_source_manifest_v1(fixture.source())
        .expect("test-only source manifest compiles");
    for sources in [
        SeriesShadowRebuildSourcesV1 {
            semantic_source: b"substituted semantic source",
            compiler_source: EPHEMERAL_COMPILER_SOURCE_MANIFEST,
            toolchain_manifest: EPHEMERAL_TOOLCHAIN_MANIFEST,
        },
        SeriesShadowRebuildSourcesV1 {
            semantic_source: SEMANTIC_SOURCE,
            compiler_source: b"substituted compiler source manifest",
            toolchain_manifest: EPHEMERAL_TOOLCHAIN_MANIFEST,
        },
        SeriesShadowRebuildSourcesV1 {
            semantic_source: SEMANTIC_SOURCE,
            compiler_source: EPHEMERAL_COMPILER_SOURCE_MANIFEST,
            toolchain_manifest: b"substituted toolchain manifest",
        },
    ] {
        assert_eq!(
            require_deterministic_series_shadow_rebuild_v1(&manifest, sources),
            Err(SeriesShadowBundleCompileErrorV4::SourceIdentity)
        );
    }
}

#[test]
fn input_bundle_and_framing_substitution_refuse() {
    let fixture = Fixture::new();
    let manifest = compile_series_shadow_source_manifest_v1(fixture.source())
        .expect("test-only source manifest compiles");

    let mut repeated_core = manifest.clone();
    let repeated_core_offset = CHILD_REQUESTS_OFFSET
        + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
        + SERIES_CONSUME_CORE_REQUEST_BYTES_V3
        + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
        + SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3;
    flip(&mut repeated_core, repeated_core_offset);
    assert_eq!(
        SeriesShadowSourceManifestV1::decode(&repeated_core),
        Err(SeriesShadowBundleCompileErrorV4::Manifest)
    );

    let mut generated_bundle = manifest.clone();
    flip(
        &mut generated_bundle,
        SECTIONS_OFFSET + fixture.lifecycle.len(),
    );
    assert_eq!(
        SeriesShadowSourceManifestV1::decode(&generated_bundle),
        Err(SeriesShadowBundleCompileErrorV4::Manifest)
    );

    let mut trailing = manifest.clone();
    trailing.push(0);
    assert_eq!(
        SeriesShadowSourceManifestV1::decode(&trailing),
        Err(SeriesShadowBundleCompileErrorV4::Manifest)
    );

    let mut fixed_rule = manifest.clone();
    flip(&mut fixed_rule, FIXED_RULES_OFFSET);
    assert_ne!(
        require_deterministic_series_shadow_rebuild_v1(
            &fixed_rule,
            SeriesShadowRebuildSourcesV1 {
                semantic_source: SEMANTIC_SOURCE,
                compiler_source: EPHEMERAL_COMPILER_SOURCE_MANIFEST,
                toolchain_manifest: EPHEMERAL_TOOLCHAIN_MANIFEST,
            },
        ),
        Ok(())
    );

    let mut child_request = manifest;
    flip(&mut child_request, CHILD_REQUESTS_OFFSET);
    assert_ne!(
        require_deterministic_series_shadow_rebuild_v1(
            &child_request,
            SeriesShadowRebuildSourcesV1 {
                semantic_source: SEMANTIC_SOURCE,
                compiler_source: EPHEMERAL_COMPILER_SOURCE_MANIFEST,
                toolchain_manifest: EPHEMERAL_TOOLCHAIN_MANIFEST,
            },
        ),
        Ok(())
    );
}

fn identity(tag: u8) -> ContentId {
    ContentId::new([tag; 32]).expect("test identity is nonzero")
}

fn flip(bytes: &mut [u8], offset: usize) {
    let byte = bytes.get_mut(offset).expect("fixture offset is in bounds");
    *byte ^= 1;
}
