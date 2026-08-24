//! Typed current Product schedule projection for a Failure session Begin.

use crate::{
    content_id, CompiledScheduleV1, ContentId, Error, MarketInstanceV2Id,
    ProductFailureBeginScheduleProjectionV1Id, Result, SeriesPlanV5Id, MAX_RECOVERY_ATTEMPTS,
};

/// Domain for one complete compiled schedule plus exact compiler provenance.
pub const PRODUCT_FAILURE_BEGIN_SCHEDULE_PROJECTION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product/failure-begin-schedule-projection/v1\0";

/// Exact fixed body width of [`CompiledScheduleV1`].
pub const PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V1: usize = 25 + MAX_RECOVERY_ATTEMPTS * 24;

/// Exact immutable provenance of one current V5 ordinal compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductFailureBeginCompilerProvenanceV1 {
    /// Current Registry ReleaseV2.
    pub registry_release_id: ContentId,
    /// Current Registry ProfileV4.
    pub capability_profile_id: ContentId,
    /// Current compiler BundleV5.
    pub compiler_bundle_id: ContentId,
    /// Exact recurring Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact finite Series ordinal.
    pub ordinal: u32,
    /// Shared full-width Market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Product template used by the compiler.
    pub product_template_id: ContentId,
    /// Native claim basis used by the compiler.
    pub native_claim_basis_id: ContentId,
    /// Evidence-only recovery policy used by the compiler.
    pub recovery_policy_id: ContentId,
    /// Exact price-measure policy used by the compiler.
    pub price_measure_policy_id: ContentId,
    /// Market genesis profile used by the compiler.
    pub market_genesis_profile_id: ContentId,
    /// Current Series AttachmentV4 used by the compiler.
    pub attachment_plan_id: ContentId,
}

impl ProductFailureBeginCompilerProvenanceV1 {
    fn validate(self) -> Result<()> {
        for id in [
            self.registry_release_id,
            self.capability_profile_id,
            self.compiler_bundle_id,
            self.product_template_id,
            self.native_claim_basis_id,
            self.recovery_policy_id,
            self.price_measure_policy_id,
            self.market_genesis_profile_id,
            self.attachment_plan_id,
        ] {
            id.validate()?;
        }
        self.series_plan_id.validate()?;
        self.market_instance_id.validate()
    }
}

/// Derive the typed identity of the full schedule and current compiler graph.
///
/// This identity is a semantic projection, not adapter authority. An SBF
/// consumer must still authenticate every owning artifact and mutable binding.
pub fn derive_product_failure_begin_schedule_projection_v1(
    schedule: CompiledScheduleV1,
    provenance: ProductFailureBeginCompilerProvenanceV1,
) -> Result<ProductFailureBeginScheduleProjectionV1Id> {
    schedule.validate()?;
    provenance.validate()?;
    let body = encode_compiled_schedule_body_v1(schedule);
    let mut preimage = [0_u8; PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V1 + 356];
    preimage[..PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V1].copy_from_slice(&body);
    let mut cursor = PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V1;
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
        provenance.attachment_plan_id.bytes(),
    ] {
        preimage[cursor..cursor + 32].copy_from_slice(&id);
        cursor += 32;
    }
    if cursor != preimage.len() {
        return Err(Error::TrailingBytes);
    }
    Ok(ProductFailureBeginScheduleProjectionV1Id::from_bytes(
        content_id(
            PRODUCT_FAILURE_BEGIN_SCHEDULE_PROJECTION_DOMAIN_V1,
            &preimage,
        )
        .bytes(),
    ))
}

fn encode_compiled_schedule_body_v1(
    schedule: CompiledScheduleV1,
) -> [u8; PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V1] {
    let mut output = [0_u8; PRODUCT_FAILURE_BEGIN_SCHEDULE_BODY_BYTES_V1];
    output[0..8].copy_from_slice(&schedule.start_bucket.to_le_bytes());
    output[8..16].copy_from_slice(&schedule.end_bucket_exclusive.to_le_bytes());
    output[16..24].copy_from_slice(&schedule.primary_maturity_bucket_exclusive.to_le_bytes());
    output[24] = schedule.recovery_attempt_count;
    let mut index = 0_usize;
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
