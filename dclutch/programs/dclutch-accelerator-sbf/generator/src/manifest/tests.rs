use dclutch_core_contract::ContentId;
use dclutch_vm::account_profile::lifecycle_v3::{
    HEADER_BYTES as LIFECYCLE_HEADER_BYTES_V5, encode::encode_lifecycle_policy_v5_atomic,
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
        }
    }

    fn source(&self) -> SeriesShadowBundleSourceV4<'_> {
        self.source_with_semantic_release(identity(7))
    }

    fn source_with_semantic_release(
        &self,
        accelerator_semantic_release: ContentId,
    ) -> SeriesShadowBundleSourceV4<'_> {
        SeriesShadowBundleSourceV4 {
            occurrence_count: 1,
            descriptor: SeriesShadowDescriptorSemanticsV4 {
                kind: identity(1),
                config_schema: identity(2),
                request_schema: identity(3),
                root_schema: identity(4),
                derivation_policy: identity(5),
                capacity_profile:
                    dclutch_trading_sbf::series::release_v5::series_consume_capacity_profile_v1(
                        &self.lengths,
                        1,
                        1,
                    )
                    .expect("test capacity"),
                root_state_bytes: 64,
            },
            release_sources: SeriesShadowReleaseSourcesV4 {
                semantic_source: SEMANTIC_SOURCE,
                compiler_source: EPHEMERAL_COMPILER_SOURCE_MANIFEST,
                toolchain_manifest: EPHEMERAL_TOOLCHAIN_MANIFEST,
                accelerator_semantic_release,
                translation_validation: identity(9),
            },
            lifecycle: &self.lifecycle,
            fixed_data_lengths: &self.lengths,
            funding_count: 1,
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

    let mut wrong_funding_count = manifest.clone();
    flip(&mut wrong_funding_count, FUNDING_COUNT_OFFSET);
    assert_eq!(
        SeriesShadowSourceManifestV1::decode(&wrong_funding_count),
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
}

/// The semantic release derives a certificate before strategy or descriptor.
///
/// The semantic release is an independently checked source fact. Changing it
/// changes the generated certificate and every artifact that names that
/// certificate, while the account/request/transition/effect tuple itself
/// stays fixed. This is the acyclic edge that permits an ArtifactRelease to
/// bind the later ELF without weakening its exact-release checks.
#[test]
fn semantic_release_reaches_certificate_and_selected_include_reproducibly() {
    let fixture = Fixture::new();
    let first =
        crate::compile_series_shadow_bundle_v4(fixture.source_with_semantic_release(identity(7)))
            .expect("bundle compiles for the first semantic release");
    let second =
        crate::compile_series_shadow_bundle_v4(fixture.source_with_semantic_release(identity(8)))
            .expect("bundle compiles for the second semantic release");

    assert_ne!(first.certificate_program, second.certificate_program);
    assert_ne!(first.certificate, second.certificate);

    // The certificate is named by the strategy, the strategy is named by the
    // descriptor, and both are inside the complete-bundle digest.
    assert_ne!(first.strategy, second.strategy);
    assert_ne!(first.capability_program, second.capability_program);
    assert_ne!(first.bundle_digest, second.bundle_digest);

    // The certificate tuple excludes strategy and descriptor identity, so the
    // independently emitted contract artifacts stay fixed.
    assert_eq!(first.account_profile, second.account_profile);
    assert_eq!(first.request_profile, second.request_profile);
    assert_eq!(first.transition, second.transition);
    assert_eq!(first.effect, second.effect);
    assert_eq!(first.lifecycle, second.lifecycle);

    // The selected include is deterministic for one source tuple and changes
    // when the independently checked semantic release changes.
    let emit = |semantic_release| {
        let manifest = compile_series_shadow_source_manifest_v1(
            fixture.source_with_semantic_release(semantic_release),
        )
        .expect("manifest compiles");
        crate::emit_series_shadow_generated_include_v1(&manifest).expect("include emits")
    };
    assert_ne!(emit(identity(7)), emit(identity(8)));
    assert_eq!(emit(identity(7)), emit(identity(7)));
}

#[test]
fn wrong_semantic_release_or_contract_artifact_tuple_refuses() {
    let fixture = Fixture::new();
    let manifest = compile_series_shadow_source_manifest_v1(fixture.source())
        .expect("test-only source manifest compiles");

    let mut wrong_semantic_release = manifest.clone();
    flip(
        &mut wrong_semantic_release,
        IDENTITIES_OFFSET + ACCELERATOR_SEMANTIC_RELEASE_IDENTITY * 32,
    );
    assert_eq!(
        SeriesShadowSourceManifestV1::decode(&wrong_semantic_release),
        Err(SeriesShadowBundleCompileErrorV4::Manifest)
    );

    let mut wrong_account_profile = manifest;
    let account_profile_offset = SECTIONS_OFFSET
        + fixture.lifecycle.len()
        + dclutch_market::capability_program::v4::CAPABILITY_PROGRAM_V4_BYTES;
    flip(&mut wrong_account_profile, account_profile_offset);
    assert_eq!(
        SeriesShadowSourceManifestV1::decode(&wrong_account_profile),
        Err(SeriesShadowBundleCompileErrorV4::Manifest)
    );
}

fn identity(tag: u8) -> ContentId {
    ContentId::new([tag; 32]).expect("test identity is nonzero")
}

fn flip(bytes: &mut [u8], offset: usize) {
    let byte = bytes.get_mut(offset).expect("fixture offset is in bounds");
    *byte ^= 1;
}
