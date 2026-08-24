// SPDX-License-Identifier: AGPL-3.0-or-later

//! Current Product-to-General founding authority.
//!
//! This module owns the narrow join between one Product-current pre-root
//! authorization, the exact General MarketBinding V2 body it authenticated,
//! and one immutable Realm RevenuePolicyRecord V2.  It derives every General
//! and treasury PDA before any account is created.  The eventual atomic
//! founder consumes the private plan and returns an authenticated postwrite to
//! Product; no public field tuple can stand in for that authority.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, require_system_program, RentParameters, SYSTEM_PROGRAM_ID,
};
use crate::seeds;
use clutch_general_v2_contract::{
    CurrentMarketAuthorityV4, DeletableRentOwnerV1, GeneralFoundingPolicyV1, Id32,
    MarketBindingV2, MarketBindingV4, MarketRuntimeV3AccountV1, Sha256BackendV1,
    GENERAL_REPLAY_ACCOUNT_V1_BYTES, MARKET_BINDING_ACCOUNT_BYTES_V4,
    MARKET_RUNTIME_ACCOUNT_BYTES,
};
use clutch_product_series::{ContentId, MarketFoundationSlotV4};
use clutch_retirement::POSITION_V3_BYTES;
use clutch_solana_layout::revenue::TREASURY_SERVICE_LEDGER_V1_BYTES;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::revenue_policy_v2::{
    derive_revenue_market_treasury_v1, write_product_funded_treasury_position_v1,
    write_product_funded_treasury_replay_v1,
    write_product_funded_treasury_service_ledger_v1,
    AuthenticatedRevenuePolicyRecordV2, AuthenticatedTreasuryMarketFactsV1,
    RevenueMarketTreasuryDerivationV1,
};
use super::product_market_foundation_current::{
    AuthenticatedProductMarketFoundationDebitV4,
    AuthenticatedProductMarketFoundationStepPostwriteV4,
};

const GENERAL_CURRENT_FOUNDING_JOIN_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/general/current-founding-join/v4\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RuntimeSha256;

impl Sha256BackendV1 for RuntimeSha256 {
    fn sha256(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Product-owned current founding authority consumed by the General join.
///
/// A concrete implementation must retain the exact authenticated current
/// Product root/link, BundleV7, QuoteV6, AttachmentV6, ScheduleV4, GraphV4, collateral
/// founding, and General-policy evidence.  The default authentication method
/// refuses, so getters alone never confer authority.
pub(crate) trait AuthenticatedCurrentProductGeneralFoundingV4:
    AuthenticatedTreasuryMarketFactsV1
{
    fn authenticate_current_product_general_founding_v4(
        &self,
        _program_id: &Pubkey,
        _market_binding_account: Pubkey,
        _market_runtime_account: Pubkey,
        _policy: GeneralFoundingPolicyV1,
        _base: &MarketBindingV2,
        _revenue: &AuthenticatedRevenuePolicyRecordV2,
        _treasury: &RevenueMarketTreasuryDerivationV1,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }

    fn product_market_root_account(&self) -> Id32;
    fn product_market_binding_id(&self) -> Id32;
    fn product_generation(&self) -> u64;
    fn series_market_link_account(&self) -> Id32;
    fn series_market_link_binding_v2_id(&self) -> Id32;
    fn series_ordinal(&self) -> u32;
    fn compiler_bundle_v7_id(&self) -> Id32;
    fn funding_quote_v6_id(&self) -> Id32;
    fn attachment_plan_v6_id(&self) -> Id32;
    fn foundation_schedule_v4_id(&self) -> Id32;
    fn foundation_account_graph_v4_id(&self) -> Id32;
    fn market_liability_founding_id(&self) -> Id32;
    fn claim_mint_founding_plan_id(&self) -> Id32;
    fn claim_issuance_binding_id(&self) -> Id32;
    fn general_founding_capability_id(&self) -> Id32;
    fn product_preauthorization_id(&self) -> Id32;
    fn realm_id(&self) -> Id32;
    fn collateral_policy_id(&self) -> Id32;
    fn collateral_release_id(&self) -> Id32;
}

/// Private exact plan retained across General's atomic account creation.
#[derive(Debug)]
pub(crate) struct AuthenticatedGeneralMarketFoundingPlanV4 {
    founding_policy: GeneralFoundingPolicyV1,
    base: MarketBindingV2,
    authority: CurrentMarketAuthorityV4,
    revenue: AuthenticatedRevenuePolicyRecordV2,
    treasury: RevenueMarketTreasuryDerivationV1,
    market_binding_account: Pubkey,
    market_binding_bump: u8,
    market_runtime_account: Pubkey,
    market_runtime_bump: u8,
    realm_id: Id32,
    collateral_policy_id: Id32,
    collateral_release_id: Id32,
    join_id: ContentId,
}

impl AuthenticatedGeneralMarketFoundingPlanV4 {
    pub(crate) const fn founding_policy(&self) -> GeneralFoundingPolicyV1 {
        self.founding_policy
    }
    pub(crate) const fn base(&self) -> &MarketBindingV2 { &self.base }
    pub(crate) const fn authority(&self) -> CurrentMarketAuthorityV4 { self.authority }
    pub(crate) const fn revenue(&self) -> AuthenticatedRevenuePolicyRecordV2 { self.revenue }
    pub(crate) const fn treasury(&self) -> RevenueMarketTreasuryDerivationV1 { self.treasury }
    pub(crate) const fn market_binding_account(&self) -> Pubkey { self.market_binding_account }
    pub(crate) const fn market_binding_bump(&self) -> u8 { self.market_binding_bump }
    pub(crate) const fn market_runtime_account(&self) -> Pubkey { self.market_runtime_account }
    pub(crate) const fn market_runtime_bump(&self) -> u8 { self.market_runtime_bump }
    pub(crate) const fn realm_id(&self) -> Id32 { self.realm_id }
    pub(crate) const fn collateral_policy_id(&self) -> Id32 { self.collateral_policy_id }
    pub(crate) const fn collateral_release_id(&self) -> Id32 { self.collateral_release_id }
    pub(crate) const fn id(&self) -> ContentId { self.join_id }
}

/// Private move-only postwrite for one Product-funded General core slot.
/// Product consumes it by value before advancing the V4 foundation cursor.
#[derive(Debug)]
pub(crate) struct AuthenticatedGeneralCoreFoundationPostwriteV4 {
    debit_id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_steps_id: ContentId,
    market_binding_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    market_instance_id: clutch_product_series::MarketInstanceV2Id,
    generation: u64,
    slot: MarketFoundationSlotV4,
    account: Pubkey,
    principal_lamports: u64,
    principal_before_lamports: u64,
    principal_after_lamports: u64,
    destination_donation_floor_lamports: u64,
    destination_balance_after_lamports: u64,
    vault_donation_before_lamports: u64,
    vault_donation_after_lamports: u64,
    foundation_vault_account: Pubkey,
    rent_refund_owner: Pubkey,
    neutral_lamport_sink: Pubkey,
    account_data_id: ContentId,
    accepted_poststate_receipt_id: ContentId,
}

/// Derive and authenticate the complete current General founding graph.
pub(crate) fn prepare_general_market_founding_v4<P>(
    program_id: &Pubkey,
    product: &P,
    founding_policy_bytes: &[u8],
    base: MarketBindingV2,
    revenue: AuthenticatedRevenuePolicyRecordV2,
) -> Outcome<AuthenticatedGeneralMarketFoundingPlanV4>
where
    P: AuthenticatedCurrentProductGeneralFoundingV4,
{
    let founding_policy = GeneralFoundingPolicyV1::decode(founding_policy_bytes)?;
    let founding_policy_id = founding_policy.semantic_id(&RuntimeSha256)?;
    base.validate()?;
    founding_policy.binds_market(base.base())?;
    let relation = base.base();
    let market_instance = relation.market_instance_v2_id.bytes();
    let (market_binding_account, market_binding_bump) =
        seeds::general_v2_market_binding_pda(program_id, &market_instance);
    let (market_runtime_account, market_runtime_bump) =
        seeds::general_v2_market_runtime_pda(program_id, &market_binding_account.to_bytes());
    require(
        relation.market == Id32::from_bytes(market_runtime_account.to_bytes())
            && relation.stored_bump == market_binding_bump
            && product.product_generation() != 0
            && product.general_founding_capability_id() == founding_policy_id
            && product.realm_id().bytes() == revenue.realm().bytes(),
        ClutchError::MismatchedState,
    )?;
    let treasury = derive_revenue_market_treasury_v1(
        program_id,
        revenue,
        clutch_solana_layout::Hash32::from_bytes(market_instance),
        market_runtime_account,
    )?;
    product.authenticate_current_product_general_founding_v4(
        program_id,
        market_binding_account,
        market_runtime_account,
        founding_policy,
        &base,
        &revenue,
        &treasury,
    )?;
    let revenue_policy = revenue.policy();
    require(
        revenue_policy.is_successor_development_profile()
            && relation.neutral_sink.bytes() != revenue.treasury_owner().bytes(),
        ClutchError::MismatchedState,
    )?;
    let authority = CurrentMarketAuthorityV4::new(
        product.product_market_root_account(),
        product.product_market_binding_id(),
        product.product_generation(),
        product.series_market_link_account(),
        product.series_market_link_binding_v2_id(),
        product.series_ordinal(),
        product.compiler_bundle_v7_id(),
        product.funding_quote_v6_id(),
        product.attachment_plan_v6_id(),
        product.foundation_schedule_v4_id(),
        product.foundation_account_graph_v4_id(),
        product.market_liability_founding_id(),
        product.claim_mint_founding_plan_id(),
        product.claim_issuance_binding_id(),
        product.general_founding_capability_id(),
        product.product_preauthorization_id(),
        Id32::from_bytes(revenue.record_account().to_bytes()),
        Id32::from_bytes(revenue.record_semantic_id().bytes()),
        Id32::from_bytes(revenue.policy_digest().bytes()),
        Id32::from_bytes(revenue.treasury_owner().bytes()),
        Id32::from_bytes(revenue.treasury_position_derivation_policy_id().bytes()),
        Id32::from_bytes(treasury.treasury_position_account().to_bytes()),
        Id32::from_bytes(treasury.treasury_service_ledger_account().to_bytes()),
    )?;
    let join_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            GENERAL_CURRENT_FOUNDING_JOIN_DOMAIN_V4,
            program_id.as_ref(),
            market_binding_account.as_ref(),
            market_runtime_account.as_ref(),
            &product.product_preauthorization_id().bytes(),
            &revenue.record_semantic_id().bytes(),
            treasury.treasury_position_account().as_ref(),
            treasury.treasury_replay_account().as_ref(),
            treasury.treasury_service_ledger_account().as_ref(),
        ])
        .to_bytes(),
    );
    require(!join_id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedGeneralMarketFoundingPlanV4 {
        founding_policy,
        base,
        authority,
        revenue,
        treasury,
        market_binding_account,
        market_binding_bump,
        market_runtime_account,
        market_runtime_bump,
        realm_id: product.realm_id(),
        collateral_policy_id: product.collateral_policy_id(),
        collateral_release_id: product.collateral_release_id(),
        join_id,
    })
}

}
