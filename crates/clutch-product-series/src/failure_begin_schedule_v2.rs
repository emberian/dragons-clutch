//! Typed current Product schedule projection for one Failure session attempt.

use crate::{
    content_id, CompiledProductSeriesBundleV6Id, CompiledScheduleV1, ContentId,
    EvidenceOnlyRecoveryPolicyId, Error, MarketFoundationScheduleV3Id,
    MarketGenesisProfileV2Id, MarketInstanceV2Id, NativeClaimBasisId,
    PriceMeasurePolicyV1Id, ProductFailureBeginScheduleProjectionV2Id, ProductTemplateId,
    Result, SeriesAttachmentPlanV5Id, SeriesFundingQuoteV5Id, SeriesFundingTermsV2Id,
    SeriesPlanV5Id, MAX_RECOVERY_ATTEMPTS,
};

/// Domain for one complete schedule plus exact BundleV6/QuoteV5 provenance.
pub const PRODUCT_FAILURE_BEGIN_SCHEDULE_PROJECTION_DOMAIN_V2: &[u8] =
    b"dragons-clutch/product/failure-begin-schedule-projection/v2\0";

/// Exact canonical byte width of [`CompiledScheduleV1`].
pub const PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V2: usize =
    25 + MAX_RECOVERY_ATTEMPTS * 24;

/// Exact fixed byte width of [`ProductFailureBeginCompilerProvenanceV2`].
pub const PRODUCT_FAILURE_BEGIN_COMPILER_PROVENANCE_BYTES_V2: usize = 493;

/// Immutable current compiler and Failure-quote provenance for one attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductFailureBeginCompilerProvenanceV2 {
    /// Loader-authenticated Registry ReleaseV2.
    pub registry_release_id: ContentId,
    /// Exact Registry ProfileV4.
    pub capability_profile_id: ContentId,
    /// Exact current compiler BundleV6.
    pub compiler_bundle_id: CompiledProductSeriesBundleV6Id,
    /// Exact recurring Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact finite ordinal.
    pub ordinal: u32,
    /// Exact compiled Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Product template used by `compile_ordinal_v6`.
    pub product_template_id: ProductTemplateId,
    /// Native claim basis used by `compile_ordinal_v6`.
    pub native_claim_basis_id: NativeClaimBasisId,
    /// Evidence-only Recovery policy used by `compile_ordinal_v6`.
    pub recovery_policy_id: EvidenceOnlyRecoveryPolicyId,
    /// Quantized price policy used by `compile_ordinal_v6`.
    pub price_measure_policy_id: PriceMeasurePolicyV1Id,
    /// Market genesis profile used by `compile_ordinal_v6`.
    pub market_genesis_profile_id: MarketGenesisProfileV2Id,
    /// Exact immutable Series funding ownership.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Exact current QuoteV5.
    pub funding_quote_id: SeriesFundingQuoteV5Id,
    /// Exact 47-slot schedule inside QuoteV5.
    pub foundation_schedule_id: MarketFoundationScheduleV3Id,
    /// Exact current AttachmentV5.
    pub attachment_plan_id: SeriesAttachmentPlanV5Id,
    /// Exact Failure recovery-quote schedule admitted by QuoteV5.
    pub failure_recovery_quote_schedule_id: ContentId,
    /// Exact attempt row selected for this session.
    pub attempt_index: u8,
    /// Exact repair generation in the selected compiled row.
    pub source_repair_generation: u64,
}

impl ProductFailureBeginCompilerProvenanceV2 {
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

/// Derive the typed current schedule/attempt identity.
///
/// This remains a pure projection. A live adapter must hostile-authenticate
/// RegistryV3, BundleV6, QuoteV5, AttachmentV5, RootV2, LinkV2, and the exact
/// Failure quote receipt before treating the result as authority.
pub fn derive_product_failure_begin_schedule_projection_v2(
    schedule: CompiledScheduleV1,
    provenance: ProductFailureBeginCompilerProvenanceV2,
) -> Result<ProductFailureBeginScheduleProjectionV2Id> {
    schedule.validate()?;
    provenance.validate(schedule)?;
    let schedule_body = encode_compiled_schedule_body_v2(schedule);
    let mut preimage = [0u8;
        PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V2
            + PRODUCT_FAILURE_BEGIN_COMPILER_PROVENANCE_BYTES_V2];
    preimage[..PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V2]
        .copy_from_slice(&schedule_body);
    let mut cursor = PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V2;
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
    Ok(ProductFailureBeginScheduleProjectionV2Id::from_bytes(
        content_id(PRODUCT_FAILURE_BEGIN_SCHEDULE_PROJECTION_DOMAIN_V2, &preimage).bytes(),
    ))
}

fn encode_compiled_schedule_body_v2(
    schedule: CompiledScheduleV1,
) -> [u8; PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V2] {
    let mut output = [0u8; PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V2];
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AbsoluteRecoveryAttemptV1;

    fn id(byte: u8) -> ContentId { ContentId::from_bytes([byte; 32]) }

    fn schedule() -> CompiledScheduleV1 {
        let mut attempts = [AbsoluteRecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        attempts[0] = AbsoluteRecoveryAttemptV1 {
            repair_generation: 7,
            opens_at_bucket: 30,
            closes_at_bucket: 40,
        };
        CompiledScheduleV1 {
            start_bucket: 10,
            end_bucket_exclusive: 20,
            primary_maturity_bucket_exclusive: 25,
            recovery_attempt_count: 1,
            recovery_attempts: attempts,
        }
    }

    fn provenance() -> ProductFailureBeginCompilerProvenanceV2 {
        ProductFailureBeginCompilerProvenanceV2 {
            registry_release_id: id(1),
            capability_profile_id: id(2),
            compiler_bundle_id: CompiledProductSeriesBundleV6Id::from_bytes([3; 32]),
            series_plan_id: SeriesPlanV5Id::from_bytes([4; 32]),
            ordinal: 5,
            market_instance_id: MarketInstanceV2Id::from_bytes([6; 32]),
            product_template_id: ProductTemplateId::from_bytes([7; 32]),
            native_claim_basis_id: NativeClaimBasisId::from_bytes([8; 32]),
            recovery_policy_id: EvidenceOnlyRecoveryPolicyId::from_bytes([9; 32]),
            price_measure_policy_id: PriceMeasurePolicyV1Id::from_bytes([10; 32]),
            market_genesis_profile_id: MarketGenesisProfileV2Id::from_bytes([11; 32]),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([12; 32]),
            funding_quote_id: SeriesFundingQuoteV5Id::from_bytes([13; 32]),
            foundation_schedule_id: MarketFoundationScheduleV3Id::from_bytes([14; 32]),
            attachment_plan_id: SeriesAttachmentPlanV5Id::from_bytes([15; 32]),
            failure_recovery_quote_schedule_id: id(16),
            attempt_index: 0,
            source_repair_generation: 7,
        }
    }

    #[test]
    fn schedule_attempt_and_current_provenance_are_identity_bearing() {
        let original =
            derive_product_failure_begin_schedule_projection_v2(schedule(), provenance()).unwrap();
        let mut attempt = provenance();
        attempt.source_repair_generation = 8;
        assert!(derive_product_failure_begin_schedule_projection_v2(schedule(), attempt).is_err());
        let mut quote = provenance();
        quote.funding_quote_id = SeriesFundingQuoteV5Id::from_bytes([20; 32]);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v2(schedule(), quote).unwrap(),
            original
        );
        let mut bundle = provenance();
        bundle.compiler_bundle_id = CompiledProductSeriesBundleV6Id::from_bytes([21; 32]);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v2(schedule(), bundle).unwrap(),
            original
        );
        let mut attachment = provenance();
        attachment.attachment_plan_id = SeriesAttachmentPlanV5Id::from_bytes([22; 32]);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v2(schedule(), attachment).unwrap(),
            original
        );
        let mut terms = provenance();
        terms.funding_terms_id = SeriesFundingTermsV2Id::from_bytes([23; 32]);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v2(schedule(), terms).unwrap(),
            original
        );
        let mut foundation = provenance();
        foundation.foundation_schedule_id = MarketFoundationScheduleV3Id::from_bytes([24; 32]);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v2(schedule(), foundation).unwrap(),
            original
        );
        let mut failure_quote = provenance();
        failure_quote.failure_recovery_quote_schedule_id = id(25);
        assert_ne!(
            derive_product_failure_begin_schedule_projection_v2(schedule(), failure_quote)
                .unwrap(),
            original
        );
        let mut wrong_row = provenance();
        wrong_row.attempt_index = 1;
        assert!(
            derive_product_failure_begin_schedule_projection_v2(schedule(), wrong_row).is_err()
        );
    }
}
