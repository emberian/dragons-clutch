//! Typed Product projection for one market's structured-claim family.
//!
//! Product owns the market lifecycle root and Series link. Structured claims
//! own only the claim-family counts and terminal receipt below. This module
//! joins those facts without inventing a second Product lifecycle account or
//! allowing an untyped market key to stand in for `MarketInstanceV2`.

use clutch_product_series::{
    ContentId, MarketGenesisProfileV2, MarketInstancePreimageV2, MarketInstanceV2Id,
    ProductTemplateV4, SeriesAttachmentPlanId, SeriesPlanV5, SeriesPlanV5Id,
};

use crate::{Error, Result};

/// Domain for `SHA256(domain || exact projection preimage)`.
pub const STRUCTURED_MARKET_PROJECTION_V1_DOMAIN: &[u8] =
    b"dragons-clutch/structured-claim/market-projection/v1\0";
/// Exact projection preimage width, excluding the hash domain.
pub const STRUCTURED_MARKET_PROJECTION_PREIMAGE_BYTES_V1: usize = 276;

/// Exhaustive Structured state projected into the Product market root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StructuredMarketProjectionStateV1 {
    /// No descriptor has ever been admitted for this market.
    Absent = 0,
    /// At least one admitted descriptor remains live.
    Live = 1,
    /// Every admitted descriptor is terminal and one aggregate receipt seals it.
    Terminal = 2,
}

impl StructuredMarketProjectionStateV1 {
    /// Stable canonical byte without relying on an unchecked representation cast.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Absent => 0,
            Self::Live => 1,
            Self::Terminal => 2,
        }
    }
}

/// Exact structured-claim projection consumed by Product's private aggregator.
///
/// The struct is a forgeable pure value. A live adapter must authenticate the
/// named artifacts, owner release, root, counts, and terminal receipt before
/// Product promotes it. [`project_structured_market_v1`] proves the immutable
/// Product/Series join and the exhaustive count partition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredMarketProjectionV1 {
    /// Exact successor economic market.
    pub market_instance_id: MarketInstanceV2Id,
    /// Exact recurring Series plan.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact finite Series ordinal.
    pub ordinal: u32,
    /// Operational attachment fixed by the Series.
    pub attachment_plan_id: SeriesAttachmentPlanId,
    /// Product compiler release fixed by the Product template.
    pub compiler_release_id: ContentId,
    /// Capability profile fixed by the Market genesis profile.
    pub capability_profile_id: ContentId,
    /// Exact Structured semantic-owner release authenticated by the adapter.
    pub owner_release_id: ContentId,
    /// Exact market-scoped Structured root authenticated by the adapter.
    pub structured_root_id: ContentId,
    /// Exhaustive family state.
    pub state: StructuredMarketProjectionStateV1,
    /// Monotone number of admitted descriptor identities.
    pub admitted_descriptor_count: u32,
    /// Admitted descriptors that can still carry supply or backing.
    pub live_descriptor_count: u32,
    /// Admitted descriptors sealed terminal by their owning runtime.
    pub terminal_descriptor_count: u32,
    /// Aggregate terminal receipt, or all-zero exactly when not terminal.
    pub terminal_receipt_id: [u8; 32],
}

impl StructuredMarketProjectionV1 {
    /// Encode the sole canonical hash preimage for Product aggregation.
    pub fn encode_preimage(
        self,
    ) -> Result<[u8; STRUCTURED_MARKET_PROJECTION_PREIMAGE_BYTES_V1]> {
        self.validate_counts()?;
        let mut output = [0_u8; STRUCTURED_MARKET_PROJECTION_PREIMAGE_BYTES_V1];
        let mut cursor = 0_usize;
        for identity in [
            self.market_instance_id.bytes(),
            self.series_plan_id.bytes(),
            self.attachment_plan_id.bytes(),
            self.compiler_release_id.bytes(),
            self.capability_profile_id.bytes(),
            self.owner_release_id.bytes(),
            self.structured_root_id.bytes(),
        ] {
            put(&mut output, &mut cursor, &identity)?;
        }
        put(&mut output, &mut cursor, &self.ordinal.to_le_bytes())?;
        put(&mut output, &mut cursor, &[self.state.byte(), 0, 0, 0])?;
        for count in [
            self.admitted_descriptor_count,
            self.live_descriptor_count,
            self.terminal_descriptor_count,
        ] {
            put(&mut output, &mut cursor, &count.to_le_bytes())?;
        }
        put(&mut output, &mut cursor, &self.terminal_receipt_id)?;
        if cursor != output.len() {
            return Err(Error::InvalidLength);
        }
        Ok(output)
    }

    fn validate_counts(self) -> Result<()> {
        self.market_instance_id
            .validate()
            .map_err(|_| Error::InvalidIdentity)?;
        self.series_plan_id
            .validate()
            .map_err(|_| Error::InvalidIdentity)?;
        self.attachment_plan_id
            .validate()
            .map_err(|_| Error::InvalidIdentity)?;
        for identity in [
            self.compiler_release_id,
            self.capability_profile_id,
            self.owner_release_id,
            self.structured_root_id,
        ] {
            if identity.is_zero() {
                return Err(Error::InvalidIdentity);
            }
        }
        if self
            .live_descriptor_count
            .checked_add(self.terminal_descriptor_count)
            != Some(self.admitted_descriptor_count)
        {
            return Err(Error::InvariantViolation);
        }
        let has_receipt = self.terminal_receipt_id != [0; 32];
        match self.state {
            StructuredMarketProjectionStateV1::Absent => {
                if self.admitted_descriptor_count != 0
                    || self.live_descriptor_count != 0
                    || self.terminal_descriptor_count != 0
                    || has_receipt
                {
                    return Err(Error::InvariantViolation);
                }
            }
            StructuredMarketProjectionStateV1::Live => {
                if self.admitted_descriptor_count == 0
                    || self.live_descriptor_count == 0
                    || has_receipt
                {
                    return Err(Error::InvariantViolation);
                }
            }
            StructuredMarketProjectionStateV1::Terminal => {
                if self.admitted_descriptor_count == 0
                    || self.live_descriptor_count != 0
                    || self.terminal_descriptor_count != self.admitted_descriptor_count
                    || !has_receipt
                {
                    return Err(Error::InvariantViolation);
                }
            }
        }
        Ok(())
    }
}

/// Join exact Product artifacts and one exhaustive Structured family state.
#[allow(clippy::too_many_arguments)]
pub fn project_structured_market_v1(
    series: &SeriesPlanV5,
    ordinal: u32,
    market: &MarketInstancePreimageV2,
    template: &ProductTemplateV4,
    genesis: &MarketGenesisProfileV2,
    owner_release_id: ContentId,
    structured_root_id: ContentId,
    state: StructuredMarketProjectionStateV1,
    admitted_descriptor_count: u32,
    live_descriptor_count: u32,
    terminal_descriptor_count: u32,
    terminal_receipt_id: [u8; 32],
) -> Result<StructuredMarketProjectionV1> {
    series
        .validate_shape()
        .map_err(|_| Error::InvalidIdentity)?;
    market.validate().map_err(|_| Error::InvalidIdentity)?;
    template
        .validate_shape()
        .map_err(|_| Error::InvalidIdentity)?;
    genesis
        .validate_shape()
        .map_err(|_| Error::InvalidIdentity)?;
    let series_plan_id = series.id().map_err(|_| Error::InvalidIdentity)?;
    let market_instance_id = market.id().map_err(|_| Error::InvalidIdentity)?;
    let start_bucket = series
        .start_bucket(ordinal)
        .map_err(|_| Error::InvalidIdentity)?;
    if series.product_template_id != template.id().map_err(|_| Error::InvalidIdentity)?
        || series.market_genesis_profile_id != genesis.id().map_err(|_| Error::InvalidIdentity)?
        || market.product_template_id != series.product_template_id
        || market.market_genesis_profile_id != series.market_genesis_profile_id
        || market.start_bucket != start_bucket
        || market.collateral_cap != series.market_collateral_cap
    {
        return Err(Error::InvalidIdentity);
    }
    let projection = StructuredMarketProjectionV1 {
        market_instance_id,
        series_plan_id,
        ordinal,
        attachment_plan_id: series.attachment_plan_id,
        compiler_release_id: template.compiler_release_id,
        capability_profile_id: genesis.capability_profile_id,
        owner_release_id,
        structured_root_id,
        state,
        admitted_descriptor_count,
        live_descriptor_count,
        terminal_descriptor_count,
        terminal_receipt_id,
    };
    projection.validate_counts()?;
    Ok(projection)
}

fn put<const N: usize>(output: &mut [u8; N], cursor: &mut usize, bytes: &[u8]) -> Result<()> {
    let end = cursor
        .checked_add(bytes.len())
        .ok_or(Error::ArithmeticOverflow)?;
    let target = output.get_mut(*cursor..end).ok_or(Error::InvalidLength)?;
    target.copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustive_states_refuse_mixed_or_unreceipted_terminal_counts() {
        let base = StructuredMarketProjectionV1 {
            market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
            series_plan_id: SeriesPlanV5Id::from_bytes([2; 32]),
            ordinal: 0,
            attachment_plan_id: SeriesAttachmentPlanId::from_bytes([3; 32]),
            compiler_release_id: ContentId::from_bytes([4; 32]),
            capability_profile_id: ContentId::from_bytes([5; 32]),
            owner_release_id: ContentId::from_bytes([6; 32]),
            structured_root_id: ContentId::from_bytes([7; 32]),
            state: StructuredMarketProjectionStateV1::Absent,
            admitted_descriptor_count: 0,
            live_descriptor_count: 0,
            terminal_descriptor_count: 0,
            terminal_receipt_id: [0; 32],
        };
        assert!(base.encode_preimage().is_ok());

        let mut live = base;
        live.state = StructuredMarketProjectionStateV1::Live;
        live.admitted_descriptor_count = 2;
        live.live_descriptor_count = 1;
        live.terminal_descriptor_count = 1;
        assert!(live.encode_preimage().is_ok());
        live.terminal_receipt_id = [8; 32];
        assert_eq!(live.encode_preimage(), Err(Error::InvariantViolation));

        let mut terminal = base;
        terminal.state = StructuredMarketProjectionStateV1::Terminal;
        terminal.admitted_descriptor_count = 2;
        terminal.terminal_descriptor_count = 2;
        assert_eq!(terminal.encode_preimage(), Err(Error::InvariantViolation));
        terminal.terminal_receipt_id = [9; 32];
        assert!(terminal.encode_preimage().is_ok());
        terminal.live_descriptor_count = 1;
        assert_eq!(terminal.encode_preimage(), Err(Error::InvariantViolation));
    }

    #[test]
    fn projection_preimage_commits_every_exact_identity_and_count() {
        let projection = StructuredMarketProjectionV1 {
            market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
            series_plan_id: SeriesPlanV5Id::from_bytes([2; 32]),
            ordinal: 3,
            attachment_plan_id: SeriesAttachmentPlanId::from_bytes([4; 32]),
            compiler_release_id: ContentId::from_bytes([5; 32]),
            capability_profile_id: ContentId::from_bytes([6; 32]),
            owner_release_id: ContentId::from_bytes([7; 32]),
            structured_root_id: ContentId::from_bytes([8; 32]),
            state: StructuredMarketProjectionStateV1::Live,
            admitted_descriptor_count: 2,
            live_descriptor_count: 1,
            terminal_descriptor_count: 1,
            terminal_receipt_id: [0; 32],
        };
        let expected = projection.encode_preimage().unwrap();
        let mut changed = projection;
        changed.owner_release_id = ContentId::from_bytes([9; 32]);
        assert_ne!(changed.encode_preimage().unwrap(), expected);
        changed = projection;
        changed.terminal_descriptor_count = 0;
        changed.admitted_descriptor_count = 1;
        assert_ne!(changed.encode_preimage().unwrap(), expected);
    }
}
