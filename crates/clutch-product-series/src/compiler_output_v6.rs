//! Current compiler manifest for the ProfileV4 x QuoteV5 x AttachmentV5 graph.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, CompiledProductSeriesBundleV6Id, ContentId, EvidenceOnlyRecoveryPolicyId,
    EvidenceOnlyRecoveryPolicyV1, FixedCodec, MarketGenesisProfileV2, MarketGenesisProfileV2Id,
    NativeClaimBasisId, NativeClaimBasisV1, PriceMeasurePolicyV1, PriceMeasurePolicyV1Id,
    ProductTemplateId, ProductTemplateV4, RegistryCapabilityProfileV4Id,
    RegistryCapabilityProjectionV2, Result, SeriesAttachmentPlanV5, SeriesAttachmentPlanV5Id,
    SeriesFundingQuoteV5, SeriesFundingQuoteV5Id, SeriesFundingTermsV2, SeriesFundingTermsV2Id,
    SeriesPlanV5, SeriesPlanV5Id,
};

const MAGIC_V6: [u8; 8] = *b"DCCBNDV6";
const VERSION_V6: u16 = 6;

/// Semantic identity domain for the 47-slot compiler output.
pub const COMPILED_PRODUCT_SERIES_BUNDLE_V6_DOMAIN: &[u8] =
    b"dragons-clutch/compiled-product-series-bundle/v6";
/// Exact canonical bundle width.
pub const COMPILED_PRODUCT_SERIES_BUNDLE_V6_BYTES: usize = 528;

/// Canonical owners consumed by the untrusted V6 bundle assembler.
#[derive(Clone, Copy, Debug)]
pub struct ProductSeriesBundleInputsV6<'a> {
    /// Complete central capability projection derived from ProfileV4.
    pub registry: &'a RegistryCapabilityProjectionV2,
    /// Exact authenticated Source release selected for registration.
    pub source_release_manifest_id: ContentId,
    /// Native claim basis.
    pub basis: &'a NativeClaimBasisV1,
    /// Evidence-only Recovery policy.
    pub recovery: &'a EvidenceOnlyRecoveryPolicyV1,
    /// Relative Product template.
    pub template: &'a ProductTemplateV4,
    /// Quantized price policy.
    pub price_policy: &'a PriceMeasurePolicyV1,
    /// Realm/profile-bound Genesis.
    pub genesis: &'a MarketGenesisProfileV2,
    /// Current 47-slot funding quote.
    pub funding_quote: &'a SeriesFundingQuoteV5,
    /// Current operational attachment.
    pub attachment: &'a SeriesAttachmentPlanV5,
    /// Finite recurring Series.
    pub series: &'a SeriesPlanV5,
    /// Immutable refund and sink ownership.
    pub funding_terms: &'a SeriesFundingTermsV2,
}

/// Assemble a deterministic V6 proposal from the complete owning bodies.
pub fn assemble_compiled_product_series_bundle_v6(
    inputs: ProductSeriesBundleInputsV6<'_>,
) -> Result<CompiledProductSeriesBundleV6> {
    inputs.source_release_manifest_id.validate()?;
    inputs.series.validate_bindings_v5(
        inputs.template,
        inputs.basis,
        inputs.recovery,
        inputs.price_policy,
        inputs.genesis,
        inputs.attachment,
        inputs.registry,
    )?;
    inputs.funding_terms.validate_bindings(
        inputs.series,
        inputs.template,
        inputs.basis,
        inputs.recovery,
        inputs.price_policy,
        inputs.genesis,
        inputs.registry,
    )?;
    inputs.funding_quote.validate()?;
    inputs.attachment.validate()?;
    let funding_quote_id = inputs.funding_quote.id()?;
    if inputs.attachment.funding_quote_id != funding_quote_id {
        return Err(crate::Error::MismatchedArtifact);
    }
    let owners = inputs.registry.semantic_owners;
    let value = CompiledProductSeriesBundleV6 {
        registry_release_id: inputs.registry.registry_release_id,
        capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes(
            inputs.registry.capability_profile_id.bytes(),
        ),
        source_release_manifest_id: inputs.source_release_manifest_id,
        source_plane_contract_id: owners.source_plane_contract_id,
        source_spec_id: owners.source_spec_id,
        summary_program_id: owners.summary_program_id,
        product_compiler_release_id: owners.product_compiler_release_id,
        native_claim_basis_id: inputs.basis.id()?,
        evidence_only_recovery_policy_id: inputs.recovery.id()?,
        product_template_id: inputs.template.id()?,
        price_measure_policy_id: inputs.price_policy.id()?,
        market_genesis_profile_id: inputs.genesis.id()?,
        funding_quote_id,
        attachment_plan_id: inputs.attachment.id()?,
        series_plan_id: inputs.series.id()?,
        funding_terms_id: inputs.funding_terms.id()?,
    };
    value.validate()?;
    Ok(value)
}

/// Exact typed artifact graph emitted by the current untrusted compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledProductSeriesBundleV6 {
    /// Central Registry ReleaseV2.
    pub registry_release_id: ContentId,
    /// Exact current ProfileV4 capability owner.
    pub capability_profile_id: RegistryCapabilityProfileV4Id,
    /// Source release manifest.
    pub source_release_manifest_id: ContentId,
    /// SourcePlane compatibility contract.
    pub source_plane_contract_id: ContentId,
    /// Source specification.
    pub source_spec_id: ContentId,
    /// Summary program.
    pub summary_program_id: ContentId,
    /// Product compiler release.
    pub product_compiler_release_id: ContentId,
    /// Native claim basis.
    pub native_claim_basis_id: NativeClaimBasisId,
    /// Evidence-only Recovery policy.
    pub evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    /// Relative Product template.
    pub product_template_id: ProductTemplateId,
    /// Quantized price policy.
    pub price_measure_policy_id: PriceMeasurePolicyV1Id,
    /// Realm/profile Genesis.
    pub market_genesis_profile_id: MarketGenesisProfileV2Id,
    /// Exact current QuoteV5.
    pub funding_quote_id: SeriesFundingQuoteV5Id,
    /// Exact current AttachmentV5.
    pub attachment_plan_id: SeriesAttachmentPlanV5Id,
    /// Finite Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Funding/refund ownership.
    pub funding_terms_id: SeriesFundingTermsV2Id,
}

impl CompiledProductSeriesBundleV6 {
    fn validate(&self) -> Result<()> {
        for id in [
            self.registry_release_id,
            self.capability_profile_id.content_id(),
            self.source_release_manifest_id,
            self.source_plane_contract_id,
            self.source_spec_id,
            self.summary_program_id,
            self.product_compiler_release_id,
            self.native_claim_basis_id.content_id(),
            self.evidence_only_recovery_policy_id.content_id(),
            self.product_template_id.content_id(),
            self.price_measure_policy_id.content_id(),
            self.market_genesis_profile_id.content_id(),
            self.funding_quote_id.content_id(),
            self.attachment_plan_id.content_id(),
            self.series_plan_id.content_id(),
            self.funding_terms_id.content_id(),
        ] {
            id.validate()?;
        }
        Ok(())
    }

    /// Typed identity of the complete exact V6 graph.
    pub fn id(&self) -> Result<CompiledProductSeriesBundleV6Id> {
        let mut body = [0u8; COMPILED_PRODUCT_SERIES_BUNDLE_V6_BYTES];
        self.encode_into(&mut body)?;
        Ok(CompiledProductSeriesBundleV6Id::from_bytes(
            content_id(COMPILED_PRODUCT_SERIES_BUNDLE_V6_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for CompiledProductSeriesBundleV6 {
    const ENCODED_LEN: usize = COMPILED_PRODUCT_SERIES_BUNDLE_V6_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MAGIC_V6);
        writer.u16(VERSION_V6);
        writer.reserved(6);
        for id in [
            self.registry_release_id,
            self.capability_profile_id.content_id(),
            self.source_release_manifest_id,
            self.source_plane_contract_id,
            self.source_spec_id,
            self.summary_program_id,
            self.product_compiler_release_id,
            self.native_claim_basis_id.content_id(),
            self.evidence_only_recovery_policy_id.content_id(),
            self.product_template_id.content_id(),
            self.price_measure_policy_id.content_id(),
            self.market_genesis_profile_id.content_id(),
            self.funding_quote_id.content_id(),
            self.attachment_plan_id.content_id(),
            self.series_plan_id.content_id(),
            self.funding_terms_id.content_id(),
        ] {
            writer.id(id);
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&MAGIC_V6)?;
        if reader.u16() != VERSION_V6 {
            return Err(crate::Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            registry_release_id: reader.id(),
            capability_profile_id: RegistryCapabilityProfileV4Id::from_bytes(reader.id().bytes()),
            source_release_manifest_id: reader.id(),
            source_plane_contract_id: reader.id(),
            source_spec_id: reader.id(),
            summary_program_id: reader.id(),
            product_compiler_release_id: reader.id(),
            native_claim_basis_id: NativeClaimBasisId::from_bytes(reader.id().bytes()),
            evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes(
                reader.id().bytes(),
            ),
            product_template_id: ProductTemplateId::from_bytes(reader.id().bytes()),
            price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes(reader.id().bytes()),
            market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes(reader.id().bytes()),
            funding_quote_id: SeriesFundingQuoteV5Id::from_bytes(reader.id().bytes()),
            attachment_plan_id: SeriesAttachmentPlanV5Id::from_bytes(reader.id().bytes()),
            series_plan_id: SeriesPlanV5Id::from_bytes(reader.id().bytes()),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes(reader.id().bytes()),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v5_bundle_bytes_cannot_decode_as_v6() {
        let mut historical = [0u8; COMPILED_PRODUCT_SERIES_BUNDLE_V6_BYTES];
        historical[..8].copy_from_slice(b"DCCBNDV5");
        historical[8..10].copy_from_slice(&5u16.to_le_bytes());
        assert!(CompiledProductSeriesBundleV6::decode(&historical).is_err());
    }
}
