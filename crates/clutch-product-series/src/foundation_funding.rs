//! Canonical successor quote for recurring-Series admission and phased Market founding.
//!
//! This is a fresh semantic owner. It is not a decoder alias for the historical
//! five-component quote. Every created ordinal owns a separate SeriesAdmission
//! debit, while MarketCore is consumed only by the first founder and is
//! itemized into a fixed bounded account schedule.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, ComponentDebitV1, ContentId, Error, FixedCodec, MarketFoundationScheduleV1Id,
    RecoveryAttemptFundingV1, Result, SeriesAttachmentPlanId, SeriesAttachmentPlanV3Id,
    SeriesFundingQuoteV2Id, SeriesFundingQuoteV3Id, MAX_RECOVERY_ATTEMPTS,
};

const QUOTE_MAGIC_V2: [u8; 8] = *b"DCFQUOT2";
const QUOTE_SCHEMA_V2: u16 = 2;
const ATTACHMENT_MAGIC_V2: [u8; 8] = *b"DCSATTV2";
const ATTACHMENT_SCHEMA_V2: u16 = 2;
const QUOTE_MAGIC_V3: [u8; 8] = *b"DCFQUOT3";
const QUOTE_SCHEMA_V3: u16 = 3;
const ATTACHMENT_MAGIC_V3: [u8; 8] = *b"DCSATTV3";
const ATTACHMENT_SCHEMA_V3: u16 = 3;

/// Semantic identity domain for the six-compartment funding quote.
pub const SERIES_FUNDING_QUOTE_V2_DOMAIN: &[u8] = b"dragons-clutch/series-funding-quote/v2";
/// Semantic identity domain for the QuoteV2-bound attachment plan.
pub const SERIES_ATTACHMENT_PLAN_V2_DOMAIN: &[u8] = b"dragons-clutch/series-attachment-plan/v2";
/// Semantic identity domain for the current six-compartment funding quote.
pub const SERIES_FUNDING_QUOTE_V3_DOMAIN: &[u8] = b"dragons-clutch/series-funding-quote/v3";
/// Semantic identity domain for the QuoteV3-bound attachment plan.
pub const SERIES_ATTACHMENT_PLAN_V3_DOMAIN: &[u8] = b"dragons-clutch/series-attachment-plan/v3";

/// Maximum outcome count represented by the bounded foundation schedule.
pub const MARKET_FOUNDATION_MAX_OUTCOMES_V1: usize = 16;
/// Thirteen fixed core roles precede outcome mint and custody roles.
pub const MARKET_FOUNDATION_CORE_SLOT_COUNT_V1: usize = 13;
/// Thirteen core, sixteen mint, and sixteen custody slots.
pub const MARKET_FOUNDATION_SLOT_COUNT_V1: usize =
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V1 + 2 * MARKET_FOUNDATION_MAX_OUTCOMES_V1;
/// Six disjoint Series funding compartments.
pub const SERIES_FUNDING_COMPONENT_COUNT_V2: usize = 6;
/// Exact historical hostile-codec width of [`SeriesFundingQuoteV2`].
pub const SERIES_FUNDING_QUOTE_BYTES_V2: usize = 648;
/// Exact hostile-codec width of [`SeriesAttachmentPlanV2`].
pub const SERIES_ATTACHMENT_PLAN_BYTES_V2: usize = 112;
/// Exact hostile-codec width of [`SeriesFundingQuoteV3`].
pub const SERIES_FUNDING_QUOTE_BYTES_V3: usize = 584;
/// Exact hostile-codec width of [`SeriesAttachmentPlanV3`].
pub const SERIES_ATTACHMENT_PLAN_BYTES_V3: usize = 112;

/// Stable six-compartment funding order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesFundingComponentV2 {
    /// Shared foundation principal, consumed by a founder only.
    MarketCore = 0,
    /// Per-ordinal `0xad` admission-link rent, always consumed.
    SeriesAdmission = 1,
    /// Founder-only shared evidence-Recovery work and rent. Convergers prove
    /// this quote's exact market-liveness references but debit this component zero times.
    RecoveryReserve = 2,
    /// Source/archive/window/statistic work.
    SourceWork = 3,
    /// Series-scoped passive-liquidity facility.
    LiquidityFacility = 4,
    /// Series-scoped wrapper/structured set.
    WrapperSet = 5,
}

impl SeriesFundingComponentV2 {
    /// Stable array index without an unchecked cast.
    pub const fn index(self) -> usize {
        match self {
            Self::MarketCore => 0,
            Self::SeriesAdmission => 1,
            Self::RecoveryReserve => 2,
            Self::SourceWork => 3,
            Self::LiquidityFacility => 4,
            Self::WrapperSet => 5,
        }
    }
}

/// Whether an ordinal founds or converges into an existing shared Market.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesMarketDispositionV1 {
    /// The link consumes one MarketCore allocation and owns phased founding.
    Founder = 1,
    /// The link joins the exact Active Market and consumes no MarketCore.
    Converger = 2,
}

impl SeriesMarketDispositionV1 {
    pub(crate) const fn byte(self) -> u8 {
        match self {
            Self::Founder => 1,
            Self::Converger => 2,
        }
    }

    pub(crate) fn decode(byte: u8) -> Result<Self> {
        match byte {
            1 => Ok(Self::Founder),
            2 => Ok(Self::Converger),
            _ => Err(Error::InvalidParameter),
        }
    }
}

/// Exact quote-owned principal for every phased foundation account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundationScheduleV1 {
    /// Active outcome count; inactive mint/custody tails are zero.
    pub outcome_count: u8,
    /// Canonical slot principals.
    pub slot_principal_lamports: [u64; MARKET_FOUNDATION_SLOT_COUNT_V1],
    /// Finite timeout after which an inert foundation may abort.
    pub founding_timeout_buckets: u64,
}

impl MarketFoundationScheduleV1 {
    /// Validate exact active presence, zero padding, and checked total.
    pub fn validate(&self) -> Result<()> {
        let outcomes = usize::from(self.outcome_count);
        if outcomes == 0
            || outcomes > MARKET_FOUNDATION_MAX_OUTCOMES_V1
            || self.founding_timeout_buckets == 0
        {
            return Err(Error::InvalidParameter);
        }
        let mint_end = MARKET_FOUNDATION_CORE_SLOT_COUNT_V1
            .checked_add(outcomes)
            .ok_or(Error::ArithmeticOverflow)?;
        let custody_start = MARKET_FOUNDATION_CORE_SLOT_COUNT_V1
            .checked_add(MARKET_FOUNDATION_MAX_OUTCOMES_V1)
            .ok_or(Error::ArithmeticOverflow)?;
        let custody_end = custody_start
            .checked_add(outcomes)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut index = 0usize;
        while index < MARKET_FOUNDATION_SLOT_COUNT_V1 {
            let active = index < MARKET_FOUNDATION_CORE_SLOT_COUNT_V1
                || (index >= MARKET_FOUNDATION_CORE_SLOT_COUNT_V1 && index < mint_end)
                || (index >= custody_start && index < custody_end);
            if active != (self.slot_principal_lamports[index] != 0) {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        self.total_principal_lamports()?;
        Ok(())
    }

    /// Checked sum of every active principal.
    pub fn total_principal_lamports(&self) -> Result<u64> {
        let mut total = 0u64;
        for amount in self.slot_principal_lamports {
            total = total.checked_add(amount).ok_or(Error::ArithmeticOverflow)?;
        }
        if total == 0 {
            return Err(Error::InsufficientPrepayment);
        }
        Ok(total)
    }

    /// Typed identity of the exact itemization and timeout.
    pub fn id(&self) -> Result<MarketFoundationScheduleV1Id> {
        self.validate()?;
        let mut body = [0u8; 376];
        body[0] = self.outcome_count;
        body[8..16].copy_from_slice(&self.founding_timeout_buckets.to_le_bytes());
        let mut at = 16usize;
        for amount in self.slot_principal_lamports {
            body[at..at + 8].copy_from_slice(&amount.to_le_bytes());
            at += 8;
        }
        Ok(MarketFoundationScheduleV1Id::from_bytes(
            content_id(b"dragons-clutch/market-foundation-schedule/v1", &body).bytes(),
        ))
    }
}

/// Withdrawn historical six-compartment quote and founder-only itemization.
///
/// This exact 648-byte codec remains available only so already-persisted kind
/// 48 artifacts can be decoded and audited. New registration must use V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingQuoteV2 {
    /// Exact evidence-only Recovery policy.
    pub evidence_only_recovery_policy_id: ContentId,
    /// Six independently accounted per-ordinal allocations.
    pub components: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Sole decomposition of MarketCore lamports.
    pub foundation: MarketFoundationScheduleV1,
    /// Active Recovery attempt count.
    pub recovery_attempt_count: u8,
    /// Exact active attempts followed by canonical zero padding.
    pub recovery_attempt_funding: [RecoveryAttemptFundingV1; MAX_RECOVERY_ATTEMPTS],
    /// Separately named Recovery account rent principal.
    pub recovery_rent_principal_lamports: u64,
}

impl SeriesFundingQuoteV2 {
    /// Validate compartment separation and exact Recovery/Foundation sums.
    pub fn validate(&self) -> Result<()> {
        self.evidence_only_recovery_policy_id.validate()?;
        self.foundation.validate()?;
        let market_core = self.components[SeriesFundingComponentV2::MarketCore.index()];
        let admission = self.components[SeriesFundingComponentV2::SeriesAdmission.index()];
        let recovery = self.components[SeriesFundingComponentV2::RecoveryReserve.index()];
        if market_core.collateral_atoms != 0
            || market_core.lamports != self.foundation.total_principal_lamports()?
            || admission.lamports == 0
            || admission.collateral_atoms != 0
            || recovery.collateral_atoms != 0
            || self.recovery_rent_principal_lamports == 0
        {
            return Err(Error::InvalidParameter);
        }
        let count = usize::from(self.recovery_attempt_count);
        if count == 0 || count > MAX_RECOVERY_ATTEMPTS {
            return Err(Error::InvalidParameter);
        }
        let mut work = 0u64;
        let mut index = 0usize;
        while index < MAX_RECOVERY_ATTEMPTS {
            let attempt = self.recovery_attempt_funding[index];
            if index < count {
                if attempt.max_progress_units == 0 || attempt.lamports_per_progress_unit == 0 {
                    return Err(Error::InvalidParameter);
                }
                work = work
                    .checked_add(attempt.maximum_lamports()?)
                    .ok_or(Error::ArithmeticOverflow)?;
            } else if attempt != RecoveryAttemptFundingV1::ZERO {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        if work
            .checked_add(self.recovery_rent_principal_lamports)
            .ok_or(Error::ArithmeticOverflow)?
            != recovery.lamports
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Typed identity of the exact canonical body.
    pub fn id(&self) -> Result<SeriesFundingQuoteV2Id> {
        let mut body = [0u8; SERIES_FUNDING_QUOTE_BYTES_V2];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingQuoteV2Id::from_bytes(
            content_id(SERIES_FUNDING_QUOTE_V2_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesFundingQuoteV2 {
    const ENCODED_LEN: usize = SERIES_FUNDING_QUOTE_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&QUOTE_MAGIC_V2);
        writer.u16(QUOTE_SCHEMA_V2);
        writer.u8(self.foundation.outcome_count);
        writer.u8(self.recovery_attempt_count);
        writer.reserved(4);
        writer.id(self.evidence_only_recovery_policy_id);
        for component in self.components {
            writer.u64(component.lamports);
            writer.u64(component.collateral_atoms);
        }
        for principal in self.foundation.slot_principal_lamports {
            writer.u64(principal);
        }
        writer.u64(self.foundation.founding_timeout_buckets);
        writer.u64(self.recovery_rent_principal_lamports);
        for attempt in self.recovery_attempt_funding {
            writer.u64(attempt.max_progress_units);
            writer.u64(attempt.lamports_per_progress_unit);
        }
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&QUOTE_MAGIC_V2)?;
        if reader.u16() != QUOTE_SCHEMA_V2 {
            return Err(Error::BadVersion);
        }
        let outcome_count = reader.u8();
        let recovery_attempt_count = reader.u8();
        reader.reserved(4)?;
        let evidence_only_recovery_policy_id = reader.id();
        let mut components = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        for component in &mut components {
            component.lamports = reader.u64();
            component.collateral_atoms = reader.u64();
        }
        let mut slot_principal_lamports = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V1];
        for principal in &mut slot_principal_lamports {
            *principal = reader.u64();
        }
        let founding_timeout_buckets = reader.u64();
        let recovery_rent_principal_lamports = reader.u64();
        let mut recovery_attempt_funding = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        for attempt in &mut recovery_attempt_funding {
            attempt.max_progress_units = reader.u64();
            attempt.lamports_per_progress_unit = reader.u64();
        }
        reader.finish()?;
        let value = Self {
            evidence_only_recovery_policy_id,
            components,
            foundation: MarketFoundationScheduleV1 {
                outcome_count,
                slot_principal_lamports,
                founding_timeout_buckets,
            },
            recovery_attempt_count,
            recovery_attempt_funding,
            recovery_rent_principal_lamports,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Operational attachment choices bound to the successor funding quote.
///
/// The generic [`SeriesAttachmentPlanId`] is a versioned-family identity. Its
/// digest domain distinguishes this V2 body from the decode-only V1 artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesAttachmentPlanV2 {
    /// Exact six-compartment quote.
    pub funding_quote_id: SeriesFundingQuoteV2Id,
    /// Exact liquidity-facility plan.
    pub liquidity_facility_plan_id: ContentId,
    /// Exact canonical wrapper-recipe set.
    pub wrapper_recipe_set_id: ContentId,
}

impl SeriesAttachmentPlanV2 {
    /// Validate typed nonzero references and refuse role aliasing.
    pub fn validate(&self) -> Result<()> {
        self.funding_quote_id.validate()?;
        self.liquidity_facility_plan_id.validate()?;
        self.wrapper_recipe_set_id.validate()?;
        if self.funding_quote_id.content_id() == self.liquidity_facility_plan_id
            || self.funding_quote_id.content_id() == self.wrapper_recipe_set_id
            || self.liquidity_facility_plan_id == self.wrapper_recipe_set_id
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Versioned-family attachment identity.
    pub fn id(&self) -> Result<SeriesAttachmentPlanId> {
        let mut body = [0u8; SERIES_ATTACHMENT_PLAN_BYTES_V2];
        self.encode_into(&mut body)?;
        Ok(SeriesAttachmentPlanId::from_bytes(
            content_id(SERIES_ATTACHMENT_PLAN_V2_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesAttachmentPlanV2 {
    const ENCODED_LEN: usize = SERIES_ATTACHMENT_PLAN_BYTES_V2;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&ATTACHMENT_MAGIC_V2);
        writer.u16(ATTACHMENT_SCHEMA_V2);
        writer.reserved(6);
        writer.id(self.funding_quote_id.content_id());
        writer.id(self.liquidity_facility_plan_id);
        writer.id(self.wrapper_recipe_set_id);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&ATTACHMENT_MAGIC_V2)?;
        if reader.u16() != ATTACHMENT_SCHEMA_V2 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            funding_quote_id: SeriesFundingQuoteV2Id::from_bytes(reader.id().bytes()),
            liquidity_facility_plan_id: reader.id(),
            wrapper_recipe_set_id: reader.id(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Current six-compartment quote and founder-only account itemization.
///
/// V3 removes the duplicate per-attempt pricing table from withdrawn V2 and
/// instead binds the single immutable market-liveness policy and its exact
/// Recovery quote schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingQuoteV3 {
    /// Exact evidence-only Recovery policy.
    pub evidence_only_recovery_policy_id: ContentId,
    /// Existing market-scoped runtime-liveness policy semantic owner.
    pub failure_liveness_policy_id: ContentId,
    /// Exact Recovery-compartment schedule owned by that liveness policy.
    pub failure_recovery_quote_schedule_id: ContentId,
    /// Six independently accounted maxima. SeriesAdmission is per ordinal;
    /// MarketCore and RecoveryReserve are consumed only by the founder.
    pub components: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Sole decomposition of MarketCore lamports.
    pub foundation: MarketFoundationScheduleV1,
    /// Separately named Recovery account rent principal. The remainder of the
    /// RecoveryReserve allocation is present work capital and must match the
    /// authenticated runtime-liveness policy at admission.
    pub recovery_rent_principal_lamports: u64,
}

impl SeriesFundingQuoteV3 {
    /// Validate compartment separation and exact Recovery/Foundation sums.
    pub fn validate(&self) -> Result<()> {
        self.evidence_only_recovery_policy_id.validate()?;
        self.failure_liveness_policy_id.validate()?;
        self.failure_recovery_quote_schedule_id.validate()?;
        self.foundation.validate()?;
        let market_core = self.components[SeriesFundingComponentV2::MarketCore.index()];
        let admission = self.components[SeriesFundingComponentV2::SeriesAdmission.index()];
        let recovery = self.components[SeriesFundingComponentV2::RecoveryReserve.index()];
        if market_core.collateral_atoms != 0
            || market_core.lamports != self.foundation.total_principal_lamports()?
            || admission.lamports == 0
            || admission.collateral_atoms != 0
            || recovery.collateral_atoms != 0
            || self.recovery_rent_principal_lamports == 0
            || recovery.lamports <= self.recovery_rent_principal_lamports
            || self.evidence_only_recovery_policy_id == self.failure_liveness_policy_id
            || self.evidence_only_recovery_policy_id == self.failure_recovery_quote_schedule_id
            || self.failure_liveness_policy_id == self.failure_recovery_quote_schedule_id
        {
            return Err(Error::InvalidParameter);
        }
        Ok(())
    }

    /// Typed identity of the exact canonical V3 body.
    pub fn id(&self) -> Result<SeriesFundingQuoteV3Id> {
        let mut body = [0u8; SERIES_FUNDING_QUOTE_BYTES_V3];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingQuoteV3Id::from_bytes(
            content_id(SERIES_FUNDING_QUOTE_V3_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesFundingQuoteV3 {
    const ENCODED_LEN: usize = SERIES_FUNDING_QUOTE_BYTES_V3;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&QUOTE_MAGIC_V3);
        writer.u16(QUOTE_SCHEMA_V3);
        writer.u8(self.foundation.outcome_count);
        writer.reserved(5);
        writer.id(self.evidence_only_recovery_policy_id);
        writer.id(self.failure_liveness_policy_id);
        writer.id(self.failure_recovery_quote_schedule_id);
        for component in self.components {
            writer.u64(component.lamports);
            writer.u64(component.collateral_atoms);
        }
        for principal in self.foundation.slot_principal_lamports {
            writer.u64(principal);
        }
        writer.u64(self.foundation.founding_timeout_buckets);
        writer.u64(self.recovery_rent_principal_lamports);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&QUOTE_MAGIC_V3)?;
        if reader.u16() != QUOTE_SCHEMA_V3 {
            return Err(Error::BadVersion);
        }
        let outcome_count = reader.u8();
        reader.reserved(5)?;
        let evidence_only_recovery_policy_id = reader.id();
        let failure_liveness_policy_id = reader.id();
        let failure_recovery_quote_schedule_id = reader.id();
        let mut components = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        for component in &mut components {
            component.lamports = reader.u64();
            component.collateral_atoms = reader.u64();
        }
        let mut slot_principal_lamports = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V1];
        for principal in &mut slot_principal_lamports {
            *principal = reader.u64();
        }
        let founding_timeout_buckets = reader.u64();
        let recovery_rent_principal_lamports = reader.u64();
        reader.finish()?;
        let value = Self {
            evidence_only_recovery_policy_id,
            failure_liveness_policy_id,
            failure_recovery_quote_schedule_id,
            components,
            foundation: MarketFoundationScheduleV1 {
                outcome_count,
                slot_principal_lamports,
                founding_timeout_buckets,
            },
            recovery_rent_principal_lamports,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Operational attachment choices bound to one exact current QuoteV3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesAttachmentPlanV3 {
    /// Exact current six-compartment quote.
    pub funding_quote_id: SeriesFundingQuoteV3Id,
    /// Exact liquidity-facility plan.
    pub liquidity_facility_plan_id: ContentId,
    /// Exact canonical wrapper-recipe set.
    pub wrapper_recipe_set_id: ContentId,
}

impl SeriesAttachmentPlanV3 {
    /// Validate typed nonzero references and refuse role aliasing.
    pub fn validate(&self) -> Result<()> {
        self.funding_quote_id.validate()?;
        self.liquidity_facility_plan_id.validate()?;
        self.wrapper_recipe_set_id.validate()?;
        if self.funding_quote_id.content_id() == self.liquidity_facility_plan_id
            || self.funding_quote_id.content_id() == self.wrapper_recipe_set_id
            || self.liquidity_facility_plan_id == self.wrapper_recipe_set_id
        {
            return Err(Error::MismatchedArtifact);
        }
        Ok(())
    }

    /// Typed identity of this exact V3 attachment body.
    pub fn id(&self) -> Result<SeriesAttachmentPlanV3Id> {
        let mut body = [0u8; SERIES_ATTACHMENT_PLAN_BYTES_V3];
        self.encode_into(&mut body)?;
        Ok(SeriesAttachmentPlanV3Id::from_bytes(
            content_id(SERIES_ATTACHMENT_PLAN_V3_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesAttachmentPlanV3 {
    const ENCODED_LEN: usize = SERIES_ATTACHMENT_PLAN_BYTES_V3;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&ATTACHMENT_MAGIC_V3);
        writer.u16(ATTACHMENT_SCHEMA_V3);
        writer.reserved(6);
        writer.id(self.funding_quote_id.content_id());
        writer.id(self.liquidity_facility_plan_id);
        writer.id(self.wrapper_recipe_set_id);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&ATTACHMENT_MAGIC_V3)?;
        if reader.u16() != ATTACHMENT_SCHEMA_V3 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            funding_quote_id: SeriesFundingQuoteV3Id::from_bytes(reader.id().bytes()),
            liquidity_facility_plan_id: reader.id(),
            wrapper_recipe_set_id: reader.id(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ContentId {
        ContentId::from_bytes([byte; 32])
    }

    fn foundation_and_components() -> (
        MarketFoundationScheduleV1,
        [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) {
        let mut slot_principal_lamports = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V1];
        for principal in &mut slot_principal_lamports[..MARKET_FOUNDATION_CORE_SLOT_COUNT_V1 + 2] {
            *principal = 10;
        }
        let custody_start =
            MARKET_FOUNDATION_CORE_SLOT_COUNT_V1 + MARKET_FOUNDATION_MAX_OUTCOMES_V1;
        for principal in &mut slot_principal_lamports[custody_start..custody_start + 2] {
            *principal = 10;
        }
        let foundation = MarketFoundationScheduleV1 {
            outcome_count: 2,
            slot_principal_lamports,
            founding_timeout_buckets: 40,
        };
        let mut components = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        components[SeriesFundingComponentV2::MarketCore.index()].lamports =
            foundation.total_principal_lamports().unwrap();
        components[SeriesFundingComponentV2::SeriesAdmission.index()].lamports = 20;
        components[SeriesFundingComponentV2::RecoveryReserve.index()].lamports = 31;
        (foundation, components)
    }

    fn quote_v2() -> SeriesFundingQuoteV2 {
        let (foundation, components) = foundation_and_components();
        let mut recovery_attempt_funding = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
        recovery_attempt_funding[0] = RecoveryAttemptFundingV1 {
            max_progress_units: 3,
            lamports_per_progress_unit: 7,
        };
        SeriesFundingQuoteV2 {
            evidence_only_recovery_policy_id: id(1),
            components,
            foundation,
            recovery_attempt_count: 1,
            recovery_attempt_funding,
            recovery_rent_principal_lamports: 10,
        }
    }

    fn quote_v3() -> SeriesFundingQuoteV3 {
        let (foundation, components) = foundation_and_components();
        SeriesFundingQuoteV3 {
            evidence_only_recovery_policy_id: id(1),
            failure_liveness_policy_id: id(2),
            failure_recovery_quote_schedule_id: id(3),
            components,
            foundation,
            recovery_rent_principal_lamports: 10,
        }
    }

    #[test]
    fn withdrawn_v2_quote_and_attachment_remain_exactly_decodable() {
        let quote = quote_v2();
        let mut quote_bytes = [0u8; SERIES_FUNDING_QUOTE_BYTES_V2];
        quote.encode_into(&mut quote_bytes).unwrap();
        assert_eq!(SeriesFundingQuoteV2::decode(&quote_bytes), Ok(quote));

        let attachment = SeriesAttachmentPlanV2 {
            funding_quote_id: quote.id().unwrap(),
            liquidity_facility_plan_id: id(4),
            wrapper_recipe_set_id: id(5),
        };
        let mut attachment_bytes = [0u8; SERIES_ATTACHMENT_PLAN_BYTES_V2];
        attachment.encode_into(&mut attachment_bytes).unwrap();
        assert_eq!(
            SeriesAttachmentPlanV2::decode(&attachment_bytes),
            Ok(attachment)
        );
    }

    #[test]
    fn v3_quote_and_attachment_have_fresh_exact_coordinates() {
        let quote = quote_v3();
        let mut quote_bytes = [0u8; SERIES_FUNDING_QUOTE_BYTES_V3];
        quote.encode_into(&mut quote_bytes).unwrap();
        assert_eq!(SeriesFundingQuoteV3::decode(&quote_bytes), Ok(quote));
        assert_eq!(&quote_bytes[..8], b"DCFQUOT3");

        let attachment = SeriesAttachmentPlanV3 {
            funding_quote_id: quote.id().unwrap(),
            liquidity_facility_plan_id: id(4),
            wrapper_recipe_set_id: id(5),
        };
        let mut attachment_bytes = [0u8; SERIES_ATTACHMENT_PLAN_BYTES_V3];
        attachment.encode_into(&mut attachment_bytes).unwrap();
        assert_eq!(
            SeriesAttachmentPlanV3::decode(&attachment_bytes),
            Ok(attachment)
        );
        assert_eq!(&attachment_bytes[..8], b"DCSATTV3");
    }

    #[test]
    fn v2_and_v3_quote_coordinates_never_cross_decode() {
        let mut v2 = [0u8; SERIES_FUNDING_QUOTE_BYTES_V2];
        quote_v2().encode_into(&mut v2).unwrap();
        let mut v3 = [0u8; SERIES_FUNDING_QUOTE_BYTES_V3];
        quote_v3().encode_into(&mut v3).unwrap();
        assert_eq!(SeriesFundingQuoteV3::decode(&v2), Err(Error::TrailingBytes));
        assert_eq!(SeriesFundingQuoteV2::decode(&v3), Err(Error::Truncated));
    }

    #[test]
    fn quote_refuses_inactive_slot_principal_and_component_aliasing() {
        let mut noncanonical = quote_v3();
        noncanonical.foundation.slot_principal_lamports[MARKET_FOUNDATION_CORE_SLOT_COUNT_V1 + 3] =
            1;
        assert_eq!(noncanonical.validate(), Err(Error::NonCanonicalPadding));

        let quote = quote_v3();
        let quote_id = quote.id().unwrap();
        let attachment = SeriesAttachmentPlanV3 {
            funding_quote_id: quote_id,
            liquidity_facility_plan_id: quote_id.content_id(),
            wrapper_recipe_set_id: id(4),
        };
        assert_eq!(attachment.validate(), Err(Error::MismatchedArtifact));
    }

    #[test]
    fn quote_refuses_series_shaped_replacement_of_market_liveness_authority() {
        let mut aliased = quote_v3();
        aliased.failure_recovery_quote_schedule_id = aliased.failure_liveness_policy_id;
        assert_eq!(aliased.validate(), Err(Error::InvalidParameter));

        let original = quote_v3();
        let mut substituted = original;
        substituted.failure_recovery_quote_schedule_id = id(9);
        assert_ne!(substituted.id().unwrap(), original.id().unwrap());
    }
}
