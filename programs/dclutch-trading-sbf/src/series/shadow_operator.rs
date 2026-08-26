//! Chain-derived recurring-Series Shadow-AOT request construction.
//!
//! This module has no execution or mutation authority. It consumes the exact
//! Registry/deployment witness already produced by the family-neutral
//! ExecutionStrategy adapter, rejoins it to the selected Series artifacts, and
//! constructs the generic Shadow V3 request in a caller-owned buffer. The
//! common Hot outer remains the sole interpreter, accelerator CPI caller, and
//! state/effect authority.

use dclutch_capability_program_contract::hot_v3::HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3;
use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::{
    shadow_digest_v3::{
        ShadowEffectProjectionV3, ShadowInvocationContextV3, ShadowRuntimeObservationV3,
        candidate_digest_v3, effect_digest_v3, family_request_digest_v3,
        invocation_context_digest_v3, runtime_observations_digest_v3,
    },
    shadow_v3::{
        SHADOW_ACK_SCHEMA_ID_V3, SHADOW_REQUEST_HEADER_BYTES_V3, SHADOW_REQUEST_SCHEMA_ID_V3,
        ShadowArtifactTupleV3, ShadowExecutionDigestsV3, ShadowRequestV3, ShadowRuntimeShapeV3,
    },
    v2::{AcceleratorTransportProfileV2, StrategyDispositionV2},
};
use solana_program::hash::hash;

use crate::execution_strategy_v2::AuthenticatedExecutionStrategyV2;

use super::{
    artifacts_v3::SeriesArtifactBundleV3,
    instruction::{SERIES_ACTION_MAXIMUM_BYTES_V3, SeriesActionV3},
};

/// First exact strategy-owned physical account after the common Hot prefix.
pub const SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3: usize = HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3;
/// Finalized translation Certificate raw record.
pub const SERIES_SHADOW_CERTIFICATE_RAW_ACCOUNT_V3: usize =
    SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3;
/// Vacant staging cursor for the Certificate record.
pub const SERIES_SHADOW_CERTIFICATE_STAGING_ACCOUNT_V3: usize =
    SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3 + 1;
/// Finalized immutable accelerator ArtifactRelease raw record.
pub const SERIES_SHADOW_ARTIFACT_RAW_ACCOUNT_V3: usize =
    SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3 + 2;
/// Vacant staging cursor for the ArtifactRelease record.
pub const SERIES_SHADOW_ARTIFACT_STAGING_ACCOUNT_V3: usize =
    SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3 + 3;
/// Current accelerator Program account authenticated by the ArtifactRelease.
pub const SERIES_SHADOW_ACCELERATOR_PROGRAM_ACCOUNT_V3: usize =
    SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3 + 4;
/// Current accelerator ProgramData account authenticated by the ArtifactRelease.
pub const SERIES_SHADOW_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3: usize =
    SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3 + 5;
/// Trading caller-authority PDA supplied after the six Shadow strategy extras.
pub const SERIES_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3: usize =
    SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3 + 6;
/// First AccountProfile-defined physical runtime account.
pub const SERIES_SHADOW_RUNTIME_ACCOUNTS_START_V3: usize =
    SERIES_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3 + 1;
/// Maximum fixed-buffer width of one generic Series Shadow request.
pub const SERIES_SHADOW_MAXIMUM_REQUEST_BYTES_V3: usize =
    SHADOW_REQUEST_HEADER_BYTES_V3 + SERIES_ACTION_MAXIMUM_BYTES_V3;

/// Stable refusal from chain-derived Shadow selection or construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesShadowOperatorErrorV3 {
    /// The authenticated Strategy selected another disposition or transport.
    Strategy,
    /// Descriptor/Strategy/Certificate/ArtifactRelease facts did not form one tuple.
    Artifact,
    /// A required account/content identity was zero or substituted.
    Identity,
    /// Runtime geometry or the complete family request differed.
    Request,
    /// Caller-owned output buffer had another exact width.
    Buffer,
}

/// Result alias for Series Shadow operator construction.
pub type Result<T> = core::result::Result<T, SeriesShadowOperatorErrorV3>;

/// Exact accelerator deployment and artifact tuple admitted from chain state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowSelectionV3 {
    artifacts: ShadowArtifactTupleV3,
    action: SeriesActionV3,
    family_request_digest: ContentId,
    artifact_release: ContentId,
    accelerator_program: ContentId,
    accelerator_programdata: ContentId,
    accelerator_semantic_release: ContentId,
}

impl SeriesShadowSelectionV3 {
    /// Exact generic interpreter/accelerator artifact tuple.
    pub const fn artifacts(self) -> ShadowArtifactTupleV3 {
        self.artifacts
    }

    /// Action already admitted by the selected Series artifact bundle.
    pub const fn action(self) -> SeriesActionV3 {
        self.action
    }

    /// Finalized immutable ArtifactRelease record identity.
    pub const fn artifact_release(self) -> ContentId {
        self.artifact_release
    }

    /// Current checked accelerator Program identity.
    pub const fn accelerator_program(self) -> ContentId {
        self.accelerator_program
    }

    /// Current checked accelerator ProgramData identity.
    pub const fn accelerator_programdata(self) -> ContentId {
        self.accelerator_programdata
    }

    /// Semantic release implemented by the checked accelerator ELF.
    pub const fn accelerator_semantic_release(self) -> ContentId {
        self.accelerator_semantic_release
    }
}

/// Immutable chain facts for one Shadow call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowRequestContextV3 {
    /// Current immutable ReleaseSet.
    pub release_set: ContentId,
    /// Current logical Market account identity.
    pub market: ContentId,
    /// Current Trading capability-root account identity.
    pub root: ContentId,
    /// Current Registry program identity.
    pub registry_program: ContentId,
    /// Current release-selected Trading program identity.
    pub trading_program: ContentId,
    /// Digest of the exact Trading root prestate.
    pub root_prestate_digest: ContentId,
}

/// Complete common-interpreter transcript used to construct one Shadow request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesShadowInterpreterTranscriptV3<'a> {
    /// Product-authoritative semantic tail count.
    pub tail_count: u32,
    /// Exact AccountProfile-ordered runtime observations.
    pub runtime_observations: &'a [ShadowRuntimeObservationV3<'a>],
    /// Complete interpreted scalar candidate bank.
    pub candidate_scalars: &'a [u64],
    /// Complete interpreted identity candidate bank.
    pub candidate_identities: &'a [[u8; 32]],
    /// Complete interpreted effect projection before CPI or writes.
    pub effect: ShadowEffectProjectionV3<'a>,
}

/// Rejoin the selected Series descriptor to one authenticated Shadow release.
pub fn select_series_shadow_accelerator_v3(
    strategy: AuthenticatedExecutionStrategyV2,
    bundle: SeriesArtifactBundleV3<'_>,
) -> Result<SeriesShadowSelectionV3> {
    if strategy.strategy().disposition() != StrategyDispositionV2::ShadowAot
        || strategy.strategy().transport_profile()
            != Ok(AcceleratorTransportProfileV2::ShadowTranscriptV3)
        || strategy.strategy().request_schema().to_bytes() != SHADOW_REQUEST_SCHEMA_ID_V3
        || strategy.strategy().ack_schema().to_bytes() != SHADOW_ACK_SCHEMA_ID_V3
    {
        return Err(SeriesShadowOperatorErrorV3::Strategy);
    }
    let descriptor_id = content_id(&bundle.descriptor.encode())?;
    if descriptor_id != strategy.capability_program_id()
        || bundle.descriptor != strategy.capability_program()
        || bundle.strategy != strategy.strategy()
    {
        return Err(SeriesShadowOperatorErrorV3::Artifact);
    }
    let certificate = strategy
        .certificate_program_id()
        .ok_or(SeriesShadowOperatorErrorV3::Artifact)?;
    let artifact_release_id = strategy
        .artifact_release_id()
        .ok_or(SeriesShadowOperatorErrorV3::Artifact)?;
    let artifact_release = strategy
        .artifact_release()
        .ok_or(SeriesShadowOperatorErrorV3::Artifact)?;
    let accelerator_program = ContentId::new(artifact_release.program().to_bytes())
        .map_err(|_| SeriesShadowOperatorErrorV3::Identity)?;
    let accelerator_programdata = ContentId::new(artifact_release.programdata())
        .map_err(|_| SeriesShadowOperatorErrorV3::Identity)?;
    Ok(SeriesShadowSelectionV3 {
        artifacts: ShadowArtifactTupleV3 {
            capability_program: descriptor_id,
            account_profile: bundle.descriptor.account_profile(),
            request_profile: bundle.descriptor.request_profile_program(),
            transition: bundle.strategy.transition_program(),
            effect: bundle.descriptor.effect_program(),
            strategy: strategy.strategy_program_id(),
            certificate,
        },
        action: bundle.request.action(),
        family_request_digest: family_request_digest_v3(bundle.request.bytes())
            .map_err(|_| SeriesShadowOperatorErrorV3::Request)?,
        artifact_release: artifact_release_id.content_id(),
        accelerator_program,
        accelerator_programdata,
        accelerator_semantic_release: artifact_release.semantic_release_id(),
    })
}

/// Construct one exact generic Shadow request after chain-derived selection.
pub fn build_series_shadow_request_v3<'a>(
    selection: SeriesShadowSelectionV3,
    context: SeriesShadowRequestContextV3,
    family_request: &'a [u8],
    transcript: SeriesShadowInterpreterTranscriptV3<'a>,
) -> Result<ShadowRequestV3<'a>> {
    if family_request.is_empty() || family_request.len() > SERIES_ACTION_MAXIMUM_BYTES_V3 {
        return Err(SeriesShadowOperatorErrorV3::Request);
    }
    let account_count = u32::try_from(transcript.runtime_observations.len())
        .map_err(|_| SeriesShadowOperatorErrorV3::Request)?;
    let scalar_count = u32::try_from(transcript.candidate_scalars.len())
        .map_err(|_| SeriesShadowOperatorErrorV3::Request)?;
    let identity_count = u32::try_from(transcript.candidate_identities.len())
        .map_err(|_| SeriesShadowOperatorErrorV3::Request)?;
    if transcript.effect.tail_count != transcript.tail_count
        || transcript.effect.output_lamports.len() != transcript.runtime_observations.len()
    {
        return Err(SeriesShadowOperatorErrorV3::Request);
    }
    let shape = ShadowRuntimeShapeV3 {
        tail_count: transcript.tail_count,
        account_count,
        scalar_count,
        identity_count,
    };
    shape
        .validate()
        .map_err(|_| SeriesShadowOperatorErrorV3::Request)?;
    let family_digest = family_request_digest_v3(family_request)
        .map_err(|_| SeriesShadowOperatorErrorV3::Request)?;
    if family_digest != selection.family_request_digest {
        return Err(SeriesShadowOperatorErrorV3::Request);
    }
    let digests = ShadowExecutionDigestsV3 {
        runtime_observations: runtime_observations_digest_v3(transcript.runtime_observations)
            .map_err(|_| SeriesShadowOperatorErrorV3::Request)?,
        family_request: family_digest,
        interpreted_candidate: candidate_digest_v3(
            transcript.tail_count,
            transcript.candidate_scalars,
            transcript.candidate_identities,
        )
        .map_err(|_| SeriesShadowOperatorErrorV3::Request)?,
        interpreted_effect: effect_digest_v3(transcript.effect)
            .map_err(|_| SeriesShadowOperatorErrorV3::Request)?,
    };
    let invocation_context = invocation_context_digest_v3(ShadowInvocationContextV3 {
        release_set: context.release_set,
        market: context.market,
        root: context.root,
        capability_program: selection.artifacts.capability_program,
        selected_action: u32::from(selection.action as u8),
        family_request_digest: family_digest,
        root_prestate_digest: context.root_prestate_digest,
    })
    .map_err(|_| SeriesShadowOperatorErrorV3::Request)?;
    Ok(ShadowRequestV3 {
        release_set: context.release_set,
        market: context.market,
        root: context.root,
        registry_program: context.registry_program,
        trading_program: context.trading_program,
        accelerator_program: selection.accelerator_program,
        artifacts: selection.artifacts,
        invocation_context,
        digests,
        shape,
        family_request,
    })
}

/// Encode one constructed request into its exact caller-owned prefix.
pub fn encode_series_shadow_request_v3(
    request: ShadowRequestV3<'_>,
    output: &mut [u8; SERIES_SHADOW_MAXIMUM_REQUEST_BYTES_V3],
) -> Result<usize> {
    let width = request
        .encoded_len()
        .map_err(|_| SeriesShadowOperatorErrorV3::Buffer)?;
    let destination = output
        .get_mut(..width)
        .ok_or(SeriesShadowOperatorErrorV3::Buffer)?;
    request
        .encode_into(destination)
        .map_err(|_| SeriesShadowOperatorErrorV3::Buffer)?;
    output
        .get_mut(width..)
        .ok_or(SeriesShadowOperatorErrorV3::Buffer)?
        .fill(0);
    Ok(width)
}

fn content_id(bytes: &[u8]) -> Result<ContentId> {
    ContentId::new(hash(bytes).to_bytes()).map_err(|_| SeriesShadowOperatorErrorV3::Identity)
}

const _: () = assert!(SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3 == 38);
const _: () = assert!(SERIES_SHADOW_ACCELERATOR_PROGRAM_ACCOUNT_V3 == 42);
const _: () = assert!(SERIES_SHADOW_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3 == 43);
const _: () = assert!(SERIES_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3 == 44);
const _: () = assert!(SERIES_SHADOW_RUNTIME_ACCOUNTS_START_V3 == 45);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::series::instruction::{SeriesActionV3, encode_series_action_header_v3};

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("content")
    }

    fn selection(family_request: &[u8]) -> SeriesShadowSelectionV3 {
        SeriesShadowSelectionV3 {
            artifacts: ShadowArtifactTupleV3 {
                capability_program: id(1),
                account_profile: id(2),
                request_profile: id(3),
                transition: id(4),
                effect: id(5),
                strategy: id(6),
                certificate: id(7),
            },
            action: SeriesActionV3::Close,
            family_request_digest: family_request_digest_v3(family_request)
                .expect("family request digest"),
            artifact_release: id(8),
            accelerator_program: id(9),
            accelerator_programdata: id(10),
            accelerator_semantic_release: id(11),
        }
    }

    fn context() -> SeriesShadowRequestContextV3 {
        SeriesShadowRequestContextV3 {
            release_set: id(12),
            market: id(13),
            root: id(14),
            registry_program: id(15),
            trading_program: id(16),
            root_prestate_digest: id(17),
        }
    }

    fn close_request() -> [u8; 128] {
        encode_series_action_header_v3(SeriesActionV3::Close, id(24), None, None, 8, 0, 0)
            .expect("canonical Close")
    }

    #[test]
    fn chain_indices_and_transcript_bytes_are_exact() {
        let family = close_request();
        let runtime = [ShadowRuntimeObservationV3 {
            key: [25; 32],
            owner: [26; 32],
            lamports: 27,
            data: b"exact runtime bytes",
            signer: false,
            writable: false,
            executable: false,
        }];
        let scalars = [28_u64];
        let identities = [[29_u8; 32]];
        let output_lamports = [30_u64];
        let effect = ShadowEffectProjectionV3 {
            tail_count: 0,
            output_lamports: &output_lamports,
            request_bank: &[],
            routes: &[],
        };
        let transcript = SeriesShadowInterpreterTranscriptV3 {
            tail_count: 0,
            runtime_observations: &runtime,
            candidate_scalars: &scalars,
            candidate_identities: &identities,
            effect,
        };
        let request =
            build_series_shadow_request_v3(selection(&family), context(), &family, transcript)
                .expect("Shadow request");
        let mut bytes = [0_u8; SERIES_SHADOW_MAXIMUM_REQUEST_BYTES_V3];
        let width = encode_series_shadow_request_v3(request, &mut bytes).expect("encode");
        assert_eq!(width, SHADOW_REQUEST_HEADER_BYTES_V3 + family.len());
        assert_eq!(
            ShadowRequestV3::decode(bytes.get(..width).expect("encoded prefix")),
            Ok(request)
        );
        assert!(
            bytes
                .get(width..)
                .expect("unused tail")
                .iter()
                .all(|b| *b == 0)
        );
        assert_eq!(SERIES_SHADOW_STRATEGY_ACCOUNTS_START_V3, 38);
        assert_eq!(SERIES_SHADOW_ACCELERATOR_PROGRAM_ACCOUNT_V3, 42);
        assert_eq!(SERIES_SHADOW_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3, 43);
        assert_eq!(SERIES_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3, 44);
        assert_eq!(SERIES_SHADOW_RUNTIME_ACCOUNTS_START_V3, 45);
        assert_eq!(
            request.digests.family_request,
            family_request_digest_v3(&family).expect("family digest")
        );
        assert_eq!(
            request.digests.runtime_observations,
            runtime_observations_digest_v3(&runtime).expect("runtime digest")
        );
        assert_eq!(
            request.digests.interpreted_candidate,
            candidate_digest_v3(0, &scalars, &identities).expect("candidate digest")
        );
        assert_eq!(
            request.digests.interpreted_effect,
            effect_digest_v3(effect).expect("effect digest")
        );
        assert_eq!(request.shape.account_count, 1);
        assert_eq!(request.shape.scalar_count, 1);
        assert_eq!(request.shape.identity_count, 1);
    }

    #[test]
    fn empty_family_and_empty_runtime_geometry_refuse() {
        let output_lamports = [1_u64];
        let runtime = [ShadowRuntimeObservationV3 {
            key: [31; 32],
            owner: [32; 32],
            lamports: 33,
            data: &[],
            signer: false,
            writable: false,
            executable: false,
        }];
        let scalars = [34_u64];
        let good = SeriesShadowInterpreterTranscriptV3 {
            tail_count: 0,
            runtime_observations: &runtime,
            candidate_scalars: &scalars,
            candidate_identities: &[],
            effect: ShadowEffectProjectionV3 {
                tail_count: 0,
                output_lamports: &output_lamports,
                request_bank: &[],
                routes: &[],
            },
        };
        let selected_family = close_request();
        assert_eq!(
            build_series_shadow_request_v3(selection(&selected_family), context(), &[], good),
            Err(SeriesShadowOperatorErrorV3::Request)
        );
        let family = close_request();
        let empty = SeriesShadowInterpreterTranscriptV3 {
            tail_count: 0,
            runtime_observations: &[],
            candidate_scalars: &scalars,
            candidate_identities: &[],
            effect: ShadowEffectProjectionV3 {
                tail_count: 0,
                output_lamports: &[],
                request_bank: &[],
                routes: &[],
            },
        };
        assert_eq!(
            build_series_shadow_request_v3(selection(&family), context(), &family, empty),
            Err(SeriesShadowOperatorErrorV3::Request)
        );
    }

    #[test]
    fn selected_bundle_refuses_same_width_family_substitution() {
        let family = close_request();
        let mut substituted = family;
        substituted[127] ^= 1;
        let runtime = [ShadowRuntimeObservationV3 {
            key: [40; 32],
            owner: [41; 32],
            lamports: 42,
            data: &[],
            signer: false,
            writable: false,
            executable: false,
        }];
        let scalars = [43_u64];
        let output_lamports = [44_u64];
        let transcript = SeriesShadowInterpreterTranscriptV3 {
            tail_count: 0,
            runtime_observations: &runtime,
            candidate_scalars: &scalars,
            candidate_identities: &[],
            effect: ShadowEffectProjectionV3 {
                tail_count: 0,
                output_lamports: &output_lamports,
                request_bank: &[],
                routes: &[],
            },
        };
        assert_eq!(
            build_series_shadow_request_v3(selection(&family), context(), &substituted, transcript,),
            Err(SeriesShadowOperatorErrorV3::Request)
        );
    }
}
