//! General's stateless binding to generic Shadow-AOT and candidate transport.
//!
//! The family evaluator owns semantic computation only. It receives exact
//! authenticated readonly views and produces a complete candidate/effect bank
//! or refusal. This module binds those computed bytes to the family-neutral
//! Shadow V3 transcript and the chunked Accelerator V2 acknowledgement. It
//! never writes an account, invokes a child, or treats an AOT result as
//! authority; generic Trading performs strategy resolution and the one atomic
//! effect commit.

use dclutch_core_contract::ContentId;
use dclutch_execution_strategy_contract::{
    shadow_v3::{ShadowAckV3, ShadowArtifactTupleV3, ShadowDispositionV3, ShadowRequestV3},
    v2::{AcceleratorAckV2, AcceleratorRequestV2, ExecutionCandidateV2, StrategyDispositionV2},
};
use sha2::{Digest, Sha256};

use crate::artifacts_v3::{
    GeneralArtifactBytesV3, GeneralArtifactSelectionV3, authenticate_general_artifacts_v3,
};

/// Stable refusal from General Shadow/AOT transcript construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralShadowErrorV3 {
    /// The generic Shadow request hostile-decode refused.
    ShadowRequest,
    /// The complete action-selected General artifacts refused.
    Artifacts,
    /// The selected strategy was not the required Shadow-AOT disposition.
    Strategy,
    /// Artifact tuple, runtime geometry, or invocation coordinates differed.
    Binding,
    /// Family request, observations, candidate, or effect digest differed.
    Digest,
    /// Accelerator V2 request or acknowledgement geometry refused.
    Accelerator,
    /// A checked runtime width overflowed the generic `u32` shape.
    Geometry,
}

/// Result alias for General Shadow/AOT binding.
pub type Result<T> = core::result::Result<T, GeneralShadowErrorV3>;

/// Exact inputs independently recomputed by the stateless General evaluator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralShadowEvaluationV3<'a> {
    /// Exact generic Shadow request bytes supplied by Trading.
    pub shadow_request: &'a [u8],
    /// Selected ProgramSet/config identities.
    pub selection: GeneralArtifactSelectionV3,
    /// Complete finalized General artifacts selected for this action.
    pub artifacts: GeneralArtifactBytesV3<'a>,
    /// Product-authenticated semantic outcome width.
    pub tail_count: u32,
    /// Canonical AccountProfile-ordered readonly observations.
    pub runtime_observations: &'a [u8],
    /// Complete candidate bank computed by the General evaluator.
    pub accelerated_candidate: &'a [u8],
    /// Complete pre-CPI effect projection computed by the General evaluator.
    pub accelerated_effect: &'a [u8],
}

/// Authenticate a Shadow request and acknowledge exact semantic equivalence.
///
/// This function cannot manufacture an interpreted result: candidate/effect
/// bytes are independently computed accelerator outputs, while the expected
/// digests are fixed in Trading's authenticated Shadow request.
pub fn evaluate_general_shadow_v3(input: GeneralShadowEvaluationV3<'_>) -> Result<ShadowAckV3> {
    let shadow = ShadowRequestV3::decode(input.shadow_request)
        .map_err(|_| GeneralShadowErrorV3::ShadowRequest)?;
    let bundle = authenticate_general_artifacts_v3(
        input.selection,
        input.artifacts,
        shadow.family_request,
        input.tail_count,
    )
    .map_err(|_| GeneralShadowErrorV3::Artifacts)?;
    if bundle.strategy.disposition() != StrategyDispositionV2::ShadowAot {
        return Err(GeneralShadowErrorV3::Strategy);
    }
    let certificate = bundle
        .strategy
        .certificate_program()
        .ok_or(GeneralShadowErrorV3::Strategy)?;
    let expected_artifacts = ShadowArtifactTupleV3 {
        capability_program: content(input.artifacts.descriptor)?,
        account_profile: bundle.descriptor.account_profile(),
        request_profile: bundle.descriptor.request_profile_program(),
        transition: bundle.strategy.transition_program(),
        effect: bundle.descriptor.effect_program(),
        strategy: content(input.artifacts.strategy)?,
        certificate,
    };
    let account_count = affine_count(
        bundle.account_profile.fixed_account_count(),
        bundle.account_profile.item_account_stride(),
        input.tail_count,
    )?;
    let scalar_count = affine_count(
        bundle.account_profile.common_scalar_count(),
        bundle.account_profile.item_scalar_stride(),
        input.tail_count,
    )?;
    let identity_count = affine_count(
        bundle.account_profile.common_identity_count(),
        bundle.account_profile.item_identity_stride(),
        input.tail_count,
    )?;
    if shadow.artifacts != expected_artifacts
        || shadow.shape.tail_count != input.tail_count
        || shadow.shape.account_count != account_count
        || shadow.shape.scalar_count != scalar_count
        || shadow.shape.identity_count != identity_count
    {
        return Err(GeneralShadowErrorV3::Binding);
    }
    if shadow.digests.runtime_observations != content(input.runtime_observations)?
        || shadow.digests.family_request != content(shadow.family_request)?
        || shadow.digests.interpreted_candidate != content(input.accelerated_candidate)?
        || shadow.digests.interpreted_effect != content(input.accelerated_effect)?
    {
        return Err(GeneralShadowErrorV3::Digest);
    }
    Ok(ShadowAckV3::accepted(
        shadow,
        content(input.shadow_request)?,
    ))
}

/// Construct one canonical refused acknowledgement after a semantic refusal.
///
/// Callers must first authenticate the same request/artifact boundary used by
/// [`evaluate_general_shadow_v3`]; refusal carries no candidate or effect.
pub fn refuse_general_shadow_v3(shadow_request: &[u8]) -> Result<ShadowAckV3> {
    let shadow =
        ShadowRequestV3::decode(shadow_request).map_err(|_| GeneralShadowErrorV3::ShadowRequest)?;
    Ok(ShadowAckV3::refused(shadow, content(shadow_request)?))
}

/// Exact artifact/runtime binding for generic Accelerator V2 transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralAcceleratorBindingV3 {
    /// Descriptor-selected Strategy content identity.
    pub strategy: ContentId,
    /// Strategy-selected translation certificate identity.
    pub certificate: ContentId,
    /// Action-selected CapabilityProgramV3 identity.
    pub capability_program: ContentId,
    /// Invocation-context digest repeated by every chunk.
    pub invocation_context: ContentId,
    /// Product-authenticated semantic outcome width.
    pub tail_count: u32,
    /// Exact scalar register count.
    pub scalar_count: u32,
    /// Exact identity register count.
    pub identity_count: u32,
}

/// Bind one complete evaluator result to one canonical Accelerator V2 chunk.
///
/// `input_bank` is the complete reconstructed input even when the transport is
/// scratch-backed. `candidate` is the complete output; chunking is purely a
/// physical return-data refinement and never truncates General semantics.
pub fn general_accelerator_ack_v3<'a>(
    request_bytes: &[u8],
    input_bank: &[u8],
    candidate: ExecutionCandidateV2<'a>,
    binding: GeneralAcceleratorBindingV3,
) -> Result<AcceleratorAckV2<'a>> {
    let request = AcceleratorRequestV2::decode(request_bytes)
        .map_err(|_| GeneralShadowErrorV3::Accelerator)?;
    if request.strategy_program() != binding.strategy
        || request.certificate_program() != binding.certificate
        || request.capability_program() != binding.capability_program
        || request.invocation_context() != binding.invocation_context
        || request.tail_count() != binding.tail_count
        || request.scalar_count() != binding.scalar_count
        || request.identity_count() != binding.identity_count
        || request.input_bank_digest() != content(input_bank)?
        || u64::try_from(input_bank.len()).map_err(|_| GeneralShadowErrorV3::Geometry)?
            != request.total_bank_bytes()
    {
        return Err(GeneralShadowErrorV3::Binding);
    }
    let request_digest = content(request_bytes)?;
    match candidate {
        ExecutionCandidateV2::Refused => Ok(AcceleratorAckV2::refused(request, request_digest)),
        ExecutionCandidateV2::Accepted(bank) => {
            if u64::try_from(bank.len()).map_err(|_| GeneralShadowErrorV3::Geometry)?
                != request.total_bank_bytes()
            {
                return Err(GeneralShadowErrorV3::Geometry);
            }
            let start = usize::try_from(request.chunk_offset())
                .map_err(|_| GeneralShadowErrorV3::Geometry)?;
            let remaining = bank
                .len()
                .checked_sub(start)
                .ok_or(GeneralShadowErrorV3::Geometry)?;
            let payload_bytes = remaining
                .min(dclutch_execution_strategy_contract::v2::ACCELERATOR_CHUNK_PAYLOAD_BYTES_V2);
            let payload = bank
                .get(start..start + payload_bytes)
                .ok_or(GeneralShadowErrorV3::Geometry)?;
            AcceleratorAckV2::accepted(request, request_digest, content(bank)?, payload)
                .map_err(|_| GeneralShadowErrorV3::Accelerator)
        }
    }
}

/// Require an accepted Shadow acknowledgement bound to one exact request.
pub fn require_general_shadow_acceptance_v3(
    ack: ShadowAckV3,
    request: ShadowRequestV3<'_>,
    request_digest: ContentId,
    accelerator_program: ContentId,
) -> Result<()> {
    if ack.disposition() != ShadowDispositionV3::Accepted {
        return Err(GeneralShadowErrorV3::Binding);
    }
    ack.validate_for(request, request_digest, accelerator_program)
        .map_err(|_| GeneralShadowErrorV3::Binding)
}

fn affine_count(common: u16, stride: u16, tail_count: u32) -> Result<u32> {
    u32::from(stride)
        .checked_mul(tail_count)
        .and_then(|tail| u32::from(common).checked_add(tail))
        .ok_or(GeneralShadowErrorV3::Geometry)
}

fn content(bytes: &[u8]) -> Result<ContentId> {
    ContentId::new(Sha256::digest(bytes).into()).map_err(|_| GeneralShadowErrorV3::Digest)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;

    use dclutch_execution_strategy_contract::v2::{
        ACCELERATOR_REQUEST_HEADER_BYTES_V2, AcceleratorDispositionV2, AcceleratorRequestV2,
        BankTransportV2, RequestTransportV2, classify_bank_transport_v2,
    };

    use super::*;

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("nonzero fixture identity")
    }

    fn binding(tail_count: u32, scalar_count: u32) -> GeneralAcceleratorBindingV3 {
        GeneralAcceleratorBindingV3 {
            strategy: id(1),
            certificate: id(2),
            capability_program: id(3),
            invocation_context: id(4),
            tail_count,
            scalar_count,
            identity_count: 4,
        }
    }

    fn encoded_request(
        binding: GeneralAcceleratorBindingV3,
        input: &[u8],
        chunk_index: u32,
    ) -> vec::Vec<u8> {
        let transport =
            match classify_bank_transport_v2(binding.scalar_count, binding.identity_count)
                .expect("transport")
            {
                BankTransportV2::InlineReturnData { .. } => RequestTransportV2::Inline,
                BankTransportV2::AuthenticatedScratchPages { .. } => {
                    RequestTransportV2::ScratchPages
                }
            };
        let inline = if transport == RequestTransportV2::Inline {
            input
        } else {
            &[]
        };
        let request = AcceleratorRequestV2::new(
            transport,
            binding.strategy,
            binding.certificate,
            binding.capability_program,
            binding.invocation_context,
            content(input).expect("input digest"),
            binding.tail_count,
            binding.scalar_count,
            binding.identity_count,
            chunk_index,
            inline,
        )
        .expect("request");
        let mut output = vec![0_u8; ACCELERATOR_REQUEST_HEADER_BYTES_V2 + inline.len()];
        request.encode_into(&mut output).expect("encode request");
        output
    }

    fn bank_bytes(binding: GeneralAcceleratorBindingV3) -> usize {
        usize::try_from(
            dclutch_execution_strategy_contract::v2::register_bank_bytes_v2(
                binding.scalar_count,
                binding.identity_count,
            )
            .expect("bank width"),
        )
        .expect("usize bank")
    }

    #[test]
    fn n1_inline_candidate_is_bound_exactly() {
        let binding = binding(1, 12);
        let input = vec![0x11; bank_bytes(binding)];
        let candidate = vec![0x22; bank_bytes(binding)];
        let request = encoded_request(binding, &input, 0);
        let ack = general_accelerator_ack_v3(
            &request,
            &input,
            ExecutionCandidateV2::Accepted(&candidate),
            binding,
        )
        .expect("accepted inline ack");
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Accepted);
        assert_eq!(ack.payload(), candidate);
        assert_eq!(
            ack.total_bank_digest(),
            Some(content(&candidate).expect("digest"))
        );
    }

    #[test]
    fn n258_candidate_uses_exact_nonsemantic_chunks() {
        let binding = binding(258, 269);
        let input = vec![0x33; bank_bytes(binding)];
        let candidate = vec![0x44; bank_bytes(binding)];
        let first_request = encoded_request(binding, &input, 0);
        let first = general_accelerator_ack_v3(
            &first_request,
            &input,
            ExecutionCandidateV2::Accepted(&candidate),
            binding,
        )
        .expect("first chunk");
        assert_eq!(first.payload().len(), 880);
        assert!(first.chunk_count() > 1);

        let last_index = first.chunk_count() - 1;
        let last_request = encoded_request(binding, &input, last_index);
        let last = general_accelerator_ack_v3(
            &last_request,
            &input,
            ExecutionCandidateV2::Accepted(&candidate),
            binding,
        )
        .expect("last chunk");
        assert_eq!(last.chunk_index(), last_index);
        assert!(last.payload().len() < 880);
        assert_eq!(last.total_bank_digest(), first.total_bank_digest());
    }

    #[test]
    fn substituted_input_candidate_width_and_binding_refuse() {
        let binding = binding(258, 269);
        let input = vec![0x51; bank_bytes(binding)];
        let candidate = vec![0x52; bank_bytes(binding)];
        let request = encoded_request(binding, &input, 0);
        let mut substituted = binding;
        substituted.capability_program = id(99);
        assert_eq!(
            general_accelerator_ack_v3(
                &request,
                &input,
                ExecutionCandidateV2::Accepted(&candidate),
                substituted,
            ),
            Err(GeneralShadowErrorV3::Binding)
        );
        assert_eq!(
            general_accelerator_ack_v3(
                &request,
                &input[..input.len() - 1],
                ExecutionCandidateV2::Accepted(&candidate),
                binding,
            ),
            Err(GeneralShadowErrorV3::Binding)
        );
        assert_eq!(
            general_accelerator_ack_v3(
                &request,
                &input,
                ExecutionCandidateV2::Accepted(&candidate[..candidate.len() - 1]),
                binding,
            ),
            Err(GeneralShadowErrorV3::Geometry)
        );
    }

    #[test]
    fn semantic_refusal_has_no_candidate_authority() {
        let binding = binding(1, 12);
        let input = vec![0x61; bank_bytes(binding)];
        let request = encoded_request(binding, &input, 0);
        let ack =
            general_accelerator_ack_v3(&request, &input, ExecutionCandidateV2::Refused, binding)
                .expect("refusal ack");
        assert_eq!(ack.disposition(), AcceleratorDispositionV2::Refused);
        assert!(ack.payload().is_empty());
        assert_eq!(ack.total_bank_digest(), None);
    }
}
