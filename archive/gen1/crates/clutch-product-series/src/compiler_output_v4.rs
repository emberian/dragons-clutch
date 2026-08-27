//! Current compiler manifest for the ProfileV3 × QuoteV4 × AttachmentV4 graph.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, CompiledProductSeriesBundleV4Id, ContentId, EvidenceOnlyRecoveryPolicyId,
    EvidenceOnlyRecoveryPolicyV1, FixedCodec, MarketGenesisProfileV2, MarketGenesisProfileV2Id,
    NativeClaimBasisId, NativeClaimBasisV1, PriceMeasurePolicyV1, PriceMeasurePolicyV1Id,
    ProductTemplateId, ProductTemplateV4, RegistryCapabilityProfileV3Id,
    RegistryCapabilityProjectionV2, Result, SeriesAttachmentPlanV4, SeriesAttachmentPlanV4Id,
    SeriesFundingQuoteV4, SeriesFundingQuoteV4Id, SeriesFundingTermsV2, SeriesFundingTermsV2Id,
    SeriesPlanV5, SeriesPlanV5Id,
};

const MAGIC_V4: [u8; 8] = *b"DCCBNDV4";
const VERSION_V4: u16 = 4;

/// Semantic identity domain for the current compiler output.
pub const COMPILED_PRODUCT_SERIES_BUNDLE_V4_DOMAIN: &[u8] =
    b"dragons-clutch/compiled-product-series-bundle/v4";
/// Exact canonical current bundle width.
pub const COMPILED_PRODUCT_SERIES_BUNDLE_V4_BYTES: usize = 528;

/// Canonical owners consumed by the untrusted V4 bundle assembler.
#[derive(Clone, Copy, Debug)]
pub struct ProductSeriesBundleInputsV4<'a> {
    /// Complete central capability projection derived from ProfileV3.
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
    /// Current 46-slot funding quote.
    pub funding_quote: &'a SeriesFundingQuoteV4,
    /// Current operational attachment.
    pub attachment: &'a SeriesAttachmentPlanV4,
    /// Finite recurring Series.
    pub series: &'a SeriesPlanV5,
    /// Immutable refund and sink ownership.
    pub funding_terms: &'a SeriesFundingTermsV2,
}

/// Assemble a deterministic V4 proposal from the complete owning bodies.
pub fn assemble_compiled_product_series_bundle_v4(
    inputs: ProductSeriesBundleInputsV4<'_>,
) -> Result<CompiledProductSeriesBundleV4> {
    inputs.source_release_manifest_id.validate()?;
    inputs.series.validate_bindings_v4(
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
    let value = CompiledProductSeriesBundleV4 {
        registry_release_id: inputs.registry.registry_release_id,
        capability_profile_id: RegistryCapabilityProfileV3Id::from_bytes(
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

/// Exact typed artifact graph emitted by an untrusted current compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledProductSeriesBundleV4 {
    /// Central Registry release.
    pub registry_release_id: ContentId,
    /// Exact current ProfileV3 capability owner.
    pub capability_profile_id: RegistryCapabilityProfileV3Id,
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
    /// Exact current QuoteV4.
    pub funding_quote_id: SeriesFundingQuoteV4Id,
    /// Exact current AttachmentV4.
    pub attachment_plan_id: SeriesAttachmentPlanV4Id,
    /// Finite Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Funding/refund ownership.
    pub funding_terms_id: SeriesFundingTermsV2Id,
}

impl CompiledProductSeriesBundleV4 {
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

    /// Typed identity of the complete exact V4 graph.
    pub fn id(&self) -> Result<CompiledProductSeriesBundleV4Id> {
        let mut body = [0u8; COMPILED_PRODUCT_SERIES_BUNDLE_V4_BYTES];
        self.encode_into(&mut body)?;
        Ok(CompiledProductSeriesBundleV4Id::from_bytes(
            content_id(COMPILED_PRODUCT_SERIES_BUNDLE_V4_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for CompiledProductSeriesBundleV4 {
    const ENCODED_LEN: usize = COMPILED_PRODUCT_SERIES_BUNDLE_V4_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MAGIC_V4);
        writer.u16(VERSION_V4);
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
        reader.magic(&MAGIC_V4)?;
        if reader.u16() != VERSION_V4 {
            return Err(crate::Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            registry_release_id: reader.id(),
            capability_profile_id: RegistryCapabilityProfileV3Id::from_bytes(reader.id().bytes()),
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
            funding_quote_id: SeriesFundingQuoteV4Id::from_bytes(reader.id().bytes()),
            attachment_plan_id: SeriesAttachmentPlanV4Id::from_bytes(reader.id().bytes()),
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

    fn bundle() -> CompiledProductSeriesBundleV4 {
        let id = |byte| ContentId::from_bytes([byte; 32]);
        CompiledProductSeriesBundleV4 {
            registry_release_id: id(1),
            capability_profile_id: RegistryCapabilityProfileV3Id::from_bytes([2; 32]),
            source_release_manifest_id: id(3),
            source_plane_contract_id: id(4),
            source_spec_id: id(5),
            summary_program_id: id(6),
            product_compiler_release_id: id(7),
            native_claim_basis_id: NativeClaimBasisId::from_bytes([8; 32]),
            evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes([9; 32]),
            product_template_id: ProductTemplateId::from_bytes([10; 32]),
            price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes([11; 32]),
            market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes([12; 32]),
            funding_quote_id: SeriesFundingQuoteV4Id::from_bytes([13; 32]),
            attachment_plan_id: SeriesAttachmentPlanV4Id::from_bytes([14; 32]),
            series_plan_id: SeriesPlanV5Id::from_bytes([15; 32]),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([16; 32]),
        }
    }

    #[test]
    fn v4_bundle_codec_preserves_all_sixteen_typed_edges() {
        let value = bundle();
        let mut bytes = [0u8; COMPILED_PRODUCT_SERIES_BUNDLE_V4_BYTES];
        value.encode_into(&mut bytes).unwrap();
        assert_eq!(CompiledProductSeriesBundleV4::decode(&bytes), Ok(value));
        assert_eq!(&bytes[..8], b"DCCBNDV4");
    }

    #[test]
    fn v4_bundle_refuses_zero_quote_authority() {
        let mut value = bundle();
        value.funding_quote_id = SeriesFundingQuoteV4Id::from_bytes([0; 32]);
        assert_eq!(value.validate(), Err(crate::Error::ZeroIdentity));
    }

    #[test]
    fn v4_bundle_never_reinterprets_the_same_edges_under_v3_coordinates() {
        let mut provisional = [0u8; COMPILED_PRODUCT_SERIES_BUNDLE_V4_BYTES];
        bundle().encode_into(&mut provisional).unwrap();
        provisional[..8].copy_from_slice(b"DCCBNDV3");
        provisional[8..10].copy_from_slice(&3u16.to_le_bytes());
        assert!(crate::CompiledProductSeriesBundleV3::decode(&provisional).is_ok());
        assert_eq!(
            CompiledProductSeriesBundleV4::decode(&provisional),
            Err(crate::Error::BadMagic)
        );
    }
}
