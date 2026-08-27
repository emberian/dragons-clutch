use crate::codec::{Reader, Writer};
use crate::source::{
    SourcePlaneProgramV3, StatisticKeyV3, StatisticKindV3, SummaryProgramV3, WindowSpecV3,
    COVERAGE_COMPLETE_REQUIRED,
};
use crate::{content_id, ContentId, Error, FixedCodec, Result};

const PAYOUT_MAGIC: [u8; 8] = *b"DCPAYTV3";
const TEMPLATE_MAGIC: [u8; 8] = *b"DCTMPLV3";
const WORK_MAGIC: [u8; 8] = *b"DCWORKV3";
const LIQUIDITY_MAGIC: [u8; 8] = *b"DCLIQEV3";
const SERIES_MAGIC: [u8; 8] = *b"DCSERIV3";
const INSTANCE_MAGIC: [u8; 8] = *b"DCINSTV3";
const FUNDING_MAGIC: [u8; 8] = *b"DCFUNDV3";

const PAYOUT_TABLE_DOMAIN: &[u8] = b"dragons-clutch/payout-table/v3";
const TEMPLATE_DOMAIN: &[u8] = b"dragons-clutch/product-template/v3";
const WORK_DOMAIN: &[u8] = b"dragons-clutch/work-envelope/v3";
const LIQUIDITY_DOMAIN: &[u8] = b"dragons-clutch/liquidity-envelope/v3";
const SERIES_DOMAIN: &[u8] = b"dragons-clutch/series-plan/v3";
const INSTANCE_DOMAIN: &[u8] = b"dragons-clutch/instance/v3";

/// Maximum native outcome count in the fixed compiler interface.
pub const MAX_OUTCOMES: usize = 16;
/// Maximum immutable payout-vector count.
pub const MAX_PAYOUTS: usize = 8;
/// Maximum finite recurring Instance count.
pub const MAX_SERIES_INSTANCES: u32 = 65_536;
/// Registered `FAIL_UNIFORM_REFUND_01` policy.
pub const FAILURE_UNIFORM_REFUND_01: u32 = 1;
/// Registered `FAIL_EXTENDED_WINDOW_02` policy.
pub const EXTENDED_WINDOW_02: u32 = 2;
/// Exact generation policy implemented by this core.
pub const REPAIR_EXACT_01: u32 = 1;

const PAYOUT_VECTOR_BYTES: usize = 8 + MAX_OUTCOMES * 8;
/// Exact PayoutTable V3 width.
pub const PAYOUT_TABLE_BYTES: usize = 16 + MAX_PAYOUTS * PAYOUT_VECTOR_BYTES;
/// Exact ProductTemplate V3 width.
pub const PRODUCT_TEMPLATE_BYTES: usize = 248;
/// Exact WorkEnvelope V3 width.
pub const WORK_ENVELOPE_BYTES: usize = 32;
/// Exact LiquidityEnvelope V3 width.
pub const LIQUIDITY_ENVELOPE_BYTES: usize = 56;
/// Exact SeriesPlan V3 width.
pub const SERIES_PLAN_BYTES: usize = 272;
/// Exact InstanceDescriptor V3 width.
pub const INSTANCE_DESCRIPTOR_BYTES: usize = 248;
/// Exact mutable prepaid Series funding width.
pub const SERIES_FUNDING_BYTES: usize = 72;

/// Authenticated projection of the canonical partition artifact.
///
/// This view is not persisted by the compiler and cannot authenticate itself.
/// Its adapter must first prove the partition is exhaustive, disjoint, ordered,
/// canonical, and content-bound, then project only these join facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PartitionViewV3 {
    /// Canonical partition artifact identity.
    pub partition_id: ContentId,
    /// Number of liabilities the partition permits.
    pub outcome_count: u8,
}

impl PartitionViewV3 {
    /// Validate the bounded projection shape.
    pub fn validate(&self) -> Result<()> {
        self.partition_id.validate()?;
        if !(2..=u8::try_from(MAX_OUTCOMES).map_err(|_| Error::InvalidParameter)?)
            .contains(&self.outcome_count)
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// One exact integer-simplex payout vector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoutVectorV3 {
    /// Shared positive integer denominator.
    pub denominator: u64,
    /// Active weights followed by exact zero padding.
    pub weights: [u64; MAX_OUTCOMES],
}

impl PayoutVectorV3 {
    /// Exact inactive payout-vector padding.
    pub const ZERO: Self = Self {
        denominator: 0,
        weights: [0; MAX_OUTCOMES],
    };

    fn validate_active(&self, outcome_count: u8, common_denominator: u64) -> Result<()> {
        if common_denominator == 0 || self.denominator != common_denominator {
            return Err(Error::InvalidParameter);
        }
        let count = usize::from(outcome_count);
        let mut sum = 0_u64;
        let mut index = 0_usize;
        while index < MAX_OUTCOMES {
            let weight = self.weights[index];
            if index < count {
                if weight > common_denominator {
                    return Err(Error::InvalidParameter);
                }
                sum = sum.checked_add(weight).ok_or(Error::ArithmeticOverflow)?;
            } else if weight != 0 {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if sum != common_denominator {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }
}

/// Canonical owner of the immutable payout vectors referenced by a Template.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayoutTableV3 {
    /// Active outcome count.
    pub outcome_count: u8,
    /// Active payout-vector count.
    pub payout_count: u8,
    /// Vector selected by the registered failure policy.
    pub failure_payout_index: u8,
    /// Active payout vectors followed by exact zero padding.
    pub payouts: [PayoutVectorV3; MAX_PAYOUTS],
}

impl PayoutTableV3 {
    /// Validate simplex arithmetic and all fixed-array padding.
    pub fn validate(&self) -> Result<()> {
        if !(2..=u8::try_from(MAX_OUTCOMES).map_err(|_| Error::InvalidParameter)?)
            .contains(&self.outcome_count)
            || self.payout_count == 0
            || usize::from(self.payout_count) > MAX_PAYOUTS
            || self.failure_payout_index >= self.payout_count
        {
            return Err(Error::InvalidParameter);
        }
        let denominator = self.payouts[0].denominator;
        let mut index = 0_usize;
        while index < MAX_PAYOUTS {
            if index < usize::from(self.payout_count) {
                self.payouts[index].validate_active(self.outcome_count, denominator)?;
            } else if self.payouts[index] != PayoutVectorV3::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        Ok(())
    }

    /// Explicitly validate the selected registered failure semantics.
    ///
    /// In particular, the name `FAIL_UNIFORM_REFUND_01` is not accepted as a
    /// substitute for validating the selected vector's actual weights.
    pub fn validate_failure_policy(&self, failure_policy_id: u32) -> Result<()> {
        self.validate()?;
        match failure_policy_id {
            FAILURE_UNIFORM_REFUND_01 => {
                let vector = self.payouts[usize::from(self.failure_payout_index)];
                let count = u64::from(self.outcome_count);
                if !vector.denominator.is_multiple_of(count) {
                    return Err(Error::FailurePayoutNotUniform);
                }
                let expected = vector.denominator / count;
                if expected == 0
                    || vector.weights[..usize::from(self.outcome_count)]
                        .iter()
                        .any(|weight| *weight != expected)
                {
                    return Err(Error::FailurePayoutNotUniform);
                }
                Ok(())
            }
            // V3 has no immutable successor-window chain or extension count,
            // so accepting policy 2 would record semantics its bytes cannot execute.
            EXTENDED_WINDOW_02 => Err(Error::UnsupportedPolicy),
            _ => Err(Error::UnsupportedPolicy),
        }
    }

    /// Content identity of the unique immutable payout table.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; PAYOUT_TABLE_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(PAYOUT_TABLE_DOMAIN, &bytes))
    }
}

impl FixedCodec for PayoutTableV3 {
    const ENCODED_LEN: usize = PAYOUT_TABLE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&PAYOUT_MAGIC);
        writer.u8(self.outcome_count);
        writer.u8(self.payout_count);
        writer.u8(self.failure_payout_index);
        writer.reserved(5);
        for payout in self.payouts {
            writer.u64(payout.denominator);
            for weight in payout.weights {
                writer.u64(weight);
            }
        }
        writer.finish()?;
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&PAYOUT_MAGIC)?;
        let outcome_count = reader.u8();
        let payout_count = reader.u8();
        let failure_payout_index = reader.u8();
        reader.reserved(5)?;
        let mut payouts = [PayoutVectorV3::ZERO; MAX_PAYOUTS];
        let mut payout_index = 0;
        while payout_index < MAX_PAYOUTS {
            let denominator = reader.u64();
            let mut weights = [0; MAX_OUTCOMES];
            let mut outcome_index = 0;
            while outcome_index < MAX_OUTCOMES {
                weights[outcome_index] = reader.u64();
                outcome_index += 1;
            }
            payouts[payout_index] = PayoutVectorV3 {
                denominator,
                weights,
            };
            payout_index += 1;
        }
        reader.finish()?;
        let value = Self {
            outcome_count,
            payout_count,
            failure_payout_index,
            payouts,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Reusable product semantics that contain no absolute window or liability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductTemplateV3 {
    /// Exact reviewed SourcePlane contract.
    pub source_plane_program_id: ContentId,
    /// Existing externally authenticated SourceSpec identity.
    pub source_spec_id: ContentId,
    /// Exact source-neutral summary/evaluator program.
    pub summary_program_id: ContentId,
    /// Immutable exhaustive/disjoint/ordered/canonical partition artifact.
    pub partition_id: ContentId,
    /// Immutable payout table artifact.
    pub payout_table_id: ContentId,
    /// Remaining immutable settlement/policy bundle artifact.
    pub settlement_policy_id: ContentId,
    /// Exact compiler semantics version.
    pub compiler_version: u16,
    /// Closed source statistic.
    pub statistic: StatisticKindV3,
    /// Registered raw-window coverage policy.
    pub coverage_policy_id: u16,
    /// Registered failure policy.
    pub failure_policy_id: u32,
    /// Exact repair-generation selection policy.
    pub repair_policy_id: u32,
    /// Number of raw buckets in each Instance window.
    pub window_span_buckets: u64,
    /// Additional raw buckets required before sealing.
    pub maturity_grace_buckets: u64,
    /// Exact selected repair generation; zero is the valid original generation.
    pub repair_generation: u64,
    /// Registered coverage-policy parameter.
    pub coverage_policy_parameter: u64,
}

impl ProductTemplateV3 {
    /// Validate local shape without pretending referenced objects were supplied.
    pub fn validate_shape(&self) -> Result<()> {
        self.source_plane_program_id.validate()?;
        self.source_spec_id.validate()?;
        self.summary_program_id.validate()?;
        self.partition_id.validate()?;
        self.payout_table_id.validate()?;
        self.settlement_policy_id.validate()?;
        if self.compiler_version != 1
            || self.window_span_buckets == 0
            || self.repair_policy_id != REPAIR_EXACT_01
            || self.failure_policy_id != FAILURE_UNIFORM_REFUND_01
        {
            return Err(Error::UnsupportedPolicy);
        }
        self.window_span_buckets
            .checked_add(self.maturity_grace_buckets)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.statistic == StatisticKindV3::MaximumDrawdownInterval
            && (self.coverage_policy_id != COVERAGE_COMPLETE_REQUIRED
                || self.coverage_policy_parameter != 0)
        {
            return Err(Error::InvalidParameter);
        }
        // Reuse WindowSpec's closed coverage registry with harmless bounds.
        WindowSpecV3 {
            source_spec_id: self.source_spec_id,
            source_plane_program_id: self.source_plane_program_id,
            start_bucket: 1,
            end_bucket_exclusive: self
                .window_span_buckets
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            maturity_bucket_exclusive: self
                .window_span_buckets
                .checked_add(self.maturity_grace_buckets)
                .and_then(|value| value.checked_add(1))
                .ok_or(Error::ArithmeticOverflow)?,
            repair_generation: self.repair_generation,
            coverage_policy_id: self.coverage_policy_id,
            coverage_policy_parameter: self.coverage_policy_parameter,
        }
        .validate()?;
        Ok(())
    }

    /// Validate every content binding, supported feature, and failure vector.
    pub fn validate_bindings(
        &self,
        source_plane: &SourcePlaneProgramV3,
        summary: &SummaryProgramV3,
        payouts: &PayoutTableV3,
        partition: &PartitionViewV3,
    ) -> Result<()> {
        self.validate_shape()?;
        source_plane.validate()?;
        summary.validate()?;
        partition.validate()?;
        payouts.validate_failure_policy(self.failure_policy_id)?;
        if self.source_plane_program_id != source_plane.id()?
            || self.summary_program_id != summary.id()?
            || self.payout_table_id != payouts.id()?
            || self.partition_id != partition.partition_id
            || payouts.outcome_count != partition.outcome_count
        {
            return Err(Error::MismatchedArtifact);
        }
        if !summary.supports(self.statistic) {
            return Err(Error::UnsupportedStatistic);
        }
        Ok(())
    }

    /// Content identity of exact Template bytes. Creation must also call
    /// [`Self::validate_bindings`] before this identity may mint liabilities.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; PRODUCT_TEMPLATE_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(TEMPLATE_DOMAIN, &bytes))
    }
}

impl FixedCodec for ProductTemplateV3 {
    const ENCODED_LEN: usize = PRODUCT_TEMPLATE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&TEMPLATE_MAGIC);
        writer.id(self.source_plane_program_id);
        writer.id(self.source_spec_id);
        writer.id(self.summary_program_id);
        writer.id(self.partition_id);
        writer.id(self.payout_table_id);
        writer.id(self.settlement_policy_id);
        writer.u16(self.compiler_version);
        writer.u16(self.statistic as u16);
        writer.u16(self.coverage_policy_id);
        writer.reserved(2);
        writer.u32(self.failure_policy_id);
        writer.u32(self.repair_policy_id);
        writer.u64(self.window_span_buckets);
        writer.u64(self.maturity_grace_buckets);
        writer.u64(self.repair_generation);
        writer.u64(self.coverage_policy_parameter);
        writer.finish()?;
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&TEMPLATE_MAGIC)?;
        let source_plane_program_id = reader.id();
        let source_spec_id = reader.id();
        let summary_program_id = reader.id();
        let partition_id = reader.id();
        let payout_table_id = reader.id();
        let settlement_policy_id = reader.id();
        let compiler_version = reader.u16();
        let statistic = match reader.u16() {
            1 => StatisticKindV3::TerminalInterval,
            2 => StatisticKindV3::MaximumDrawdownInterval,
            _ => return Err(Error::UnsupportedStatistic),
        };
        let coverage_policy_id = reader.u16();
        reader.reserved(2)?;
        let value = Self {
            source_plane_program_id,
            source_spec_id,
            summary_program_id,
            partition_id,
            payout_table_id,
            settlement_policy_id,
            compiler_version,
            statistic,
            coverage_policy_id,
            failure_policy_id: reader.u32(),
            repair_policy_id: reader.u32(),
            window_span_buckets: reader.u64(),
            maturity_grace_buckets: reader.u64(),
            repair_generation: reader.u64(),
            coverage_policy_parameter: reader.u64(),
        };
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}

/// Exact per-Instance account/rent and keeper allocations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkEnvelopeV3 {
    /// Exact quote/compiler version.
    pub version: u32,
    /// Account/rent principal allocated at Instance creation.
    pub creation_lamports: u64,
    /// Independently prepaid liveness allocation; never future fee revenue.
    pub liveness_lamports: u64,
}

impl WorkEnvelopeV3 {
    /// Validate positive, finite named compartments.
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 || self.creation_lamports == 0 || self.liveness_lamports == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Content identity of the exact work quote.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; WORK_ENVELOPE_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(WORK_DOMAIN, &bytes))
    }
}

impl FixedCodec for WorkEnvelopeV3 {
    const ENCODED_LEN: usize = WORK_ENVELOPE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&WORK_MAGIC);
        writer.u32(self.version);
        writer.reserved(4);
        writer.u64(self.creation_lamports);
        writer.u64(self.liveness_lamports);
        writer.finish()?;
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&WORK_MAGIC)?;
        let version = reader.u32();
        reader.reserved(4)?;
        let value = Self {
            version,
            creation_lamports: reader.u64(),
            liveness_lamports: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Compact binding to a separately owned funded liquidity schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiquidityEnvelopeV3 {
    /// Immutable liquidity-policy/schedule artifact; Market facts live there.
    pub liquidity_policy_id: ContentId,
    /// Exact quote/compiler version.
    pub version: u32,
    /// Fully prepaid collateral allocation for each instantiated market.
    pub collateral_per_instance: u64,
}

impl LiquidityEnvelopeV3 {
    /// Validate the external schedule reference and positive allocation.
    pub fn validate(&self) -> Result<()> {
        self.liquidity_policy_id.validate()?;
        if self.version != 1 || self.collateral_per_instance == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Content identity of the compact funded-liquidity binding.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; LIQUIDITY_ENVELOPE_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(LIQUIDITY_DOMAIN, &bytes))
    }
}

impl FixedCodec for LiquidityEnvelopeV3 {
    const ENCODED_LEN: usize = LIQUIDITY_ENVELOPE_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&LIQUIDITY_MAGIC);
        writer.id(self.liquidity_policy_id);
        writer.u32(self.version);
        writer.reserved(4);
        writer.u64(self.collateral_per_instance);
        writer.finish()?;
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&LIQUIDITY_MAGIC)?;
        let liquidity_policy_id = reader.id();
        let version = reader.u32();
        reader.reserved(4)?;
        let value = Self {
            liquidity_policy_id,
            version,
            collateral_per_instance: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Immutable, finite recurring instantiation schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesPlanV3 {
    /// Reusable Template identity.
    pub template_id: ContentId,
    /// Immutable collateral Realm.
    pub realm_id: ContentId,
    /// Realm Profile identity.
    pub profile_id: ContentId,
    /// Realm-namespaced price grid.
    pub price_grid_id: ContentId,
    /// Immutable market/venue fee policy.
    pub fee_policy_id: ContentId,
    /// Exact per-Instance work quote.
    pub work_envelope_id: ContentId,
    /// Compact funded-liquidity envelope.
    pub liquidity_envelope_id: ContentId,
    /// Start bucket of ordinal zero.
    pub first_start_bucket: u64,
    /// Difference between consecutive start buckets.
    pub stride_buckets: u64,
    /// Finite number of scheduled Instances.
    pub instance_count: u32,
    /// Permissionless creation lead in buckets.
    pub creation_lead_buckets: u64,
    /// Liability cap of every concrete market.
    pub market_collateral_cap: u64,
}

impl SeriesPlanV3 {
    /// Validate local schedule shape and required references.
    pub fn validate_shape(&self) -> Result<()> {
        self.template_id.validate()?;
        self.realm_id.validate()?;
        self.profile_id.validate()?;
        self.price_grid_id.validate()?;
        self.fee_policy_id.validate()?;
        self.work_envelope_id.validate()?;
        self.liquidity_envelope_id.validate()?;
        if self.stride_buckets == 0
            || self.instance_count == 0
            || self.instance_count > MAX_SERIES_INSTANCES
            || self.creation_lead_buckets == 0
            || self.first_start_bucket < self.creation_lead_buckets
            || self.market_collateral_cap == 0
        {
            return Err(Error::InvalidParameter);
        }
        self.start_bucket(self.instance_count - 1)?;
        Ok(())
    }

    /// Validate exact artifact bindings and the last scheduled maturity.
    pub fn validate_bindings(
        &self,
        template: &ProductTemplateV3,
        work: &WorkEnvelopeV3,
        liquidity: &LiquidityEnvelopeV3,
    ) -> Result<()> {
        self.validate_shape()?;
        template.validate_shape()?;
        work.validate()?;
        liquidity.validate()?;
        if self.template_id != template.id()?
            || self.work_envelope_id != work.id()?
            || self.liquidity_envelope_id != liquidity.id()?
        {
            return Err(Error::MismatchedArtifact);
        }
        if liquidity.collateral_per_instance > self.market_collateral_cap {
            return Err(Error::InvalidParameter);
        }
        let last_start = self.start_bucket(self.instance_count - 1)?;
        last_start
            .checked_add(template.window_span_buckets)
            .and_then(|end| end.checked_add(template.maturity_grace_buckets))
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }

    /// Start bucket for one exact ordinal.
    pub fn start_bucket(&self, ordinal: u32) -> Result<u64> {
        if ordinal >= self.instance_count {
            return Err(Error::InvalidParameter);
        }
        self.first_start_bucket
            .checked_add(
                self.stride_buckets
                    .checked_mul(u64::from(ordinal))
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Content identity of the finite schedule, not of any Instance.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; SERIES_PLAN_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(SERIES_DOMAIN, &bytes))
    }
}

impl FixedCodec for SeriesPlanV3 {
    const ENCODED_LEN: usize = SERIES_PLAN_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate_shape()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&SERIES_MAGIC);
        writer.id(self.template_id);
        writer.id(self.realm_id);
        writer.id(self.profile_id);
        writer.id(self.price_grid_id);
        writer.id(self.fee_policy_id);
        writer.id(self.work_envelope_id);
        writer.id(self.liquidity_envelope_id);
        writer.u64(self.first_start_bucket);
        writer.u64(self.stride_buckets);
        writer.u32(self.instance_count);
        writer.reserved(4);
        writer.u64(self.creation_lead_buckets);
        writer.u64(self.market_collateral_cap);
        writer.finish()?;
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&SERIES_MAGIC)?;
        let value = Self {
            template_id: reader.id(),
            realm_id: reader.id(),
            profile_id: reader.id(),
            price_grid_id: reader.id(),
            fee_policy_id: reader.id(),
            work_envelope_id: reader.id(),
            liquidity_envelope_id: reader.id(),
            first_start_bucket: reader.u64(),
            stride_buckets: reader.u64(),
            instance_count: reader.u32(),
            creation_lead_buckets: {
                reader.reserved(4)?;
                reader.u64()
            },
            market_collateral_cap: reader.u64(),
        };
        reader.finish()?;
        value.validate_shape()?;
        Ok(value)
    }
}

/// Canonical semantic descriptor of one absolute, window-bound product.
///
/// Series identity, ordinal, creator, and nonce are intentionally absent, so
/// independent Series that describe the same economics and window converge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstanceDescriptorV3 {
    template_id: ContentId,
    realm_id: ContentId,
    profile_id: ContentId,
    price_grid_id: ContentId,
    fee_policy_id: ContentId,
    work_envelope_id: ContentId,
    liquidity_envelope_id: ContentId,
    start_bucket: u64,
    market_collateral_cap: u64,
}

impl InstanceDescriptorV3 {
    /// Reusable Template identity.
    pub const fn template_id(self) -> ContentId {
        self.template_id
    }

    /// Collateral Realm identity.
    pub const fn realm_id(self) -> ContentId {
        self.realm_id
    }

    /// Inclusive start bucket.
    pub const fn start_bucket(self) -> u64 {
        self.start_bucket
    }

    /// Market-local liability cap.
    pub const fn market_collateral_cap(self) -> u64 {
        self.market_collateral_cap
    }

    /// Validate descriptor shape after hostile decoding.
    pub fn validate(&self) -> Result<()> {
        self.template_id.validate()?;
        self.realm_id.validate()?;
        self.profile_id.validate()?;
        self.price_grid_id.validate()?;
        self.fee_policy_id.validate()?;
        self.work_envelope_id.validate()?;
        self.liquidity_envelope_id.validate()?;
        if self.market_collateral_cap == 0 {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Full-width semantic Instance identity.
    pub fn id(&self) -> Result<ContentId> {
        let mut bytes = [0; INSTANCE_DESCRIPTOR_BYTES];
        self.encode_into(&mut bytes)?;
        Ok(content_id(INSTANCE_DOMAIN, &bytes))
    }
}

impl FixedCodec for InstanceDescriptorV3 {
    const ENCODED_LEN: usize = INSTANCE_DESCRIPTOR_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&INSTANCE_MAGIC);
        writer.id(self.template_id);
        writer.id(self.realm_id);
        writer.id(self.profile_id);
        writer.id(self.price_grid_id);
        writer.id(self.fee_policy_id);
        writer.id(self.work_envelope_id);
        writer.id(self.liquidity_envelope_id);
        writer.u64(self.start_bucket);
        writer.u64(self.market_collateral_cap);
        writer.finish()?;
        Ok(())
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&INSTANCE_MAGIC)?;
        let value = Self {
            template_id: reader.id(),
            realm_id: reader.id(),
            profile_id: reader.id(),
            price_grid_id: reader.id(),
            fee_policy_id: reader.id(),
            work_envelope_id: reader.id(),
            liquidity_envelope_id: reader.id(),
            start_bucket: reader.u64(),
            market_collateral_cap: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Deterministic lowering output carrying schedule provenance separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompiledInstanceV3 {
    series_id: ContentId,
    ordinal: u32,
    instance_id: ContentId,
    descriptor: InstanceDescriptorV3,
    window: WindowSpecV3,
    statistic_key: StatisticKeyV3,
    creation_lamports: u64,
    liveness_lamports: u64,
    liquidity_collateral: u64,
}

impl CompiledInstanceV3 {
    /// Series schedule that requested this semantic Instance.
    pub const fn series_id(self) -> ContentId {
        self.series_id
    }

    /// Ordinal inside the requesting Series.
    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }

    /// Full-width semantic identity, independent of Series and ordinal.
    pub const fn instance_id(self) -> ContentId {
        self.instance_id
    }

    /// Complete canonical descriptor. Adapter lowerings must bind all of it.
    pub const fn descriptor(self) -> InstanceDescriptorV3 {
        self.descriptor
    }

    /// Derived exact raw-window semantics. This is a checked projection, not a
    /// second persisted owner beside Template and Instance start.
    pub const fn window(self) -> WindowSpecV3 {
        self.window
    }

    /// Derived predictable statistic request.
    pub const fn statistic_key(self) -> StatisticKeyV3 {
        self.statistic_key
    }

    /// Exact creation/rent debit from the bound WorkEnvelope.
    pub const fn creation_lamports(self) -> u64 {
        self.creation_lamports
    }

    /// Exact independently prepaid liveness debit.
    pub const fn liveness_lamports(self) -> u64 {
        self.liveness_lamports
    }

    /// Exact liquidity collateral debit from the bound envelope.
    pub const fn liquidity_collateral(self) -> u64 {
        self.liquidity_collateral
    }

    /// Recompile and compare every derived field, preventing partial-ID checks.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_against(
        &self,
        source_plane: &SourcePlaneProgramV3,
        summary: &SummaryProgramV3,
        payouts: &PayoutTableV3,
        partition: &PartitionViewV3,
        template: &ProductTemplateV3,
        work: &WorkEnvelopeV3,
        liquidity: &LiquidityEnvelopeV3,
        series: &SeriesPlanV3,
    ) -> Result<()> {
        let expected = compile_instance(
            source_plane,
            summary,
            payouts,
            partition,
            template,
            work,
            liquidity,
            series,
            self.ordinal,
        )?;
        if *self != expected {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }
}

/// Deterministically lower one finite Series ordinal into an absolute Instance.
#[allow(clippy::too_many_arguments)]
pub fn compile_instance(
    source_plane: &SourcePlaneProgramV3,
    summary: &SummaryProgramV3,
    payouts: &PayoutTableV3,
    partition: &PartitionViewV3,
    template: &ProductTemplateV3,
    work: &WorkEnvelopeV3,
    liquidity: &LiquidityEnvelopeV3,
    series: &SeriesPlanV3,
    ordinal: u32,
) -> Result<CompiledInstanceV3> {
    template.validate_bindings(source_plane, summary, payouts, partition)?;
    series.validate_bindings(template, work, liquidity)?;
    let start_bucket = series.start_bucket(ordinal)?;
    let end_bucket_exclusive = start_bucket
        .checked_add(template.window_span_buckets)
        .ok_or(Error::ArithmeticOverflow)?;
    let maturity_bucket_exclusive = end_bucket_exclusive
        .checked_add(template.maturity_grace_buckets)
        .ok_or(Error::ArithmeticOverflow)?;
    let window = WindowSpecV3 {
        source_spec_id: template.source_spec_id,
        source_plane_program_id: template.source_plane_program_id,
        start_bucket,
        end_bucket_exclusive,
        maturity_bucket_exclusive,
        repair_generation: template.repair_generation,
        coverage_policy_id: template.coverage_policy_id,
        coverage_policy_parameter: template.coverage_policy_parameter,
    };
    window.validate()?;
    let statistic_key = StatisticKeyV3 {
        window_id: window.id()?,
        summary_program_id: template.summary_program_id,
        statistic: template.statistic,
    };
    let descriptor = InstanceDescriptorV3 {
        template_id: template.id()?,
        realm_id: series.realm_id,
        profile_id: series.profile_id,
        price_grid_id: series.price_grid_id,
        fee_policy_id: series.fee_policy_id,
        work_envelope_id: series.work_envelope_id,
        liquidity_envelope_id: series.liquidity_envelope_id,
        start_bucket,
        market_collateral_cap: series.market_collateral_cap,
    };
    descriptor.validate()?;
    Ok(CompiledInstanceV3 {
        series_id: series.id()?,
        ordinal,
        instance_id: descriptor.id()?,
        descriptor,
        window,
        statistic_key,
        creation_lamports: work.creation_lamports,
        liveness_lamports: work.liveness_lamports,
        liquidity_collateral: liquidity.collateral_per_instance,
    })
}

/// Mutable exact-next cursor and segregated prepaid Series compartments.
///
/// There is no future-fee field and no route from claim principal. A lapsed or
/// already-existing Instance leaves its unused allocation visible for terminal
/// refund rather than silently relabeling it as another Instance's capital.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingV3 {
    series_id: ContentId,
    next_ordinal: u32,
    creation_lamports: u64,
    liveness_lamports: u64,
    liquidity_collateral: u64,
}

impl SeriesFundingV3 {
    /// Activate only with exact funding for every finite scheduled Instance.
    pub fn activate(
        series: &SeriesPlanV3,
        template: &ProductTemplateV3,
        work: &WorkEnvelopeV3,
        liquidity: &LiquidityEnvelopeV3,
        creation_lamports: u64,
        liveness_lamports: u64,
        liquidity_collateral: u64,
    ) -> Result<Self> {
        series.validate_bindings(template, work, liquidity)?;
        let count = u64::from(series.instance_count);
        let required_creation = work
            .creation_lamports
            .checked_mul(count)
            .ok_or(Error::ArithmeticOverflow)?;
        let required_liveness = work
            .liveness_lamports
            .checked_mul(count)
            .ok_or(Error::ArithmeticOverflow)?;
        let required_liquidity = liquidity
            .collateral_per_instance
            .checked_mul(count)
            .ok_or(Error::ArithmeticOverflow)?;
        if creation_lamports != required_creation
            || liveness_lamports != required_liveness
            || liquidity_collateral != required_liquidity
        {
            return Err(Error::InsufficientPrepayment);
        }
        Ok(Self {
            series_id: series.id()?,
            next_ordinal: 0,
            creation_lamports,
            liveness_lamports,
            liquidity_collateral,
        })
    }

    /// Immutable SeriesPlan identity served by these compartments.
    pub const fn series_id(self) -> ContentId {
        self.series_id
    }

    /// First ordinal not instantiated, lapsed, or advanced over.
    pub const fn next_ordinal(self) -> u32 {
        self.next_ordinal
    }

    /// Remaining creation/rent principal.
    pub const fn creation_lamports(self) -> u64 {
        self.creation_lamports
    }

    /// Remaining independently prepaid work budget.
    pub const fn liveness_lamports(self) -> u64 {
        self.liveness_lamports
    }

    /// Remaining funded-liquidity collateral.
    pub const fn liquidity_collateral(self) -> u64 {
        self.liquidity_collateral
    }

    /// Instantiate exactly the next ordinal in `[start - lead, start)` and
    /// atomically stage all three named debits.
    #[allow(clippy::too_many_arguments)]
    pub fn instantiate_next(
        self,
        source_plane: &SourcePlaneProgramV3,
        summary: &SummaryProgramV3,
        payouts: &PayoutTableV3,
        partition: &PartitionViewV3,
        template: &ProductTemplateV3,
        work: &WorkEnvelopeV3,
        liquidity: &LiquidityEnvelopeV3,
        series: &SeriesPlanV3,
        requested_ordinal: u32,
        current_bucket: u64,
    ) -> Result<(Self, CompiledInstanceV3)> {
        self.validate_against(series, template, work, liquidity)?;
        if self.next_ordinal >= series.instance_count {
            return Err(Error::SeriesExhausted);
        }
        if requested_ordinal != self.next_ordinal {
            return Err(Error::WrongOrdinal);
        }
        validate_creation_interval(series, requested_ordinal, current_bucket)?;
        let instance = compile_instance(
            source_plane,
            summary,
            payouts,
            partition,
            template,
            work,
            liquidity,
            series,
            requested_ordinal,
        )?;
        let next = Self {
            series_id: self.series_id,
            next_ordinal: self
                .next_ordinal
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            creation_lamports: self
                .creation_lamports
                .checked_sub(work.creation_lamports)
                .ok_or(Error::InsufficientPrepayment)?,
            liveness_lamports: self
                .liveness_lamports
                .checked_sub(work.liveness_lamports)
                .ok_or(Error::InsufficientPrepayment)?,
            liquidity_collateral: self
                .liquidity_collateral
                .checked_sub(liquidity.collateral_per_instance)
                .ok_or(Error::InsufficientPrepayment)?,
        };
        Ok((next, instance))
    }

    /// Advance an expired ordinal at or after its start without spending any
    /// compartment. The unused allocation remains terminally refundable.
    pub fn lapse_next(
        self,
        series: &SeriesPlanV3,
        template: &ProductTemplateV3,
        work: &WorkEnvelopeV3,
        liquidity: &LiquidityEnvelopeV3,
        current_bucket: u64,
    ) -> Result<Self> {
        self.validate_against(series, template, work, liquidity)?;
        if self.next_ordinal >= series.instance_count {
            return Err(Error::SeriesExhausted);
        }
        if current_bucket < series.start_bucket(self.next_ordinal)? {
            return Err(Error::NotEligible);
        }
        Ok(Self {
            next_ordinal: self
                .next_ordinal
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            ..self
        })
    }

    /// Advance over the exact same semantic Instance created independently,
    /// without debiting this Series' still-refundable allocation.
    #[allow(clippy::too_many_arguments)]
    pub fn advance_existing(
        self,
        source_plane: &SourcePlaneProgramV3,
        summary: &SummaryProgramV3,
        payouts: &PayoutTableV3,
        partition: &PartitionViewV3,
        template: &ProductTemplateV3,
        work: &WorkEnvelopeV3,
        liquidity: &LiquidityEnvelopeV3,
        series: &SeriesPlanV3,
        existing: &CompiledInstanceV3,
        current_bucket: u64,
    ) -> Result<Self> {
        self.validate_against(series, template, work, liquidity)?;
        if self.next_ordinal >= series.instance_count {
            return Err(Error::SeriesExhausted);
        }
        validate_creation_interval(series, self.next_ordinal, current_bucket)?;
        let expected = compile_instance(
            source_plane,
            summary,
            payouts,
            partition,
            template,
            work,
            liquidity,
            series,
            self.next_ordinal,
        )?;
        if existing.instance_id != expected.instance_id
            || existing.descriptor != expected.descriptor
            || existing.window != expected.window
            || existing.statistic_key != expected.statistic_key
            || existing.creation_lamports != expected.creation_lamports
            || existing.liveness_lamports != expected.liveness_lamports
            || existing.liquidity_collateral != expected.liquidity_collateral
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(Self {
            next_ordinal: self
                .next_ordinal
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?,
            ..self
        })
    }

    fn validate_against(
        &self,
        series: &SeriesPlanV3,
        template: &ProductTemplateV3,
        work: &WorkEnvelopeV3,
        liquidity: &LiquidityEnvelopeV3,
    ) -> Result<()> {
        series.validate_bindings(template, work, liquidity)?;
        self.series_id.validate()?;
        if self.series_id != series.id()? || self.next_ordinal > series.instance_count {
            return Err(Error::MismatchedArtifact);
        }
        let remaining = u64::from(series.instance_count - self.next_ordinal);
        let required_creation = work
            .creation_lamports
            .checked_mul(remaining)
            .ok_or(Error::ArithmeticOverflow)?;
        let required_liveness = work
            .liveness_lamports
            .checked_mul(remaining)
            .ok_or(Error::ArithmeticOverflow)?;
        let required_liquidity = liquidity
            .collateral_per_instance
            .checked_mul(remaining)
            .ok_or(Error::ArithmeticOverflow)?;
        if self.creation_lamports < required_creation
            || self.liveness_lamports < required_liveness
            || self.liquidity_collateral < required_liquidity
        {
            return Err(Error::InsufficientPrepayment);
        }
        Ok(())
    }
}

fn validate_creation_interval(
    series: &SeriesPlanV3,
    ordinal: u32,
    current_bucket: u64,
) -> Result<()> {
    let start = series.start_bucket(ordinal)?;
    let eligible = start
        .checked_sub(series.creation_lead_buckets)
        .ok_or(Error::ArithmeticOverflow)?;
    if current_bucket < eligible || current_bucket >= start {
        return Err(Error::NotEligible);
    }
    Ok(())
}

impl FixedCodec for SeriesFundingV3 {
    const ENCODED_LEN: usize = SERIES_FUNDING_BYTES;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.series_id.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&FUNDING_MAGIC);
        writer.id(self.series_id);
        writer.u32(self.next_ordinal);
        writer.reserved(4);
        writer.u64(self.creation_lamports);
        writer.u64(self.liveness_lamports);
        writer.u64(self.liquidity_collateral);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&FUNDING_MAGIC)?;
        let series_id = reader.id();
        let next_ordinal = reader.u32();
        reader.reserved(4)?;
        let value = Self {
            series_id,
            next_ordinal,
            creation_lamports: reader.u64(),
            liveness_lamports: reader.u64(),
            liquidity_collateral: reader.u64(),
        };
        reader.finish()?;
        value.series_id.validate()?;
        Ok(value)
    }
}
