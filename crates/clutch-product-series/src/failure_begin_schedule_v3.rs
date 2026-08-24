//! Current Product schedule projection for one Failure session attempt.

use crate::{
    content_id, CompiledProductSeriesBundleV7Id, CompiledScheduleV1, ContentId,
    EvidenceOnlyRecoveryPolicyId, Error, MarketFoundationScheduleV4Id,
    MarketGenesisProfileV2Id, MarketInstanceV2Id, NativeClaimBasisId,
    PriceMeasurePolicyV1Id, ProductFailureBeginScheduleProjectionV3Id, ProductTemplateId,
    Result, SeriesAttachmentPlanV6Id, SeriesFundingQuoteV6Id, SeriesFundingTermsV2Id,
    SeriesPlanV5Id, MAX_RECOVERY_ATTEMPTS,
};

/// Domain for one schedule plus exact BundleV7/QuoteV6 provenance.
pub const PRODUCT_FAILURE_BEGIN_SCHEDULE_PROJECTION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/product/failure-begin-schedule-projection/v3\0";
/// Exact canonical byte width of [`CompiledScheduleV1`].
pub const PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V3: usize =
    25 + MAX_RECOVERY_ATTEMPTS * 24;
/// Exact fixed byte width of [`ProductFailureBeginCompilerProvenanceV3`].
pub const PRODUCT_FAILURE_BEGIN_COMPILER_PROVENANCE_BYTES_V3: usize = 493;

/// Immutable current compiler and Failure-quote provenance for one attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductFailureBeginCompilerProvenanceV3 {
    /// Loader-authenticated Registry release.
    pub registry_release_id: ContentId,
    /// Exact current capability profile.
    pub capability_profile_id: ContentId,
    /// Exact current compiler bundle.
    pub compiler_bundle_id: CompiledProductSeriesBundleV7Id,
    /// Exact recurring Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact finite ordinal.
    pub ordinal: u32,
    /// Exact compiled Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Product template used by the current compiler.
    pub product_template_id: ProductTemplateId,
    /// Native claim basis used by the current compiler.
    pub native_claim_basis_id: NativeClaimBasisId,
    /// Evidence-only Recovery policy used by the current compiler.
    pub recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    /// Quantized price policy used by the current compiler.
    pub price_measure_policy_id: PriceMeasurePolicyV1Id,
    /// Market genesis profile used by the current compiler.
    pub market_genesis_profile_id: MarketGenesisProfileV2Id,
    /// Immutable Series funding ownership.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Exact current funding quote.
    pub funding_quote_id: SeriesFundingQuoteV6Id,
    /// Exact 50-slot foundation schedule.
    pub foundation_schedule_id: MarketFoundationScheduleV4Id,
    /// Exact current attachment plan.
    pub attachment_plan_id: SeriesAttachmentPlanV6Id,
    /// Failure Recovery quote schedule admitted by QuoteV6.
    pub failure_recovery_quote_schedule_id: ContentId,
    /// Exact attempt row selected for this session.
    pub attempt_index: u8,
    /// Exact repair generation in the selected compiled row.
    pub source_repair_generation: u64,
}

impl ProductFailureBeginCompilerProvenanceV3 {
    fn validate(self, schedule: CompiledScheduleV1) -> Result<()> {
        for id in [
            self.registry_release_id,
            self.capability_profile_id,
            self.failure_recovery_quote_schedule_id,
        ] {
            id.validate()?;
        }
        self.compiler_bundle_id.validate()?;
        self.series_plan_id.validate()?;
        self.market_instance_id.validate()?;
        self.product_template_id.validate()?;
        self.native_claim_basis_id.validate()?;
        self.recovery_policy_id.validate()?;
        self.price_measure_policy_id.validate()?;
        self.market_genesis_profile_id.validate()?;
        self.funding_terms_id.validate()?;
        self.funding_quote_id.validate()?;
        self.foundation_schedule_id.validate()?;
        self.attachment_plan_id.validate()?;
        let index = usize::from(self.attempt_index);
        if self.source_repair_generation == 0
            || index >= usize::from(schedule.recovery_attempt_count)
            || schedule.recovery_attempts[index].repair_generation
                != self.source_repair_generation
        {
            return Err(Error::WrongOrdinal);
        }
        Ok(())
    }
}

/// Derive the exact current schedule/attempt identity.
pub fn derive_product_failure_begin_schedule_projection_v3(
    schedule: CompiledScheduleV1,
    provenance: ProductFailureBeginCompilerProvenanceV3,
) -> Result<ProductFailureBeginScheduleProjectionV3Id> {
    schedule.validate()?;
    provenance.validate(schedule)?;
    let schedule_body = encode_compiled_schedule_body_v3(schedule);
    let mut preimage = [0u8;
        PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V3
            + PRODUCT_FAILURE_BEGIN_COMPILER_PROVENANCE_BYTES_V3];
    preimage[..PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V3]
        .copy_from_slice(&schedule_body);
    let mut cursor = PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V3;
    for id in [
        provenance.registry_release_id.bytes(),
        provenance.capability_profile_id.bytes(),
        provenance.compiler_bundle_id.bytes(),
        provenance.series_plan_id.bytes(),
    ] {
        preimage[cursor..cursor + 32].copy_from_slice(&id);
        cursor += 32;
    }
    preimage[cursor..cursor + 4].copy_from_slice(&provenance.ordinal.to_le_bytes());
    cursor += 4;
    for id in [
        provenance.market_instance_id.bytes(),
        provenance.product_template_id.bytes(),
        provenance.native_claim_basis_id.bytes(),
        provenance.recovery_policy_id.bytes(),
        provenance.price_measure_policy_id.bytes(),
        provenance.market_genesis_profile_id.bytes(),
        provenance.funding_terms_id.bytes(),
        provenance.funding_quote_id.bytes(),
        provenance.foundation_schedule_id.bytes(),
        provenance.attachment_plan_id.bytes(),
        provenance.failure_recovery_quote_schedule_id.bytes(),
    ] {
        preimage[cursor..cursor + 32].copy_from_slice(&id);
        cursor += 32;
    }
    preimage[cursor] = provenance.attempt_index;
    cursor += 1;
    preimage[cursor..cursor + 8]
        .copy_from_slice(&provenance.source_repair_generation.to_le_bytes());
    cursor += 8;
    if cursor != preimage.len() {
        return Err(Error::TrailingBytes);
    }
    Ok(ProductFailureBeginScheduleProjectionV3Id::from_bytes(
        content_id(PRODUCT_FAILURE_BEGIN_SCHEDULE_PROJECTION_DOMAIN_V3, &preimage).bytes(),
    ))
}

fn encode_compiled_schedule_body_v3(
    schedule: CompiledScheduleV1,
) -> [u8; PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V3] {
    let mut output = [0u8; PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V3];
    output[0..8].copy_from_slice(&schedule.start_bucket.to_le_bytes());
    output[8..16].copy_from_slice(&schedule.end_bucket_exclusive.to_le_bytes());
    output[16..24].copy_from_slice(&schedule.primary_maturity_bucket_exclusive.to_le_bytes());
    output[24] = schedule.recovery_attempt_count;
    let mut index = 0usize;
    while index < MAX_RECOVERY_ATTEMPTS {
        let offset = 25 + index * 24;
        let attempt = schedule.recovery_attempts[index];
        output[offset..offset + 8].copy_from_slice(&attempt.repair_generation.to_le_bytes());
        output[offset + 8..offset + 16].copy_from_slice(&attempt.opens_at_bucket.to_le_bytes());
        output[offset + 16..offset + 24].copy_from_slice(&attempt.closes_at_bucket.to_le_bytes());
        index += 1;
    }
    output
}
