//! Current 50-slot Market foundation funding owner.
//!
//! QuoteV5 and AttachmentV5 remain historical 47-slot coordinates. This
//! successor makes MarketCore the sole decomposition of all fifty physical
//! foundation principals, including the three General treasury accounts.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, ComponentDebitV1, ContentId, Error, FixedCodec, MarketFoundationScheduleV4,
    Result, SeriesAttachmentPlanV6Id, SeriesFundingComponentV2, SeriesFundingQuoteV6Id,
    MARKET_FOUNDATION_SLOT_COUNT_V4, SERIES_FUNDING_COMPONENT_COUNT_V2,
};

const QUOTE_MAGIC_V6: [u8; 8] = *b"DCFQUOT6";
const QUOTE_SCHEMA_V6: u16 = 6;
const ATTACHMENT_MAGIC_V6: [u8; 8] = *b"DCSATTV6";
const ATTACHMENT_SCHEMA_V6: u16 = 6;

/// Semantic identity domain for the current 50-slot funding quote.
pub const SERIES_FUNDING_QUOTE_V6_DOMAIN: &[u8] =
    b"dragons-clutch/series-funding-quote/v6";
/// Semantic identity domain for the QuoteV6-bound attachment plan.
pub const SERIES_ATTACHMENT_PLAN_V6_DOMAIN: &[u8] =
    b"dragons-clutch/series-attachment-plan/v6";
/// Exact hostile-codec width of [`SeriesFundingQuoteV6`].
pub const SERIES_FUNDING_QUOTE_BYTES_V6: usize = 16
    + 3 * 32
    + SERIES_FUNDING_COMPONENT_COUNT_V2 * 16
    + MARKET_FOUNDATION_SLOT_COUNT_V4 * 8
    + 8
    + 8;
/// Exact hostile-codec width of [`SeriesAttachmentPlanV6`].
pub const SERIES_ATTACHMENT_PLAN_BYTES_V6: usize = 112;

const _: () = {
    assert!(SERIES_FUNDING_QUOTE_BYTES_V6 == 624);
    assert!(SERIES_ATTACHMENT_PLAN_BYTES_V6 == 112);
};

/// Current funding quote with an exhaustive 50-slot Market foundation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingQuoteV6 {
    /// Exact evidence-only Recovery policy.
    pub evidence_only_recovery_policy_id: ContentId,
    /// Existing market-scoped runtime-liveness policy semantic owner.
    pub failure_liveness_policy_id: ContentId,
    /// Exact Recovery-compartment schedule owned by that liveness policy.
    pub failure_recovery_quote_schedule_id: ContentId,
    /// Six independently accounted maxima.
    pub components: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Sole decomposition of MarketCore into 50 one-account principals.
    pub foundation: MarketFoundationScheduleV4,
    /// Separately named Recovery account rent principal.
    pub recovery_rent_principal_lamports: u64,
}

impl SeriesFundingQuoteV6 {
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

    /// Typed identity of the complete canonical body.
    pub fn id(&self) -> Result<SeriesFundingQuoteV6Id> {
        let mut body = [0u8; SERIES_FUNDING_QUOTE_BYTES_V6];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingQuoteV6Id::from_bytes(
            content_id(SERIES_FUNDING_QUOTE_V6_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesFundingQuoteV6 {
    const ENCODED_LEN: usize = SERIES_FUNDING_QUOTE_BYTES_V6;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&QUOTE_MAGIC_V6);
        writer.u16(QUOTE_SCHEMA_V6);
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
        reader.magic(&QUOTE_MAGIC_V6)?;
        if reader.u16() != QUOTE_SCHEMA_V6 {
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
        let mut slot_principal_lamports = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V4];
        for principal in &mut slot_principal_lamports {
            *principal = reader.u64();
        }
        let value = Self {
            evidence_only_recovery_policy_id,
            failure_liveness_policy_id,
            failure_recovery_quote_schedule_id,
            components,
            foundation: MarketFoundationScheduleV4 {
                outcome_count,
                slot_principal_lamports,
                founding_timeout_buckets: reader.u64(),
            },
            recovery_rent_principal_lamports: reader.u64(),
        };
        reader.finish()?;
        value.validate()?;
        Ok(value)
    }
}

/// Operational attachment choices bound to one exact QuoteV6.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesAttachmentPlanV6 {
    /// Exact current 50-slot funding quote.
    pub funding_quote_id: SeriesFundingQuoteV6Id,
    /// Exact liquidity-facility plan.
    pub liquidity_facility_plan_id: ContentId,
    /// Exact canonical wrapper-recipe set.
    pub wrapper_recipe_set_id: ContentId,
}

impl SeriesAttachmentPlanV6 {
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

    /// Typed identity of this exact V6 attachment body.
    pub fn id(&self) -> Result<SeriesAttachmentPlanV6Id> {
        let mut body = [0u8; SERIES_ATTACHMENT_PLAN_BYTES_V6];
        self.encode_into(&mut body)?;
        Ok(SeriesAttachmentPlanV6Id::from_bytes(
            content_id(SERIES_ATTACHMENT_PLAN_V6_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesAttachmentPlanV6 {
    const ENCODED_LEN: usize = SERIES_ATTACHMENT_PLAN_BYTES_V6;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&ATTACHMENT_MAGIC_V6);
        writer.u16(ATTACHMENT_SCHEMA_V6);
        writer.reserved(6);
        writer.id(self.funding_quote_id.content_id());
        writer.id(self.liquidity_facility_plan_id);
        writer.id(self.wrapper_recipe_set_id);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&ATTACHMENT_MAGIC_V6)?;
        if reader.u16() != ATTACHMENT_SCHEMA_V6 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            funding_quote_id: SeriesFundingQuoteV6Id::from_bytes(reader.id().bytes()),
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
    use crate::{
        MARKET_FOUNDATION_CORE_SLOT_COUNT_V4, MARKET_FOUNDATION_MAX_OUTCOMES_V4,
    };

    fn schedule(outcomes: u8) -> MarketFoundationScheduleV4 {
        let mut slots = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V4];
        for principal in &mut slots[..MARKET_FOUNDATION_CORE_SLOT_COUNT_V4] {
            *principal = 1;
        }
        let count = usize::from(outcomes);
        for principal in &mut slots[MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
            ..MARKET_FOUNDATION_CORE_SLOT_COUNT_V4 + count]
        {
            *principal = 1;
        }
        let custody = MARKET_FOUNDATION_CORE_SLOT_COUNT_V4
            + MARKET_FOUNDATION_MAX_OUTCOMES_V4;
        for principal in &mut slots[custody..custody + count] {
            *principal = 1;
        }
        for principal in &mut slots[47..50] {
            *principal = 1;
        }
        MarketFoundationScheduleV4 {
            outcome_count: outcomes,
            slot_principal_lamports: slots,
            founding_timeout_buckets: 9,
        }
    }

    #[test]
    fn market_core_is_exactly_the_full_fifty_principals() {
        let foundation = schedule(2);
        let mut components = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
        components[SeriesFundingComponentV2::MarketCore.index()].lamports =
            foundation.total_principal_lamports().unwrap();
        components[SeriesFundingComponentV2::SeriesAdmission.index()].lamports = 1;
        components[SeriesFundingComponentV2::RecoveryReserve.index()].lamports = 2;
        let valid = SeriesFundingQuoteV6 {
            evidence_only_recovery_policy_id: ContentId::from_bytes([1; 32]),
            failure_liveness_policy_id: ContentId::from_bytes([2; 32]),
            failure_recovery_quote_schedule_id: ContentId::from_bytes([3; 32]),
            components,
            foundation,
            recovery_rent_principal_lamports: 1,
        };
        assert!(valid.validate().is_ok());
        let mut omitted_treasury = valid;
        omitted_treasury.components[SeriesFundingComponentV2::MarketCore.index()].lamports =
            omitted_treasury.components[SeriesFundingComponentV2::MarketCore.index()]
                .lamports
                .checked_sub(1)
                .unwrap();
        assert_eq!(omitted_treasury.validate(), Err(Error::InvalidParameter));
    }

    #[test]
    fn quote_v5_bytes_are_not_quote_v6() {
        let historical = [0u8; SERIES_FUNDING_QUOTE_BYTES_V6];
        assert!(SeriesFundingQuoteV6::decode(&historical).is_err());
    }
}
