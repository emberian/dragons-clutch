//! Compile-time boundary for one generator-produced Shadow release.
//!
//! The host compiler emits the included Rust bytes from one hostile-validated
//! [`SeriesShadowSourceManifestV1`](https://docs.rs/). This SBF crate never
//! accepts artifact bytes in instruction data. With no explicitly selected
//! include, the module returns `None` and the physical entrypoint must refuse.

use dclutch_core_contract::ContentId;

use crate::evaluator::EmbeddedSeriesShadowBundleV4;

#[allow(missing_docs)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/series_shadow_generated.rs"));
}

/// Stable refusal from the compile-time generated release boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesShadowEmbeddedReleaseErrorV1 {
    /// A selected generator include omitted bytes or emitted a zero identity.
    InvalidGeneratedInclude,
}

/// Exact provenance identities embedded beside one specialized artifact tuple.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowEmbeddedProvenanceV1 {
    /// Digest of the complete hostile-decodable source manifest.
    pub source_manifest: ContentId,
    /// Domain-separated digest of every generated artifact byte.
    pub bundle: ContentId,
    /// Reviewed semantic source identity.
    pub semantic_source: ContentId,
    /// Generator source identity.
    pub compiler_source: ContentId,
    /// Exact pinned toolchain-manifest identity.
    pub toolchain: ContentId,
}

/// One deliberately selected, immutable compile-time Shadow release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedSeriesShadowReleaseV1 {
    /// Exact embedded interpreter/AOT artifact bytes.
    pub bundle: EmbeddedSeriesShadowBundleV4<'static>,
    /// Exact source and deterministic-build identities carried by the ELF.
    pub provenance: SeriesShadowEmbeddedProvenanceV1,
}

/// Return the one selected generated release, or `None` for a fail-closed ELF.
pub fn selected_series_shadow_release_v1()
-> Result<Option<SelectedSeriesShadowReleaseV1>, SeriesShadowEmbeddedReleaseErrorV1> {
    if !generated::SERIES_SHADOW_RELEASE_SELECTED_V1 {
        return Ok(None);
    }
    let artifacts = [
        generated::SERIES_SHADOW_CAPABILITY_PROGRAM_V4,
        generated::SERIES_SHADOW_ACCOUNT_PROFILE_V4,
        generated::SERIES_SHADOW_REQUEST_PROFILE_V4,
        generated::SERIES_SHADOW_LIFECYCLE_V5,
        generated::SERIES_SHADOW_TRANSITION_V4,
        generated::SERIES_SHADOW_EFFECT_V4,
        generated::SERIES_SHADOW_STRATEGY_V4,
    ];
    if artifacts.iter().any(|bytes| bytes.is_empty()) {
        return Err(SeriesShadowEmbeddedReleaseErrorV1::InvalidGeneratedInclude);
    }
    let certificate = identity(generated::SERIES_SHADOW_CERTIFICATE_ID_V1)?;
    Ok(Some(SelectedSeriesShadowReleaseV1 {
        bundle: EmbeddedSeriesShadowBundleV4 {
            capability_program: generated::SERIES_SHADOW_CAPABILITY_PROGRAM_V4,
            account_profile: generated::SERIES_SHADOW_ACCOUNT_PROFILE_V4,
            request_profile: generated::SERIES_SHADOW_REQUEST_PROFILE_V4,
            lifecycle: generated::SERIES_SHADOW_LIFECYCLE_V5,
            transition: generated::SERIES_SHADOW_TRANSITION_V4,
            effect: generated::SERIES_SHADOW_EFFECT_V4,
            strategy: generated::SERIES_SHADOW_STRATEGY_V4,
            certificate,
        },
        provenance: SeriesShadowEmbeddedProvenanceV1 {
            source_manifest: identity(generated::SERIES_SHADOW_SOURCE_MANIFEST_DIGEST_V1)?,
            bundle: identity(generated::SERIES_SHADOW_BUNDLE_DIGEST_V4)?,
            semantic_source: identity(generated::SERIES_SHADOW_SEMANTIC_SOURCE_ID_V1)?,
            compiler_source: identity(generated::SERIES_SHADOW_COMPILER_SOURCE_ID_V1)?,
            toolchain: identity(generated::SERIES_SHADOW_TOOLCHAIN_ID_V1)?,
        },
    }))
}

fn identity(bytes: [u8; 32]) -> Result<ContentId, SeriesShadowEmbeddedReleaseErrorV1> {
    ContentId::new(bytes).map_err(|_| SeriesShadowEmbeddedReleaseErrorV1::InvalidGeneratedInclude)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_selection_matches_the_generated_contract() {
        let selected = selected_series_shadow_release_v1();
        assert!(selected.is_ok());
        assert_eq!(
            selected.ok().flatten().is_some(),
            generated::SERIES_SHADOW_RELEASE_SELECTED_V1
        );
    }
}
