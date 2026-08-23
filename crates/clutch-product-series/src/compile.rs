use crate::{
    EvidenceOnlyRecoveryPolicyV1, MarketGenesisProfileV1, MarketInstanceId,
    MarketInstancePreimageV1, NativeClaimBasisV1, ProductTemplateV4, Result,
    SeriesAttachmentPlanId, SeriesAttachmentPlanV1, SeriesPlanId, SeriesPlanV4,
    MAX_RECOVERY_ATTEMPTS,
};

/// One absolute evidence-only recovery attempt derived from immutable policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AbsoluteRecoveryAttemptV1 {
    /// Absolute source repair generation.
    pub repair_generation: u64,
    /// Inclusive first eligible bucket.
    pub opens_at_bucket: u64,
    /// Exclusive attempt close bucket.
    pub closes_at_bucket: u64,
}

impl AbsoluteRecoveryAttemptV1 {
    /// Canonical inactive array padding.
    pub const ZERO: Self = Self {
        repair_generation: 0,
        opens_at_bucket: 0,
        closes_at_bucket: 0,
    };
}

/// Absolute primary and finite recovery schedule for one Series ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledScheduleV1 {
    /// Inclusive first raw observation bucket.
    pub start_bucket: u64,
    /// Exclusive primary observation end.
    pub end_bucket_exclusive: u64,
    /// Exclusive primary maturity bucket.
    pub primary_maturity_bucket_exclusive: u64,
    /// Active recovery-attempt count.
    pub recovery_attempt_count: u8,
    /// Absolute active attempts followed by exact zero projection padding.
    pub recovery_attempts: [AbsoluteRecoveryAttemptV1; MAX_RECOVERY_ATTEMPTS],
}

/// Deterministic lowering of one ordinal with Series provenance kept separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledOrdinalV1 {
    /// Series schedule that requested this occurrence.
    pub series_plan_id: SeriesPlanId,
    /// Ordinal within that finite schedule.
    pub ordinal: u32,
    /// Economic market preimage excluding funding and attachments.
    pub market: MarketInstancePreimageV1,
    /// Full-width economic market identity.
    pub market_instance_id: MarketInstanceId,
    /// Operational attachment plan inherited from the Series.
    pub attachment_plan_id: SeriesAttachmentPlanId,
    /// Fully checked absolute source and recovery schedule.
    pub schedule: CompiledScheduleV1,
}

fn compile_schedule(
    template: &ProductTemplateV4,
    recovery: &EvidenceOnlyRecoveryPolicyV1,
    start_bucket: u64,
) -> Result<CompiledScheduleV1> {
    let end_bucket_exclusive = start_bucket
        .checked_add(template.window_span_buckets)
        .ok_or(crate::Error::ArithmeticOverflow)?;
    let primary_maturity_bucket_exclusive = end_bucket_exclusive
        .checked_add(template.primary_maturity_grace_buckets)
        .ok_or(crate::Error::ArithmeticOverflow)?;
    let mut recovery_attempts = [AbsoluteRecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
    let mut index = 0_usize;
    while index < usize::from(recovery.attempt_count) {
        let relative = recovery.attempts[index];
        recovery_attempts[index] = AbsoluteRecoveryAttemptV1 {
            repair_generation: template
                .base_repair_generation
                .checked_add(u64::from(relative.repair_generation_delta))
                .ok_or(crate::Error::ArithmeticOverflow)?,
            opens_at_bucket: primary_maturity_bucket_exclusive
                .checked_add(relative.opens_after_primary_maturity_buckets)
                .ok_or(crate::Error::ArithmeticOverflow)?,
            closes_at_bucket: primary_maturity_bucket_exclusive
                .checked_add(relative.closes_after_primary_maturity_buckets)
                .ok_or(crate::Error::ArithmeticOverflow)?,
        };
        index += 1;
    }
    Ok(CompiledScheduleV1 {
        start_bucket,
        end_bucket_exclusive,
        primary_maturity_bucket_exclusive,
        recovery_attempt_count: recovery.attempt_count,
        recovery_attempts,
    })
}

/// Compile one ordinal after joining every exact immutable artifact.
///
/// `burn_terminal_disposition_registry_value` is supplied by the central
/// registry/release join. This core deliberately does not allocate it.
#[allow(clippy::too_many_arguments)]
pub fn compile_ordinal(
    series: &SeriesPlanV4,
    template: &ProductTemplateV4,
    basis: &NativeClaimBasisV1,
    recovery: &EvidenceOnlyRecoveryPolicyV1,
    genesis: &MarketGenesisProfileV1,
    attachment: &SeriesAttachmentPlanV1,
    burn_terminal_disposition_registry_value: u16,
    ordinal: u32,
) -> Result<CompiledOrdinalV1> {
    series.validate_bindings(
        template,
        basis,
        recovery,
        genesis,
        attachment,
        burn_terminal_disposition_registry_value,
    )?;
    let start_bucket = series.start_bucket(ordinal)?;
    let schedule = compile_schedule(template, recovery, start_bucket)?;
    let market = MarketInstancePreimageV1 {
        product_template_id: template.id()?,
        market_genesis_profile_id: genesis.id()?,
        start_bucket,
        collateral_cap: series.market_collateral_cap,
    };
    market.validate_bindings(template, genesis)?;
    Ok(CompiledOrdinalV1 {
        series_plan_id: series.id()?,
        ordinal,
        market_instance_id: market.id()?,
        market,
        attachment_plan_id: attachment.id()?,
        schedule,
    })
}
