//! Fresh 47-slot Market foundation funding owner.
//!
//! V2/QuoteV4 remain historical coordinates. This successor adds one exact
//! Token-2022 Hoard collateral-vault rent principal without reinterpreting a
//! Hoard state account or an outcome-custody slot.

use crate::codec::{Reader, Writer};
use crate::{
    content_id, ComponentDebitV1, ContentId, Error, FixedCodec,
    MarketFoundationScheduleV3Id, Result, SeriesAttachmentPlanV5Id,
    SeriesFundingComponentV2, SeriesFundingQuoteV5Id, SERIES_FUNDING_COMPONENT_COUNT_V2,
};

const QUOTE_MAGIC_V5: [u8; 8] = *b"DCFQUOT5";
const QUOTE_SCHEMA_V5: u16 = 5;
const ATTACHMENT_MAGIC_V5: [u8; 8] = *b"DCSATTV5";
const ATTACHMENT_SCHEMA_V5: u16 = 5;

/// Semantic identity domain for the 47-slot foundation schedule.
pub const MARKET_FOUNDATION_SCHEDULE_V3_DOMAIN: &[u8] =
    b"dragons-clutch/market-foundation-schedule/v3";
/// Semantic identity domain for the current 47-slot funding quote.
pub const SERIES_FUNDING_QUOTE_V5_DOMAIN: &[u8] =
    b"dragons-clutch/series-funding-quote/v5";
/// Semantic identity domain for the QuoteV5-bound attachment plan.
pub const SERIES_ATTACHMENT_PLAN_V5_DOMAIN: &[u8] =
    b"dragons-clutch/series-attachment-plan/v5";

/// Fifteen fixed roles, ending with the Hoard collateral vault at index 14.
pub const MARKET_FOUNDATION_CORE_SLOT_COUNT_V3: usize = 15;
/// Maximum outcome count represented by the current foundation schedule.
pub const MARKET_FOUNDATION_MAX_OUTCOMES_V3: usize = 16;
/// Fifteen core roles, sixteen mints, and sixteen outcome custody accounts.
pub const MARKET_FOUNDATION_SLOT_COUNT_V3: usize =
    MARKET_FOUNDATION_CORE_SLOT_COUNT_V3 + 2 * MARKET_FOUNDATION_MAX_OUTCOMES_V3;
/// Exact hostile-codec width of [`SeriesFundingQuoteV5`].
pub const SERIES_FUNDING_QUOTE_BYTES_V5: usize = 16
    + 3 * 32
    + SERIES_FUNDING_COMPONENT_COUNT_V2 * 16
    + MARKET_FOUNDATION_SLOT_COUNT_V3 * 8
    + 8
    + 8;
/// Exact hostile-codec width of [`SeriesAttachmentPlanV5`].
pub const SERIES_ATTACHMENT_PLAN_BYTES_V5: usize = 112;

const MARKET_FOUNDATION_SCHEDULE_V3_PREIMAGE_BYTES: usize =
    16 + MARKET_FOUNDATION_SLOT_COUNT_V3 * 8;

const _: () = {
    assert!(MARKET_FOUNDATION_SLOT_COUNT_V3 == 47);
    assert!(MARKET_FOUNDATION_SCHEDULE_V3_PREIMAGE_BYTES == 392);
    assert!(SERIES_FUNDING_QUOTE_BYTES_V5 == 600);
};

/// Current exact quote-owned principal for all 47 foundation accounts.
///
/// Fixed indices 0..13 preserve the complete V2 role order. Index 14 is the
/// distinct Hoard collateral token account. Outcome mints occupy 15..30 and
/// outcome custody accounts occupy 31..46.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketFoundationScheduleV3 {
    /// Active outcome count; inactive mint/custody tails are zero.
    pub outcome_count: u8,
    /// Canonical one-account/one-principal slot array.
    pub slot_principal_lamports: [u64; MARKET_FOUNDATION_SLOT_COUNT_V3],
    /// Finite timeout after which an inert foundation may abort.
    pub founding_timeout_buckets: u64,
}

impl MarketFoundationScheduleV3 {
    /// Validate exact core presence, active outcome presence, and zero tails.
    pub fn validate(&self) -> Result<()> {
        let outcomes = usize::from(self.outcome_count);
        if outcomes == 0
            || outcomes > MARKET_FOUNDATION_MAX_OUTCOMES_V3
            || self.founding_timeout_buckets == 0
        {
            return Err(Error::InvalidParameter);
        }
        let mint_end = MARKET_FOUNDATION_CORE_SLOT_COUNT_V3
            .checked_add(outcomes)
            .ok_or(Error::ArithmeticOverflow)?;
        let custody_start = MARKET_FOUNDATION_CORE_SLOT_COUNT_V3
            .checked_add(MARKET_FOUNDATION_MAX_OUTCOMES_V3)
            .ok_or(Error::ArithmeticOverflow)?;
        let custody_end = custody_start
            .checked_add(outcomes)
            .ok_or(Error::ArithmeticOverflow)?;
        let mut index = 0usize;
        while index < MARKET_FOUNDATION_SLOT_COUNT_V3 {
            let active = index < MARKET_FOUNDATION_CORE_SLOT_COUNT_V3
                || (index >= MARKET_FOUNDATION_CORE_SLOT_COUNT_V3 && index < mint_end)
                || (index >= custody_start && index < custody_end);
            if active != (self.slot_principal_lamports[index] != 0) {
                return Err(Error::NonCanonicalPadding);
            }
            index += 1;
        }
        self.total_principal_lamports()?;
        Ok(())
    }

    /// Checked sum of every active one-account principal.
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

    /// Typed identity of the exact 47-slot itemization and timeout.
    pub fn id(&self) -> Result<MarketFoundationScheduleV3Id> {
        self.validate()?;
        let mut body = [0u8; MARKET_FOUNDATION_SCHEDULE_V3_PREIMAGE_BYTES];
        body[0] = self.outcome_count;
        body[8..16].copy_from_slice(&self.founding_timeout_buckets.to_le_bytes());
        let mut at = 16usize;
        for amount in self.slot_principal_lamports {
            body[at..at + 8].copy_from_slice(&amount.to_le_bytes());
            at += 8;
        }
        Ok(MarketFoundationScheduleV3Id::from_bytes(
            content_id(MARKET_FOUNDATION_SCHEDULE_V3_DOMAIN, &body).bytes(),
        ))
    }
}

/// Current funding quote with an exhaustive 47-slot Market foundation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingQuoteV5 {
    /// Exact evidence-only Recovery policy.
    pub evidence_only_recovery_policy_id: ContentId,
    /// Existing market-scoped runtime-liveness policy semantic owner.
    pub failure_liveness_policy_id: ContentId,
    /// Exact Recovery-compartment schedule owned by that liveness policy.
    pub failure_recovery_quote_schedule_id: ContentId,
    /// Six independently accounted maxima.
    pub components: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    /// Sole decomposition of MarketCore into 47 one-account principals.
    pub foundation: MarketFoundationScheduleV3,
    /// Separately named Recovery account rent principal.
    pub recovery_rent_principal_lamports: u64,
}

impl SeriesFundingQuoteV5 {
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
    pub fn id(&self) -> Result<SeriesFundingQuoteV5Id> {
        let mut body = [0u8; SERIES_FUNDING_QUOTE_BYTES_V5];
        self.encode_into(&mut body)?;
        Ok(SeriesFundingQuoteV5Id::from_bytes(
            content_id(SERIES_FUNDING_QUOTE_V5_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesFundingQuoteV5 {
    const ENCODED_LEN: usize = SERIES_FUNDING_QUOTE_BYTES_V5;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&QUOTE_MAGIC_V5);
        writer.u16(QUOTE_SCHEMA_V5);
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
        reader.magic(&QUOTE_MAGIC_V5)?;
        if reader.u16() != QUOTE_SCHEMA_V5 {
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
        let mut slot_principal_lamports = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V3];
        for principal in &mut slot_principal_lamports {
            *principal = reader.u64();
        }
        let value = Self {
            evidence_only_recovery_policy_id,
            failure_liveness_policy_id,
            failure_recovery_quote_schedule_id,
            components,
            foundation: MarketFoundationScheduleV3 {
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

/// Operational attachment choices bound to one exact QuoteV5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesAttachmentPlanV5 {
    /// Exact current 47-slot funding quote.
    pub funding_quote_id: SeriesFundingQuoteV5Id,
    /// Exact liquidity-facility plan.
    pub liquidity_facility_plan_id: ContentId,
    /// Exact canonical wrapper-recipe set.
    pub wrapper_recipe_set_id: ContentId,
}

impl SeriesAttachmentPlanV5 {
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

    /// Typed identity of this exact V5 attachment body.
    pub fn id(&self) -> Result<SeriesAttachmentPlanV5Id> {
        let mut body = [0u8; SERIES_ATTACHMENT_PLAN_BYTES_V5];
        self.encode_into(&mut body)?;
        Ok(SeriesAttachmentPlanV5Id::from_bytes(
            content_id(SERIES_ATTACHMENT_PLAN_V5_DOMAIN, &body).bytes(),
        ))
    }
}

impl FixedCodec for SeriesAttachmentPlanV5 {
    const ENCODED_LEN: usize = SERIES_ATTACHMENT_PLAN_BYTES_V5;

    fn encode_into(&self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let mut writer = Writer::new(output, Self::ENCODED_LEN)?;
        writer.bytes(&ATTACHMENT_MAGIC_V5);
        writer.u16(ATTACHMENT_SCHEMA_V5);
        writer.reserved(6);
        writer.id(self.funding_quote_id.content_id());
        writer.id(self.liquidity_facility_plan_id);
        writer.id(self.wrapper_recipe_set_id);
        writer.finish()
    }

    fn decode(input: &[u8]) -> Result<Self> {
        let mut reader = Reader::new(input, Self::ENCODED_LEN)?;
        reader.magic(&ATTACHMENT_MAGIC_V5)?;
        if reader.u16() != ATTACHMENT_SCHEMA_V5 {
            return Err(Error::BadVersion);
        }
        reader.reserved(6)?;
        let value = Self {
            funding_quote_id: SeriesFundingQuoteV5Id::from_bytes(reader.id().bytes()),
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

    fn schedule(outcomes: u8) -> MarketFoundationScheduleV3 {
        let mut slots = [0u64; MARKET_FOUNDATION_SLOT_COUNT_V3];
        for principal in &mut slots[..MARKET_FOUNDATION_CORE_SLOT_COUNT_V3] {
            *principal = 1;
        }
        let count = usize::from(outcomes);
        for principal in &mut slots[MARKET_FOUNDATION_CORE_SLOT_COUNT_V3
            ..MARKET_FOUNDATION_CORE_SLOT_COUNT_V3 + count]
        {
            *principal = 1;
        }
        let custody = MARKET_FOUNDATION_CORE_SLOT_COUNT_V3
            + MARKET_FOUNDATION_MAX_OUTCOMES_V3;
        for principal in &mut slots[custody..custody + count] {
            *principal = 1;
        }
        MarketFoundationScheduleV3 {
            outcome_count: outcomes,
            slot_principal_lamports: slots,
            founding_timeout_buckets: 9,
        }
    }

    #[test]
    fn hoard_collateral_vault_is_mandatory_and_disjoint() {
        let valid = schedule(2);
        assert!(valid.validate().is_ok());
        let mut missing = valid;
        missing.slot_principal_lamports[14] = 0;
        assert_eq!(missing.validate(), Err(Error::NonCanonicalPadding));
        let mut old_mint_coordinate = valid;
        old_mint_coordinate.slot_principal_lamports[30] = 1;
        assert_eq!(old_mint_coordinate.validate(), Err(Error::NonCanonicalPadding));
    }

    #[test]
    fn inactive_outcome_tails_are_canonical_zero() {
        let mut noncanonical = schedule(1);
        noncanonical.slot_principal_lamports[16] = 1;
        assert_eq!(noncanonical.validate(), Err(Error::NonCanonicalPadding));
        let mut noncanonical_custody = schedule(1);
        noncanonical_custody.slot_principal_lamports[32] = 1;
        assert_eq!(
            noncanonical_custody.validate(),
            Err(Error::NonCanonicalPadding),
        );
    }
}
