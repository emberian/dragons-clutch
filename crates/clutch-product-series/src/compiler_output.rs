//! Canonical typed output manifest for an untrusted Product compiler.
//!
//! The compiler may propose these identities, but registration reopens every
//! referenced artifact, authenticates the Product registry and Source release,
//! and recomputes this complete join. The manifest is therefore a compact
//! compiler target and provenance root, never authority by itself.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, CompiledProductSeriesBundleV1Id, ContentId, EvidenceOnlyRecoveryPolicyId,
    EvidenceOnlyRecoveryPolicyV1, FixedCodec, MarketGenesisProfileV2, MarketGenesisProfileV2Id,
    NativeClaimBasisId, NativeClaimBasisV1, PriceMeasurePolicyV1, PriceMeasurePolicyV1Id,
    ProductTemplateId, ProductTemplateV4, RegistryCapabilityProjectionV2, Result,
    SeriesAttachmentPlanId, SeriesAttachmentPlanV1, SeriesFundingQuoteId, SeriesFundingQuoteV1,
    SeriesFundingTermsV2, SeriesFundingTermsV2Id, SeriesPlanV5, SeriesPlanV5Id,
};

const MAGIC: [u8; 8] = *b"DCCBNDV1";
const VERSION: u16 = 1;

/// Semantic identity domain for [`CompiledProductSeriesBundleV1`].
pub const COMPILED_PRODUCT_SERIES_BUNDLE_V1_DOMAIN: &[u8] =
    b"dragons-clutch/compiled-product-series-bundle/v1";
/// Exact canonical bytes in one compiler-output bundle.
pub const COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES: usize = 528;

/// Complete canonical bodies consumed by the untrusted bundle assembler.
///
/// The registry and Source release values remain adapter projections. This
/// pure value cannot authenticate them; it only prevents the compiler from
/// inventing or repeating IDs that canonical Product artifacts already own.
#[derive(Clone, Copy, Debug)]
pub struct ProductSeriesBundleInputsV1<'a> {
    /// Complete registry capability projection supplied by an adapter/client.
    pub registry: &'a RegistryCapabilityProjectionV2,
    /// Exact Source release manifest selected for later registration.
    pub source_release_manifest_id: ContentId,
    /// Canonical native claim basis body.
    pub basis: &'a NativeClaimBasisV1,
    /// Canonical evidence-only recovery body.
    pub recovery: &'a EvidenceOnlyRecoveryPolicyV1,
    /// Canonical reusable Product template body.
    pub template: &'a ProductTemplateV4,
    /// Canonical quantized price/approximation policy body.
    pub price_policy: &'a PriceMeasurePolicyV1,
    /// Canonical Realm/profile-bound Genesis body.
    pub genesis: &'a MarketGenesisProfileV2,
    /// Canonical component funding quote body.
    pub funding_quote: &'a SeriesFundingQuoteV1,
    /// Canonical operational attachment body.
    pub attachment: &'a SeriesAttachmentPlanV1,
    /// Canonical finite recurring Series body.
    pub series: &'a SeriesPlanV5,
    /// Canonical funding ownership body.
    pub funding_terms: &'a SeriesFundingTermsV2,
}

/// Recompute the complete typed compiler-output bundle from canonical owners.
///
/// This function returns an untrusted proposal. A live registration path must
/// still reopen and authenticate every referenced body and the registry and
/// Source releases before accepting the bundle account.
pub fn assemble_compiled_product_series_bundle_v1(
    inputs: ProductSeriesBundleInputsV1<'_>,
) -> Result<CompiledProductSeriesBundleV1> {
    inputs.source_release_manifest_id.validate()?;
    inputs.series.validate_bindings(
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
    inputs
        .funding_quote
        .validate_recovery_binding(inputs.recovery)?;
    inputs.attachment.validate()?;
    let funding_quote_id = inputs.funding_quote.id()?;
    if inputs.attachment.funding_quote_id != funding_quote_id {
        return Err(crate::Error::MismatchedArtifact);
    }

    let owners = inputs.registry.semantic_owners;
    let value = CompiledProductSeriesBundleV1 {
        registry_release_id: inputs.registry.registry_release_id,
        capability_profile_id: inputs.registry.capability_profile_id,
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

/// Exact typed artifact graph emitted by an untrusted Product compiler.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledProductSeriesBundleV1 {
    /// Authoritative Product capability registry release selected at registration.
    pub registry_release_id: ContentId,
    /// Exact capability profile under which the compiler output is executable.
    pub capability_profile_id: ContentId,
    /// Authoritative Source release manifest selected at registration.
    pub source_release_manifest_id: ContentId,
    /// Exact SourcePlane compatibility contract named by the recurring template.
    pub source_plane_contract_id: ContentId,
    /// Exact immutable SourceSpec named by the recurring template.
    pub source_spec_id: ContentId,
    /// Exact source-neutral summary program named by the recurring template.
    pub summary_program_id: ContentId,
    /// Exact compiler algorithm/release identity named by the template.
    pub product_compiler_release_id: ContentId,
    /// Canonical payoff basis identity.
    pub native_claim_basis_id: NativeClaimBasisId,
    /// Canonical evidence-only failure/recovery policy identity.
    pub evidence_only_recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    /// Reusable relative Product template identity.
    pub product_template_id: ProductTemplateId,
    /// Exact quantized price/payoff-approximation policy identity.
    pub price_measure_policy_id: PriceMeasurePolicyV1Id,
    /// Realm/profile-bound market genesis identity.
    pub market_genesis_profile_id: MarketGenesisProfileV2Id,
    /// Exact per-occurrence funding quote identity.
    pub funding_quote_id: SeriesFundingQuoteId,
    /// Operational attachment identity.
    pub attachment_plan_id: SeriesAttachmentPlanId,
    /// Finite recurring Series identity.
    pub series_plan_id: SeriesPlanV5Id,
    /// Immutable refund/sink/collateral ownership terms identity.
    pub funding_terms_id: SeriesFundingTermsV2Id,
}

impl CompiledProductSeriesBundleV1 {
    fn validate(&self) -> Result<()> {
        self.registry_release_id.validate()?;
        self.capability_profile_id.validate()?;
        self.source_release_manifest_id.validate()?;
        self.source_plane_contract_id.validate()?;
        self.source_spec_id.validate()?;
        self.summary_program_id.validate()?;
        self.product_compiler_release_id.validate()?;
        self.native_claim_basis_id.validate()?;
        self.evidence_only_recovery_policy_id.validate()?;
        self.product_template_id.validate()?;
        self.price_measure_policy_id.validate()?;
        self.market_genesis_profile_id.validate()?;
        self.funding_quote_id.validate()?;
        self.attachment_plan_id.validate()?;
        self.series_plan_id.validate()?;
        self.funding_terms_id.validate()
    }

    /// Exact semantic identity of this compiler output graph.
    pub fn id(&self) -> Result<CompiledProductSeriesBundleV1Id> {
        let mut body = [0; COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES];
        self.encode_into(&mut body)?;
        Ok(CompiledProductSeriesBundleV1Id::from_bytes(
            content_id(COMPILED_PRODUCT_SERIES_BUNDLE_V1_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for CompiledProductSeriesBundleV1 {
    const ENCODED_LEN: usize = COMPILED_PRODUCT_SERIES_BUNDLE_V1_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&MAGIC);
        writer.u16(VERSION);
        writer.reserved(6);
        for id in [
            self.registry_release_id,
            self.capability_profile_id,
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
        reader.magic(&MAGIC)?;
        if reader.u16() != VERSION {
            return Err(crate::Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            registry_release_id: reader.id(),
            capability_profile_id: reader.id(),
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
            funding_quote_id: SeriesFundingQuoteId::from_bytes(reader.id().bytes()),
            attachment_plan_id: SeriesAttachmentPlanId::from_bytes(reader.id().bytes()),
            series_plan_id: SeriesPlanV5Id::from_bytes(reader.id().bytes()),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes(reader.id().bytes()),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}
