// SPDX-License-Identifier: AGPL-3.0-or-later

//! Current Product- and Revenue-authorized MarketBinding successor.
//!
//! V3 remains the immutable historical BundleV5/AttachmentV4 schema. V4 starts
//! again from the complete V2 General body and binds the current Product
//! RootV2/LinkV2/GraphV3/ScheduleV3/QuoteV5/AttachmentV5/BundleV6 authority.
//! It also pins the Realm-founded RevenuePolicyV2 authority and the two
//! Market-scoped treasury accounts created by the Product-to-General founder.
//! Mutable Position and service-ledger bodies remain owned by those accounts;
//! this immutable record stores only their canonical addresses.

use crate::{
    CodecError, DeletableRentOwnerV1, Id32, MarketBindingV2,
    MARKET_BINDING_ACCOUNT_BYTES_V2, MARKET_BINDING_ACCOUNT_BYTES_V4,
    MARKET_BINDING_ACCOUNT_BYTES_V5,
    MARKET_BINDING_ACCOUNT_TAG, MARKET_BINDING_ACCOUNT_VERSION_V2,
    MARKET_BINDING_ACCOUNT_VERSION_V4, MARKET_BINDING_ACCOUNT_VERSION_V5,
};

const PRODUCT_ROOT_ACCOUNT_OFFSET: usize = MARKET_BINDING_ACCOUNT_BYTES_V2;
const PRODUCT_BINDING_ID_OFFSET: usize = PRODUCT_ROOT_ACCOUNT_OFFSET + 32;
const PRODUCT_GENERATION_OFFSET: usize = PRODUCT_BINDING_ID_OFFSET + 32;
const SERIES_LINK_ACCOUNT_OFFSET: usize = PRODUCT_GENERATION_OFFSET + 8;
const SERIES_LINK_ID_OFFSET: usize = SERIES_LINK_ACCOUNT_OFFSET + 32;
const SERIES_ORDINAL_OFFSET: usize = SERIES_LINK_ID_OFFSET + 32;
const COMPILER_BUNDLE_V6_ID_OFFSET: usize = SERIES_ORDINAL_OFFSET + 4;
const FUNDING_QUOTE_V5_ID_OFFSET: usize = COMPILER_BUNDLE_V6_ID_OFFSET + 32;
const ATTACHMENT_PLAN_V5_ID_OFFSET: usize = FUNDING_QUOTE_V5_ID_OFFSET + 32;
const FOUNDATION_SCHEDULE_V3_ID_OFFSET: usize = ATTACHMENT_PLAN_V5_ID_OFFSET + 32;
const FOUNDATION_GRAPH_V3_ID_OFFSET: usize = FOUNDATION_SCHEDULE_V3_ID_OFFSET + 32;
const MARKET_LIABILITY_FOUNDING_ID_OFFSET: usize = FOUNDATION_GRAPH_V3_ID_OFFSET + 32;
const CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET: usize = MARKET_LIABILITY_FOUNDING_ID_OFFSET + 32;
const CLAIM_ISSUANCE_BINDING_ID_OFFSET: usize = CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET + 32;
const GENERAL_FOUNDING_CAPABILITY_ID_OFFSET: usize = CLAIM_ISSUANCE_BINDING_ID_OFFSET + 32;
const PRODUCT_PREAUTHORIZATION_ID_OFFSET: usize = GENERAL_FOUNDING_CAPABILITY_ID_OFFSET + 32;
const REVENUE_POLICY_RECORD_ACCOUNT_OFFSET: usize = PRODUCT_PREAUTHORIZATION_ID_OFFSET + 32;
const REVENUE_POLICY_RECORD_V2_ID_OFFSET: usize = REVENUE_POLICY_RECORD_ACCOUNT_OFFSET + 32;
const REVENUE_POLICY_V2_DIGEST_OFFSET: usize = REVENUE_POLICY_RECORD_V2_ID_OFFSET + 32;
const TREASURY_OWNER_OFFSET: usize = REVENUE_POLICY_V2_DIGEST_OFFSET + 32;
const TREASURY_POSITION_DERIVATION_POLICY_V2_ID_OFFSET: usize = TREASURY_OWNER_OFFSET + 32;
const TREASURY_POSITION_ACCOUNT_OFFSET: usize =
    TREASURY_POSITION_DERIVATION_POLICY_V2_ID_OFFSET + 32;
const TREASURY_SERVICE_LEDGER_ACCOUNT_OFFSET: usize = TREASURY_POSITION_ACCOUNT_OFFSET + 32;
const RENT_OFFSET: usize = TREASURY_SERVICE_LEDGER_ACCOUNT_OFFSET + 32;

/// Immutable current Product and Revenue coordinates authenticated before the
/// General MarketBinding write.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentMarketAuthorityV4 {
    product_market_root_account: Id32,
    product_market_binding_id: Id32,
    product_generation: u64,
    series_market_link_account: Id32,
    series_market_link_v2_id: Id32,
    series_ordinal: u32,
    compiler_bundle_v6_id: Id32,
    funding_quote_v5_id: Id32,
    attachment_plan_v5_id: Id32,
    foundation_schedule_v3_id: Id32,
    foundation_account_graph_v3_id: Id32,
    market_liability_founding_id: Id32,
    claim_mint_founding_plan_id: Id32,
    claim_issuance_binding_id: Id32,
    general_founding_capability_id: Id32,
    product_preauthorization_id: Id32,
    revenue_policy_record_account: Id32,
    revenue_policy_record_v2_id: Id32,
    revenue_policy_v2_digest: Id32,
    treasury_owner: Id32,
    treasury_position_derivation_policy_v2_id: Id32,
    treasury_position_account: Id32,
    treasury_service_ledger_account: Id32,
}

impl CurrentMarketAuthorityV4 {
    /// Construct the complete current immutable authority.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        product_market_root_account: Id32,
        product_market_binding_id: Id32,
        product_generation: u64,
        series_market_link_account: Id32,
        series_market_link_v2_id: Id32,
        series_ordinal: u32,
        compiler_bundle_v6_id: Id32,
        funding_quote_v5_id: Id32,
        attachment_plan_v5_id: Id32,
        foundation_schedule_v3_id: Id32,
        foundation_account_graph_v3_id: Id32,
        market_liability_founding_id: Id32,
        claim_mint_founding_plan_id: Id32,
        claim_issuance_binding_id: Id32,
        general_founding_capability_id: Id32,
        product_preauthorization_id: Id32,
        revenue_policy_record_account: Id32,
        revenue_policy_record_v2_id: Id32,
        revenue_policy_v2_digest: Id32,
        treasury_owner: Id32,
        treasury_position_derivation_policy_v2_id: Id32,
        treasury_position_account: Id32,
        treasury_service_ledger_account: Id32,
    ) -> Result<Self, CodecError> {
        let value = Self {
            product_market_root_account,
            product_market_binding_id,
            product_generation,
            series_market_link_account,
            series_market_link_v2_id,
            series_ordinal,
            compiler_bundle_v6_id,
            funding_quote_v5_id,
            attachment_plan_v5_id,
            foundation_schedule_v3_id,
            foundation_account_graph_v3_id,
            market_liability_founding_id,
            claim_mint_founding_plan_id,
            claim_issuance_binding_id,
            general_founding_capability_id,
            product_preauthorization_id,
            revenue_policy_record_account,
            revenue_policy_record_v2_id,
            revenue_policy_v2_digest,
            treasury_owner,
            treasury_position_derivation_policy_v2_id,
            treasury_position_account,
            treasury_service_ledger_account,
        };
        value.validate()?;
        Ok(value)
    }

    /// Product MarketLifecycleRoot V2 account.
    pub const fn product_market_root_account(self) -> Id32 { self.product_market_root_account }
    /// Exact Product MarketLifecycleBinding V2 identity.
    pub const fn product_market_binding_id(self) -> Id32 { self.product_market_binding_id }
    /// Shared nonzero Product generation.
    pub const fn product_generation(self) -> u64 { self.product_generation }
    /// Exact current SeriesMarketLink V2 account.
    pub const fn series_market_link_account(self) -> Id32 { self.series_market_link_account }
    /// Exact current SeriesMarketLink V2 semantic identity.
    pub const fn series_market_link_v2_id(self) -> Id32 { self.series_market_link_v2_id }
    /// Zero-based Series ordinal.
    pub const fn series_ordinal(self) -> u32 { self.series_ordinal }
    /// Current Product compiler BundleV6 identity.
    pub const fn compiler_bundle_v6_id(self) -> Id32 { self.compiler_bundle_v6_id }
    /// Current Product funding QuoteV5 identity.
    pub const fn funding_quote_v5_id(self) -> Id32 { self.funding_quote_v5_id }
    /// Current Product AttachmentV5 identity.
    pub const fn attachment_plan_v5_id(self) -> Id32 { self.attachment_plan_v5_id }
    /// Exact 47-slot ScheduleV3 identity.
    pub const fn foundation_schedule_v3_id(self) -> Id32 { self.foundation_schedule_v3_id }
    /// Exact 47-slot physical GraphV3 identity.
    pub const fn foundation_account_graph_v3_id(self) -> Id32 {
        self.foundation_account_graph_v3_id
    }
    /// Collateral-owned liability-founding transcript.
    pub const fn market_liability_founding_id(self) -> Id32 { self.market_liability_founding_id }
    /// Current claim-mint founding plan.
    pub const fn claim_mint_founding_plan_id(self) -> Id32 { self.claim_mint_founding_plan_id }
    /// Profile-selected claim issuance binding.
    pub const fn claim_issuance_binding_id(self) -> Id32 { self.claim_issuance_binding_id }
    /// Product-authenticated General founding capability.
    pub const fn general_founding_capability_id(self) -> Id32 {
        self.general_founding_capability_id
    }
    /// One-way Product preauthorization identity.
    pub const fn product_preauthorization_id(self) -> Id32 { self.product_preauthorization_id }
    /// Immutable Realm-scoped RevenuePolicyRecord V2 account.
    pub const fn revenue_policy_record_account(self) -> Id32 { self.revenue_policy_record_account }
    /// Rent-independent semantic identity of the complete RecordV2 policy facts.
    pub const fn revenue_policy_record_v2_id(self) -> Id32 { self.revenue_policy_record_v2_id }
    /// Exact current RevenuePolicyV2 digest.
    pub const fn revenue_policy_v2_digest(self) -> Id32 { self.revenue_policy_v2_digest }
    /// Immutable treasury beneficiary selected by the Realm founder.
    pub const fn treasury_owner(self) -> Id32 { self.treasury_owner }
    /// Exact per-Market ordinary Position/service-ledger derivation policy.
    pub const fn treasury_position_derivation_policy_v2_id(self) -> Id32 {
        self.treasury_position_derivation_policy_v2_id
    }
    /// Derived ordinary treasury Position for this Market.
    pub const fn treasury_position_account(self) -> Id32 { self.treasury_position_account }
    /// Derived counted treasury-service ledger for this Market.
    pub const fn treasury_service_ledger_account(self) -> Id32 {
        self.treasury_service_ledger_account
    }

    fn validate(&self) -> Result<(), CodecError> {
        if self.product_generation == 0 {
            return Err(CodecError::InvalidState);
        }
        let ids = self.ids();
        if ids.iter().any(|id| id.is_zero()) {
            return Err(CodecError::MismatchedBinding);
        }
        let accounts = [
            self.product_market_root_account,
            self.series_market_link_account,
            self.revenue_policy_record_account,
            self.treasury_position_account,
            self.treasury_service_ledger_account,
        ];
        let mut left = 0usize;
        while left < accounts.len() {
            let mut right = left + 1;
            while right < accounts.len() {
                if accounts[left] == accounts[right] {
                    return Err(CodecError::MismatchedBinding);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }

    fn ids(&self) -> [Id32; 21] {
        [
            self.product_market_root_account,
            self.product_market_binding_id,
            self.series_market_link_account,
            self.series_market_link_v2_id,
            self.compiler_bundle_v6_id,
            self.funding_quote_v5_id,
            self.attachment_plan_v5_id,
            self.foundation_schedule_v3_id,
            self.foundation_account_graph_v3_id,
            self.market_liability_founding_id,
            self.claim_mint_founding_plan_id,
            self.claim_issuance_binding_id,
            self.general_founding_capability_id,
            self.product_preauthorization_id,
            self.revenue_policy_record_account,
            self.revenue_policy_record_v2_id,
            self.revenue_policy_v2_digest,
            self.treasury_owner,
            self.treasury_position_derivation_policy_v2_id,
            self.treasury_position_account,
            self.treasury_service_ledger_account,
        ]
    }
}

/// Immutable current General Market binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketBindingV4 {
    base: MarketBindingV2,
    authority: CurrentMarketAuthorityV4,
    rent: DeletableRentOwnerV1,
}

impl MarketBindingV4 {
    /// Construct the exact current binding after Product/Revenue founding.
    pub fn new(
        base: MarketBindingV2,
        authority: CurrentMarketAuthorityV4,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self, CodecError> {
        let value = Self { base, authority, rent };
        value.validate()?;
        Ok(value)
    }

    /// Complete owner-net candidate-cost General body.
    pub const fn base(&self) -> &MarketBindingV2 { &self.base }
    /// Exact current Product/Revenue authority.
    pub const fn authority(&self) -> CurrentMarketAuthorityV4 { self.authority }
    /// Sole deletable rent owner for this immutable account.
    pub const fn rent(&self) -> DeletableRentOwnerV1 { self.rent }

    /// Validate the complete V2 body, current authority, and exact rent owner.
    pub fn validate(&self) -> Result<(), CodecError> {
        self.base.validate()?;
        self.authority.validate()?;
        self.rent.validate()?;
        if self.authority.ids().iter().any(|current| {
            inherited_ids(&self.base).iter().any(|inherited| current == inherited)
        }) {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    /// Encode exact tag `0x79`, version `4`, and 1,304 canonical bytes.
    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        if output.len() != MARKET_BINDING_ACCOUNT_BYTES_V4 {
            return Err(CodecError::WrongLength);
        }
        self.base.encode(&mut output[..MARKET_BINDING_ACCOUNT_BYTES_V2])?;
        output[1] = MARKET_BINDING_ACCOUNT_VERSION_V4;
        put_id(output, PRODUCT_ROOT_ACCOUNT_OFFSET, self.authority.product_market_root_account);
        put_id(output, PRODUCT_BINDING_ID_OFFSET, self.authority.product_market_binding_id);
        output[PRODUCT_GENERATION_OFFSET..PRODUCT_GENERATION_OFFSET + 8]
            .copy_from_slice(&self.authority.product_generation.to_le_bytes());
        put_id(output, SERIES_LINK_ACCOUNT_OFFSET, self.authority.series_market_link_account);
        put_id(output, SERIES_LINK_ID_OFFSET, self.authority.series_market_link_v2_id);
        output[SERIES_ORDINAL_OFFSET..SERIES_ORDINAL_OFFSET + 4]
            .copy_from_slice(&self.authority.series_ordinal.to_le_bytes());
        for (offset, id) in [
            (COMPILER_BUNDLE_V6_ID_OFFSET, self.authority.compiler_bundle_v6_id),
            (FUNDING_QUOTE_V5_ID_OFFSET, self.authority.funding_quote_v5_id),
            (ATTACHMENT_PLAN_V5_ID_OFFSET, self.authority.attachment_plan_v5_id),
            (FOUNDATION_SCHEDULE_V3_ID_OFFSET, self.authority.foundation_schedule_v3_id),
            (FOUNDATION_GRAPH_V3_ID_OFFSET, self.authority.foundation_account_graph_v3_id),
            (MARKET_LIABILITY_FOUNDING_ID_OFFSET, self.authority.market_liability_founding_id),
            (CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET, self.authority.claim_mint_founding_plan_id),
            (CLAIM_ISSUANCE_BINDING_ID_OFFSET, self.authority.claim_issuance_binding_id),
            (GENERAL_FOUNDING_CAPABILITY_ID_OFFSET, self.authority.general_founding_capability_id),
            (PRODUCT_PREAUTHORIZATION_ID_OFFSET, self.authority.product_preauthorization_id),
            (REVENUE_POLICY_RECORD_ACCOUNT_OFFSET, self.authority.revenue_policy_record_account),
            (REVENUE_POLICY_RECORD_V2_ID_OFFSET, self.authority.revenue_policy_record_v2_id),
            (REVENUE_POLICY_V2_DIGEST_OFFSET, self.authority.revenue_policy_v2_digest),
            (TREASURY_OWNER_OFFSET, self.authority.treasury_owner),
            (
                TREASURY_POSITION_DERIVATION_POLICY_V2_ID_OFFSET,
                self.authority.treasury_position_derivation_policy_v2_id,
            ),
            (TREASURY_POSITION_ACCOUNT_OFFSET, self.authority.treasury_position_account),
            (
                TREASURY_SERVICE_LEDGER_ACCOUNT_OFFSET,
                self.authority.treasury_service_ledger_account,
            ),
        ] {
            put_id(output, offset, id);
        }
        put_id(output, RENT_OFFSET, self.rent.payer);
        output[RENT_OFFSET + 32..RENT_OFFSET + 40]
            .copy_from_slice(&self.rent.refundable_principal.to_le_bytes());
        output[RENT_OFFSET + 40..RENT_OFFSET + 48]
            .copy_from_slice(&self.rent.donation_floor.to_le_bytes());
        Ok(())
    }

    /// Decode only exact V4 bytes; V1/V2/V3 cannot alias this authority.
    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() != MARKET_BINDING_ACCOUNT_BYTES_V4 {
            return Err(CodecError::WrongLength);
        }
        if input[0] != MARKET_BINDING_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != MARKET_BINDING_ACCOUNT_VERSION_V4 {
            return Err(CodecError::WrongVersion);
        }
        let mut prefix = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V2];
        prefix.copy_from_slice(&input[..MARKET_BINDING_ACCOUNT_BYTES_V2]);
        prefix[1] = MARKET_BINDING_ACCOUNT_VERSION_V2;
        let base = MarketBindingV2::decode(&prefix)?;
        let authority = CurrentMarketAuthorityV4::new(
            read_id(input, PRODUCT_ROOT_ACCOUNT_OFFSET)?,
            read_id(input, PRODUCT_BINDING_ID_OFFSET)?,
            read_u64(input, PRODUCT_GENERATION_OFFSET)?,
            read_id(input, SERIES_LINK_ACCOUNT_OFFSET)?,
            read_id(input, SERIES_LINK_ID_OFFSET)?,
            read_u32(input, SERIES_ORDINAL_OFFSET)?,
            read_id(input, COMPILER_BUNDLE_V6_ID_OFFSET)?,
            read_id(input, FUNDING_QUOTE_V5_ID_OFFSET)?,
            read_id(input, ATTACHMENT_PLAN_V5_ID_OFFSET)?,
            read_id(input, FOUNDATION_SCHEDULE_V3_ID_OFFSET)?,
            read_id(input, FOUNDATION_GRAPH_V3_ID_OFFSET)?,
            read_id(input, MARKET_LIABILITY_FOUNDING_ID_OFFSET)?,
            read_id(input, CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET)?,
            read_id(input, CLAIM_ISSUANCE_BINDING_ID_OFFSET)?,
            read_id(input, GENERAL_FOUNDING_CAPABILITY_ID_OFFSET)?,
            read_id(input, PRODUCT_PREAUTHORIZATION_ID_OFFSET)?,
            read_id(input, REVENUE_POLICY_RECORD_ACCOUNT_OFFSET)?,
            read_id(input, REVENUE_POLICY_RECORD_V2_ID_OFFSET)?,
            read_id(input, REVENUE_POLICY_V2_DIGEST_OFFSET)?,
            read_id(input, TREASURY_OWNER_OFFSET)?,
            read_id(input, TREASURY_POSITION_DERIVATION_POLICY_V2_ID_OFFSET)?,
            read_id(input, TREASURY_POSITION_ACCOUNT_OFFSET)?,
            read_id(input, TREASURY_SERVICE_LEDGER_ACCOUNT_OFFSET)?,
        )?;
        let rent = DeletableRentOwnerV1 {
            payer: read_id(input, RENT_OFFSET)?,
            refundable_principal: read_u64(input, RENT_OFFSET + 32)?,
            donation_floor: read_u64(input, RENT_OFFSET + 40)?,
        };
        Self::new(base, authority, rent)
    }
}

const SERIES_FUNDING_V5_ACCOUNT_OFFSET: usize = RENT_OFFSET;
const SERIES_PHYSICAL_FOUNDER_V5_ID_OFFSET: usize = SERIES_FUNDING_V5_ACCOUNT_OFFSET + 32;
const RENT_V5_OFFSET: usize = SERIES_PHYSICAL_FOUNDER_V5_ID_OFFSET + 32;

/// Immutable Product V3/Funding V5 and Revenue V2 coordinates for the current
/// General binding.  V4 remains a historical RootV2/LinkV2 decoder and is
/// never projected into this authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentMarketAuthorityV5 {
    product_market_root_account: Id32,
    product_market_binding_v3_id: Id32,
    product_generation: u64,
    series_market_link_account: Id32,
    series_market_link_v3_id: Id32,
    series_ordinal: u32,
    compiler_bundle_v7_id: Id32,
    funding_quote_v6_id: Id32,
    attachment_plan_v6_id: Id32,
    foundation_schedule_v4_id: Id32,
    foundation_account_graph_v4_id: Id32,
    market_liability_founding_id: Id32,
    claim_mint_founding_plan_id: Id32,
    claim_issuance_binding_id: Id32,
    general_founding_capability_id: Id32,
    product_preauthorization_id: Id32,
    revenue_policy_record_account: Id32,
    revenue_policy_record_v2_id: Id32,
    revenue_policy_v2_digest: Id32,
    treasury_owner: Id32,
    treasury_position_derivation_policy_v2_id: Id32,
    treasury_position_account: Id32,
    treasury_service_ledger_account: Id32,
    series_funding_v5_account: Id32,
    series_physical_founder_v5_id: Id32,
}

impl CurrentMarketAuthorityV5 {
    /// Construct the complete current authority from one authenticated Product
    /// V3/Funding V5 founding receipt and one immutable Revenue V2 record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        product_market_root_account: Id32,
        product_market_binding_v3_id: Id32,
        product_generation: u64,
        series_market_link_account: Id32,
        series_market_link_v3_id: Id32,
        series_ordinal: u32,
        compiler_bundle_v7_id: Id32,
        funding_quote_v6_id: Id32,
        attachment_plan_v6_id: Id32,
        foundation_schedule_v4_id: Id32,
        foundation_account_graph_v4_id: Id32,
        market_liability_founding_id: Id32,
        claim_mint_founding_plan_id: Id32,
        claim_issuance_binding_id: Id32,
        general_founding_capability_id: Id32,
        product_preauthorization_id: Id32,
        revenue_policy_record_account: Id32,
        revenue_policy_record_v2_id: Id32,
        revenue_policy_v2_digest: Id32,
        treasury_owner: Id32,
        treasury_position_derivation_policy_v2_id: Id32,
        treasury_position_account: Id32,
        treasury_service_ledger_account: Id32,
        series_funding_v5_account: Id32,
        series_physical_founder_v5_id: Id32,
    ) -> Result<Self, CodecError> {
        let value = Self {
            product_market_root_account,
            product_market_binding_v3_id,
            product_generation,
            series_market_link_account,
            series_market_link_v3_id,
            series_ordinal,
            compiler_bundle_v7_id,
            funding_quote_v6_id,
            attachment_plan_v6_id,
            foundation_schedule_v4_id,
            foundation_account_graph_v4_id,
            market_liability_founding_id,
            claim_mint_founding_plan_id,
            claim_issuance_binding_id,
            general_founding_capability_id,
            product_preauthorization_id,
            revenue_policy_record_account,
            revenue_policy_record_v2_id,
            revenue_policy_v2_digest,
            treasury_owner,
            treasury_position_derivation_policy_v2_id,
            treasury_position_account,
            treasury_service_ledger_account,
            series_funding_v5_account,
            series_physical_founder_v5_id,
        };
        value.validate()?;
        Ok(value)
    }

    pub const fn product_market_root_account(self) -> Id32 { self.product_market_root_account }
    pub const fn product_market_binding_v3_id(self) -> Id32 { self.product_market_binding_v3_id }
    pub const fn product_generation(self) -> u64 { self.product_generation }
    pub const fn series_market_link_account(self) -> Id32 { self.series_market_link_account }
    pub const fn series_market_link_v3_id(self) -> Id32 { self.series_market_link_v3_id }
    pub const fn series_ordinal(self) -> u32 { self.series_ordinal }
    pub const fn compiler_bundle_v7_id(self) -> Id32 { self.compiler_bundle_v7_id }
    pub const fn funding_quote_v6_id(self) -> Id32 { self.funding_quote_v6_id }
    pub const fn attachment_plan_v6_id(self) -> Id32 { self.attachment_plan_v6_id }
    pub const fn foundation_schedule_v4_id(self) -> Id32 { self.foundation_schedule_v4_id }
    pub const fn foundation_account_graph_v4_id(self) -> Id32 {
        self.foundation_account_graph_v4_id
    }
    pub const fn market_liability_founding_id(self) -> Id32 { self.market_liability_founding_id }
    pub const fn claim_mint_founding_plan_id(self) -> Id32 { self.claim_mint_founding_plan_id }
    pub const fn claim_issuance_binding_id(self) -> Id32 { self.claim_issuance_binding_id }
    pub const fn general_founding_capability_id(self) -> Id32 {
        self.general_founding_capability_id
    }
    pub const fn product_preauthorization_id(self) -> Id32 { self.product_preauthorization_id }
    pub const fn revenue_policy_record_account(self) -> Id32 { self.revenue_policy_record_account }
    pub const fn revenue_policy_record_v2_id(self) -> Id32 { self.revenue_policy_record_v2_id }
    pub const fn revenue_policy_v2_digest(self) -> Id32 { self.revenue_policy_v2_digest }
    pub const fn treasury_owner(self) -> Id32 { self.treasury_owner }
    pub const fn treasury_position_derivation_policy_v2_id(self) -> Id32 {
        self.treasury_position_derivation_policy_v2_id
    }
    pub const fn treasury_position_account(self) -> Id32 { self.treasury_position_account }
    pub const fn treasury_service_ledger_account(self) -> Id32 {
        self.treasury_service_ledger_account
    }
    pub const fn series_funding_v5_account(self) -> Id32 { self.series_funding_v5_account }
    pub const fn series_physical_founder_v5_id(self) -> Id32 {
        self.series_physical_founder_v5_id
    }

    fn validate(&self) -> Result<(), CodecError> {
        if self.product_generation == 0 || self.ids().iter().any(|id| id.is_zero()) {
            return Err(CodecError::MismatchedBinding);
        }
        let accounts = [
            self.product_market_root_account,
            self.series_market_link_account,
            self.revenue_policy_record_account,
            self.treasury_position_account,
            self.treasury_service_ledger_account,
            self.series_funding_v5_account,
        ];
        let mut left = 0usize;
        while left < accounts.len() {
            let mut right = left + 1;
            while right < accounts.len() {
                if accounts[left] == accounts[right] {
                    return Err(CodecError::MismatchedBinding);
                }
                right += 1;
            }
            left += 1;
        }
        Ok(())
    }

    fn ids(&self) -> [Id32; 23] {
        [
            self.product_market_root_account,
            self.product_market_binding_v3_id,
            self.series_market_link_account,
            self.series_market_link_v3_id,
            self.compiler_bundle_v7_id,
            self.funding_quote_v6_id,
            self.attachment_plan_v6_id,
            self.foundation_schedule_v4_id,
            self.foundation_account_graph_v4_id,
            self.market_liability_founding_id,
            self.claim_mint_founding_plan_id,
            self.claim_issuance_binding_id,
            self.general_founding_capability_id,
            self.product_preauthorization_id,
            self.revenue_policy_record_account,
            self.revenue_policy_record_v2_id,
            self.revenue_policy_v2_digest,
            self.treasury_owner,
            self.treasury_position_derivation_policy_v2_id,
            self.treasury_position_account,
            self.treasury_service_ledger_account,
            self.series_funding_v5_account,
            self.series_physical_founder_v5_id,
        ]
    }
}

/// Current immutable General binding. Its version byte makes RootV3/LinkV3
/// authority unnameable through the historical V4 decoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketBindingV5 {
    base: MarketBindingV2,
    authority: CurrentMarketAuthorityV5,
    rent: DeletableRentOwnerV1,
}

impl MarketBindingV5 {
    pub fn new(
        base: MarketBindingV2,
        authority: CurrentMarketAuthorityV5,
        rent: DeletableRentOwnerV1,
    ) -> Result<Self, CodecError> {
        let value = Self { base, authority, rent };
        value.validate()?;
        Ok(value)
    }

    pub const fn base(&self) -> &MarketBindingV2 { &self.base }
    pub const fn authority(&self) -> CurrentMarketAuthorityV5 { self.authority }
    pub const fn rent(&self) -> DeletableRentOwnerV1 { self.rent }

    pub fn validate(&self) -> Result<(), CodecError> {
        self.base.validate()?;
        self.authority.validate()?;
        self.rent.validate()?;
        if self.authority.ids().iter().any(|current| {
            inherited_ids(&self.base).iter().any(|inherited| current == inherited)
        }) {
            return Err(CodecError::MismatchedBinding);
        }
        Ok(())
    }

    pub fn encode(&self, output: &mut [u8]) -> Result<(), CodecError> {
        self.validate()?;
        if output.len() != MARKET_BINDING_ACCOUNT_BYTES_V5 {
            return Err(CodecError::WrongLength);
        }
        self.base.encode(&mut output[..MARKET_BINDING_ACCOUNT_BYTES_V2])?;
        output[1] = MARKET_BINDING_ACCOUNT_VERSION_V5;
        let authority = self.authority;
        put_id(output, PRODUCT_ROOT_ACCOUNT_OFFSET, authority.product_market_root_account);
        put_id(output, PRODUCT_BINDING_ID_OFFSET, authority.product_market_binding_v3_id);
        output[PRODUCT_GENERATION_OFFSET..PRODUCT_GENERATION_OFFSET + 8]
            .copy_from_slice(&authority.product_generation.to_le_bytes());
        put_id(output, SERIES_LINK_ACCOUNT_OFFSET, authority.series_market_link_account);
        put_id(output, SERIES_LINK_ID_OFFSET, authority.series_market_link_v3_id);
        output[SERIES_ORDINAL_OFFSET..SERIES_ORDINAL_OFFSET + 4]
            .copy_from_slice(&authority.series_ordinal.to_le_bytes());
        for (offset, id) in [
            (COMPILER_BUNDLE_V6_ID_OFFSET, authority.compiler_bundle_v7_id),
            (FUNDING_QUOTE_V5_ID_OFFSET, authority.funding_quote_v6_id),
            (ATTACHMENT_PLAN_V5_ID_OFFSET, authority.attachment_plan_v6_id),
            (FOUNDATION_SCHEDULE_V3_ID_OFFSET, authority.foundation_schedule_v4_id),
            (FOUNDATION_GRAPH_V3_ID_OFFSET, authority.foundation_account_graph_v4_id),
            (MARKET_LIABILITY_FOUNDING_ID_OFFSET, authority.market_liability_founding_id),
            (CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET, authority.claim_mint_founding_plan_id),
            (CLAIM_ISSUANCE_BINDING_ID_OFFSET, authority.claim_issuance_binding_id),
            (GENERAL_FOUNDING_CAPABILITY_ID_OFFSET, authority.general_founding_capability_id),
            (PRODUCT_PREAUTHORIZATION_ID_OFFSET, authority.product_preauthorization_id),
            (REVENUE_POLICY_RECORD_ACCOUNT_OFFSET, authority.revenue_policy_record_account),
            (REVENUE_POLICY_RECORD_V2_ID_OFFSET, authority.revenue_policy_record_v2_id),
            (REVENUE_POLICY_V2_DIGEST_OFFSET, authority.revenue_policy_v2_digest),
            (TREASURY_OWNER_OFFSET, authority.treasury_owner),
            (TREASURY_POSITION_DERIVATION_POLICY_V2_ID_OFFSET,
                authority.treasury_position_derivation_policy_v2_id),
            (TREASURY_POSITION_ACCOUNT_OFFSET, authority.treasury_position_account),
            (TREASURY_SERVICE_LEDGER_ACCOUNT_OFFSET, authority.treasury_service_ledger_account),
            (SERIES_FUNDING_V5_ACCOUNT_OFFSET, authority.series_funding_v5_account),
            (SERIES_PHYSICAL_FOUNDER_V5_ID_OFFSET, authority.series_physical_founder_v5_id),
        ] {
            put_id(output, offset, id);
        }
        put_id(output, RENT_V5_OFFSET, self.rent.payer);
        output[RENT_V5_OFFSET + 32..RENT_V5_OFFSET + 40]
            .copy_from_slice(&self.rent.refundable_principal.to_le_bytes());
        output[RENT_V5_OFFSET + 40..RENT_V5_OFFSET + 48]
            .copy_from_slice(&self.rent.donation_floor.to_le_bytes());
        Ok(())
    }

    pub fn decode(input: &[u8]) -> Result<Self, CodecError> {
        if input.len() != MARKET_BINDING_ACCOUNT_BYTES_V5 {
            return Err(CodecError::WrongLength);
        }
        if input[0] != MARKET_BINDING_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != MARKET_BINDING_ACCOUNT_VERSION_V5 {
            return Err(CodecError::WrongVersion);
        }
        let mut prefix = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V2];
        prefix.copy_from_slice(&input[..MARKET_BINDING_ACCOUNT_BYTES_V2]);
        prefix[1] = MARKET_BINDING_ACCOUNT_VERSION_V2;
        let base = MarketBindingV2::decode(&prefix)?;
        let authority = CurrentMarketAuthorityV5::new(
            read_id(input, PRODUCT_ROOT_ACCOUNT_OFFSET)?,
            read_id(input, PRODUCT_BINDING_ID_OFFSET)?,
            read_u64(input, PRODUCT_GENERATION_OFFSET)?,
            read_id(input, SERIES_LINK_ACCOUNT_OFFSET)?,
            read_id(input, SERIES_LINK_ID_OFFSET)?,
            read_u32(input, SERIES_ORDINAL_OFFSET)?,
            read_id(input, COMPILER_BUNDLE_V6_ID_OFFSET)?,
            read_id(input, FUNDING_QUOTE_V5_ID_OFFSET)?,
            read_id(input, ATTACHMENT_PLAN_V5_ID_OFFSET)?,
            read_id(input, FOUNDATION_SCHEDULE_V3_ID_OFFSET)?,
            read_id(input, FOUNDATION_GRAPH_V3_ID_OFFSET)?,
            read_id(input, MARKET_LIABILITY_FOUNDING_ID_OFFSET)?,
            read_id(input, CLAIM_MINT_FOUNDING_PLAN_ID_OFFSET)?,
            read_id(input, CLAIM_ISSUANCE_BINDING_ID_OFFSET)?,
            read_id(input, GENERAL_FOUNDING_CAPABILITY_ID_OFFSET)?,
            read_id(input, PRODUCT_PREAUTHORIZATION_ID_OFFSET)?,
            read_id(input, REVENUE_POLICY_RECORD_ACCOUNT_OFFSET)?,
            read_id(input, REVENUE_POLICY_RECORD_V2_ID_OFFSET)?,
            read_id(input, REVENUE_POLICY_V2_DIGEST_OFFSET)?,
            read_id(input, TREASURY_OWNER_OFFSET)?,
            read_id(input, TREASURY_POSITION_DERIVATION_POLICY_V2_ID_OFFSET)?,
            read_id(input, TREASURY_POSITION_ACCOUNT_OFFSET)?,
            read_id(input, TREASURY_SERVICE_LEDGER_ACCOUNT_OFFSET)?,
            read_id(input, SERIES_FUNDING_V5_ACCOUNT_OFFSET)?,
            read_id(input, SERIES_PHYSICAL_FOUNDER_V5_ID_OFFSET)?,
        )?;
        let rent = DeletableRentOwnerV1 {
            payer: read_id(input, RENT_V5_OFFSET)?,
            refundable_principal: read_u64(input, RENT_V5_OFFSET + 32)?,
            donation_floor: read_u64(input, RENT_V5_OFFSET + 40)?,
        };
        Self::new(base, authority, rent)
    }
}

fn inherited_ids(base: &MarketBindingV2) -> [Id32; 13] {
    let relation = base.base();
    [
        relation.market,
        relation.market_genesis_profile_v2_id,
        relation.market_instance_v2_id,
        relation.series_plan_v5_id,
        relation.series_funding_terms_v2_id,
        relation.relation_policy_id,
        relation.price_measure_policy_v1_id,
        relation.native_claim_basis_id,
        relation.admission_policy_id,
        relation.score_policy_id,
        relation.settlement_policy_id,
        relation.neutral_sink,
        base.batch_policy_id(),
    ]
}

fn put_id(output: &mut [u8], offset: usize, id: Id32) {
    output[offset..offset + 32].copy_from_slice(&id.bytes());
}

fn read_id(input: &[u8], offset: usize) -> Result<Id32, CodecError> {
    Id32::new(
        input[offset..offset + 32]
            .try_into()
            .map_err(|_| CodecError::WrongLength)?,
    )
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, CodecError> {
    Ok(u64::from_le_bytes(
        input[offset..offset + 8]
            .try_into()
            .map_err(|_| CodecError::WrongLength)?,
    ))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, CodecError> {
    Ok(u32::from_le_bytes(
        input[offset..offset + 4]
            .try_into()
            .map_err(|_| CodecError::WrongLength)?,
    ))
}

const _: () = assert!(RENT_OFFSET + 48 == MARKET_BINDING_ACCOUNT_BYTES_V4);
const _: () = assert!(MARKET_BINDING_ACCOUNT_BYTES_V4 == 1_304);
const _: () = assert!(RENT_V5_OFFSET + 48 == MARKET_BINDING_ACCOUNT_BYTES_V5);
const _: () = assert!(MARKET_BINDING_ACCOUNT_BYTES_V5 == 1_368);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MarketBindingV1, SCORE_V2_Q_COST_ACTIVE_RANK_BYTES};

    fn id(byte: u8) -> Id32 { Id32::new([byte; 32]).unwrap() }

    fn base() -> MarketBindingV2 {
        MarketBindingV2::new(
            MarketBindingV1 {
                market: id(1), market_genesis_profile_v2_id: id(2),
                market_instance_v2_id: id(3), series_plan_v5_id: id(4),
                series_funding_terms_v2_id: id(5), relation_policy_id: id(6),
                price_measure_policy_v1_id: id(7), native_claim_basis_id: id(8),
                admission_policy_id: id(9), score_policy_id: id(10),
                settlement_policy_id: id(11), neutral_sink: id(12), price_scale: 10_000,
                commit_span_slots: 1, reveal_span_slots: 2, verification_span_slots: 3,
                bond_lamports: 100, invalidity_penalty: 10, abandonment_penalty: 11,
                node_cleanup_reward: 1, price_check_reward: 1, order_reward: 1,
                slice_reward: 1, completion_reward: 1, work_close_reward: 1,
                feed_close_reward: 1, freeze_reward: 1, finalize_reward: 1,
                solver_prize: 1, root_close_reward: 1, relation_version: 2,
                outcome_count: 3, basis_degree: 2,
                rank_key_len: SCORE_V2_Q_COST_ACTIVE_RANK_BYTES as u8,
                candidate_kind_mask: 1, stored_bump: 13, flags: 0,
            },
            id(14),
        ).unwrap()
    }

    fn authority() -> CurrentMarketAuthorityV4 {
        CurrentMarketAuthorityV4::new(
            id(15), id(16), 17, id(18), id(19), 0, id(20), id(21), id(22), id(23),
            id(24), id(25), id(26), id(27), id(28), id(29), id(30), id(31), id(32),
            id(33), id(34), id(35), id(36),
        ).unwrap()
    }

    fn authority_v5() -> CurrentMarketAuthorityV5 {
        CurrentMarketAuthorityV5::new(
            id(40), id(41), 42, id(43), id(44), 0, id(45), id(46), id(47), id(48),
            id(49), id(50), id(51), id(52), id(53), id(54), id(55), id(56), id(57),
            id(58), id(59), id(60), id(61), id(62), id(63),
        ).unwrap()
    }

    #[test]
    fn v4_round_trip_preserves_current_product_revenue_authority() {
        let value = MarketBindingV4::new(
            base(), authority(),
            DeletableRentOwnerV1 { payer: id(37), refundable_principal: 38, donation_floor: 39 },
        ).unwrap();
        let mut bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V4];
        value.encode(&mut bytes).unwrap();
        assert_eq!(bytes[0], MARKET_BINDING_ACCOUNT_TAG);
        assert_eq!(bytes[1], MARKET_BINDING_ACCOUNT_VERSION_V4);
        assert_eq!(MarketBindingV4::decode(&bytes), Ok(value));
        bytes[1] = crate::MARKET_BINDING_ACCOUNT_VERSION_V3;
        assert_eq!(MarketBindingV4::decode(&bytes), Err(CodecError::WrongVersion));
    }

    #[test]
    fn v4_refuses_zero_and_aliased_current_accounts() {
        let current = authority();
        assert_eq!(
            CurrentMarketAuthorityV4::new(
                Id32::ZERO, current.product_market_binding_id(), current.product_generation(),
                current.series_market_link_account(), current.series_market_link_v2_id(),
                current.series_ordinal(), current.compiler_bundle_v6_id(),
                current.funding_quote_v5_id(), current.attachment_plan_v5_id(),
                current.foundation_schedule_v3_id(), current.foundation_account_graph_v3_id(),
                current.market_liability_founding_id(), current.claim_mint_founding_plan_id(),
                current.claim_issuance_binding_id(), current.general_founding_capability_id(),
                current.product_preauthorization_id(), current.revenue_policy_record_account(),
                current.revenue_policy_record_v2_id(), current.revenue_policy_v2_digest(),
                current.treasury_owner(), current.treasury_position_derivation_policy_v2_id(),
                current.treasury_position_account(), current.treasury_service_ledger_account(),
            ),
            Err(CodecError::MismatchedBinding),
        );
        assert!(CurrentMarketAuthorityV4::new(
            current.product_market_root_account(), current.product_market_binding_id(),
            current.product_generation(), current.product_market_root_account(),
            current.series_market_link_v2_id(), current.series_ordinal(),
            current.compiler_bundle_v6_id(), current.funding_quote_v5_id(),
            current.attachment_plan_v5_id(), current.foundation_schedule_v3_id(),
            current.foundation_account_graph_v3_id(), current.market_liability_founding_id(),
            current.claim_mint_founding_plan_id(), current.claim_issuance_binding_id(),
            current.general_founding_capability_id(), current.product_preauthorization_id(),
            current.revenue_policy_record_account(), current.revenue_policy_record_v2_id(),
            current.revenue_policy_v2_digest(), current.treasury_owner(),
            current.treasury_position_derivation_policy_v2_id(),
            current.treasury_position_account(), current.treasury_service_ledger_account(),
        ).is_err());
    }

    #[test]
    fn v5_round_trip_is_disjoint_from_historical_v4() {
        let value = MarketBindingV5::new(
            base(), authority_v5(),
            DeletableRentOwnerV1 { payer: id(64), refundable_principal: 65, donation_floor: 66 },
        ).unwrap();
        let mut bytes = [0u8; MARKET_BINDING_ACCOUNT_BYTES_V5];
        value.encode(&mut bytes).unwrap();
        assert_eq!(bytes[0], MARKET_BINDING_ACCOUNT_TAG);
        assert_eq!(bytes[1], MARKET_BINDING_ACCOUNT_VERSION_V5);
        assert_eq!(MarketBindingV5::decode(&bytes), Ok(value));
        assert_eq!(MarketBindingV4::decode(&bytes), Err(CodecError::WrongLength));
        bytes[1] = MARKET_BINDING_ACCOUNT_VERSION_V4;
        assert_eq!(MarketBindingV5::decode(&bytes), Err(CodecError::WrongVersion));
    }

    #[test]
    fn v5_refuses_funding_account_aliases() {
        let current = authority_v5();
        assert!(CurrentMarketAuthorityV5::new(
            current.product_market_root_account(), current.product_market_binding_v3_id(),
            current.product_generation(), current.series_market_link_account(),
            current.series_market_link_v3_id(), current.series_ordinal(),
            current.compiler_bundle_v7_id(), current.funding_quote_v6_id(),
            current.attachment_plan_v6_id(), current.foundation_schedule_v4_id(),
            current.foundation_account_graph_v4_id(), current.market_liability_founding_id(),
            current.claim_mint_founding_plan_id(), current.claim_issuance_binding_id(),
            current.general_founding_capability_id(), current.product_preauthorization_id(),
            current.revenue_policy_record_account(), current.revenue_policy_record_v2_id(),
            current.revenue_policy_v2_digest(), current.treasury_owner(),
            current.treasury_position_derivation_policy_v2_id(),
            current.treasury_position_account(), current.treasury_service_ledger_account(),
            current.product_market_root_account(), current.series_physical_founder_v5_id(),
        ).is_err());
    }
}
