//! Canonical admitted-AOT invocation transcript and read-only CPI frame.
//!
//! This contract grants no accelerator write or CPI authority. Registry
//! admission, certificate, artifact, and deployment authentication remain in
//! the SVM adapter. The digest merely gives that authenticated adapter and one
//! stateless accelerator an exact common transcript to recompute.

use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::ArtifactReleaseIdV1;
use dclutch_sha256_adapter::digestv;

/// Domain for one authoritative admitted-AOT invocation context.
pub const ADMITTED_INVOCATION_CONTEXT_DOMAIN_V3: &[u8] = b"dclutch:admitted-invocation-context:v3";

/// Caller-authority account in every admitted accelerator CPI.
pub const ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3: usize = 0;
/// Current release-set activation cache.
pub const ADMITTED_ACTIVATION_ACCOUNT_V3: usize = 1;
/// Immutable Registry program.
pub const ADMITTED_REGISTRY_ACCOUNT_V3: usize = 2;
/// Rent sysvar used to reauthenticate finalized records.
pub const ADMITTED_RENT_ACCOUNT_V3: usize = 3;
/// Instructions sysvar exposing the exact top-level Trading request.
pub const ADMITTED_INSTRUCTIONS_ACCOUNT_V3: usize = 4;
/// Current Trading program.
pub const ADMITTED_TRADING_PROGRAM_ACCOUNT_V3: usize = 5;
/// Current Trading ProgramData.
pub const ADMITTED_TRADING_PROGRAMDATA_ACCOUNT_V3: usize = 6;
/// Action-selected CapabilityProgramV3 raw record.
pub const ADMITTED_CAPABILITY_RAW_ACCOUNT_V3: usize = 7;
/// Vacant CapabilityProgramV3 staging cursor.
pub const ADMITTED_CAPABILITY_STAGING_ACCOUNT_V3: usize = 8;
/// Descriptor-selected ExecutionStrategy raw record.
pub const ADMITTED_STRATEGY_RAW_ACCOUNT_V3: usize = 9;
/// Vacant ExecutionStrategy staging cursor.
pub const ADMITTED_STRATEGY_STAGING_ACCOUNT_V3: usize = 10;
/// Strategy-selected Certificate raw record.
pub const ADMITTED_CERTIFICATE_RAW_ACCOUNT_V3: usize = 11;
/// Vacant Certificate staging cursor.
pub const ADMITTED_CERTIFICATE_STAGING_ACCOUNT_V3: usize = 12;
/// Strategy-selected Registry Admission raw record.
pub const ADMITTED_ADMISSION_RAW_ACCOUNT_V3: usize = 13;
/// Vacant Admission staging cursor.
pub const ADMITTED_ADMISSION_STAGING_ACCOUNT_V3: usize = 14;
/// Certificate-selected ArtifactRelease raw record.
pub const ADMITTED_ARTIFACT_RAW_ACCOUNT_V3: usize = 15;
/// Vacant ArtifactRelease staging cursor.
pub const ADMITTED_ARTIFACT_STAGING_ACCOUNT_V3: usize = 16;
/// Immutable accelerator ProgramData.
pub const ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3: usize = 17;
/// First AccountProfile-ordered read-only runtime account.
pub const ADMITTED_RUNTIME_ACCOUNTS_START_V3: usize = 18;

/// Stable refusal from admitted transcript construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmittedTranscriptErrorV3 {
    /// SHA-256 produced the reserved zero content identity.
    ZeroDigest,
}

/// Exact authenticated facts committed by one admitted invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedInvocationContextV3 {
    /// Current immutable release set.
    pub release_set: ContentId,
    /// Logical Core Market.
    pub market: ContentId,
    /// Mutable Trading root key.
    pub root: ContentId,
    /// Immutable Registry program.
    pub registry_program: ContentId,
    /// Current Trading program.
    pub trading_program: ContentId,
    /// Immutable admitted accelerator program.
    pub accelerator_program: ContentId,
    /// Action-selected CapabilityProgramV3.
    pub capability_program: ContentId,
    /// Descriptor-selected AccountProfile.
    pub account_profile: ContentId,
    /// Descriptor-selected RequestProfile.
    pub request_profile: ContentId,
    /// Strategy-selected Transition program.
    pub transition: ContentId,
    /// Descriptor-selected EffectProgram.
    pub effect: ContentId,
    /// Descriptor-selected lifecycle policy.
    pub lifecycle: ContentId,
    /// Descriptor-selected ExecutionStrategy.
    pub strategy: ContentId,
    /// Strategy-selected translation certificate.
    pub certificate: ContentId,
    /// Registry Admission authorizing the exact Certificate.
    pub admission: ContentId,
    /// Immutable ArtifactRelease selected by Certificate.
    pub artifact_release: ArtifactReleaseIdV1,
    /// Manifest-selected immutable config content identity.
    pub config: ContentId,
    /// Core-selected Product graph-root content identity.
    pub product: ContentId,
    /// Product-selected portfolio content identity.
    pub portfolio: ContentId,
    /// Exact authenticated Product-linked basis raw identity.
    pub linked_basis: ContentId,
    /// `family_request_digest_v3` of the exact complete family request.
    ///
    /// Not a bare SHA-256 of the request bytes: it is the domain-separated,
    /// length-prefixed form defined in [`crate::shadow_digest_v3`]. This
    /// distinction is load-bearing, and a doc comment that described the bare
    /// form once made a bare recomputation look correct.
    pub family_request_digest: ContentId,
    /// Digest of the exact AccountProfile-ordered read-only observations.
    pub runtime_observations_digest: ContentId,
    /// SHA-256 of the exact root prestate.
    pub root_prestate_digest: ContentId,
    /// Action selector selected by CapabilityProgramSetV1.
    pub selected_action: u32,
    /// Product-authoritative semantic width.
    pub tail_count: u32,
    /// Exact logical runtime-account count.
    pub account_count: u32,
    /// Exact scalar-bank count.
    pub scalar_count: u32,
    /// Exact identity-bank count.
    pub identity_count: u32,
}

/// Commit one complete authenticated admitted invocation context.
pub fn admitted_invocation_context_digest_v3(
    context: AdmittedInvocationContextV3,
) -> Result<ContentId, AdmittedTranscriptErrorV3> {
    // Every field is fixed-width, so the preimage is a fixed slice list. The
    // identities are borrowed where they sit; only the release id and the five
    // counts are materialized, and they are materialized contiguously so they
    // cost one slice rather than six.
    let artifact_release = context.artifact_release.to_bytes();
    let mut counts = [0_u8; 5 * 4];
    for (index, value) in [
        context.selected_action,
        context.tail_count,
        context.account_count,
        context.scalar_count,
        context.identity_count,
    ]
    .into_iter()
    .enumerate()
    {
        counts
            .get_mut(index * 4..index * 4 + 4)
            .ok_or(AdmittedTranscriptErrorV3::ZeroDigest)?
            .copy_from_slice(&value.to_le_bytes());
    }
    let preimage: [&[u8]; 25] = [
        ADMITTED_INVOCATION_CONTEXT_DOMAIN_V3,
        context.release_set.as_bytes(),
        context.market.as_bytes(),
        context.root.as_bytes(),
        context.registry_program.as_bytes(),
        context.trading_program.as_bytes(),
        context.accelerator_program.as_bytes(),
        context.capability_program.as_bytes(),
        context.account_profile.as_bytes(),
        context.request_profile.as_bytes(),
        context.transition.as_bytes(),
        context.effect.as_bytes(),
        context.lifecycle.as_bytes(),
        context.strategy.as_bytes(),
        context.certificate.as_bytes(),
        context.admission.as_bytes(),
        &artifact_release,
        context.config.as_bytes(),
        context.product.as_bytes(),
        context.portfolio.as_bytes(),
        context.linked_basis.as_bytes(),
        context.family_request_digest.as_bytes(),
        context.runtime_observations_digest.as_bytes(),
        context.root_prestate_digest.as_bytes(),
        &counts,
    ];
    ContentId::new(digestv(&preimage)).map_err(|_| AdmittedTranscriptErrorV3::ZeroDigest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero fixture identity")
    }

    fn context() -> AdmittedInvocationContextV3 {
        AdmittedInvocationContextV3 {
            release_set: id(1),
            market: id(2),
            root: id(3),
            registry_program: id(4),
            trading_program: id(5),
            accelerator_program: id(6),
            capability_program: id(7),
            account_profile: id(8),
            request_profile: id(9),
            transition: id(10),
            effect: id(11),
            lifecycle: id(12),
            strategy: id(13),
            certificate: id(14),
            admission: id(15),
            artifact_release: ArtifactReleaseIdV1::new([16; 32]).expect("artifact"),
            config: id(17),
            product: id(18),
            portfolio: id(19),
            linked_basis: id(20),
            family_request_digest: id(21),
            runtime_observations_digest: id(22),
            root_prestate_digest: id(23),
            selected_action: 24,
            tail_count: 258,
            account_count: 30,
            scalar_count: 64,
            identity_count: 42,
        }
    }

    #[test]
    fn invocation_digest_binds_admission_runtime_and_linked_basis() {
        let canonical = context();
        let digest = admitted_invocation_context_digest_v3(canonical).expect("digest");
        for hostile in [
            AdmittedInvocationContextV3 {
                admission: id(31),
                ..canonical
            },
            AdmittedInvocationContextV3 {
                runtime_observations_digest: id(32),
                ..canonical
            },
            AdmittedInvocationContextV3 {
                linked_basis: id(33),
                ..canonical
            },
            AdmittedInvocationContextV3 {
                tail_count: 257,
                ..canonical
            },
        ] {
            assert_ne!(
                admitted_invocation_context_digest_v3(hostile).expect("hostile digest"),
                digest
            );
        }
    }

    #[test]
    fn cpi_prefix_is_contiguous_and_runtime_readonly_tail_follows() {
        assert_eq!(ADMITTED_CALLER_AUTHORITY_ACCOUNT_V3, 0);
        assert_eq!(ADMITTED_ACTIVATION_ACCOUNT_V3, 1);
        assert_eq!(ADMITTED_ARTIFACT_STAGING_ACCOUNT_V3 + 1, 17);
        assert_eq!(ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3 + 1, 18);
        assert_eq!(ADMITTED_RUNTIME_ACCOUNTS_START_V3, 18);
    }
}
