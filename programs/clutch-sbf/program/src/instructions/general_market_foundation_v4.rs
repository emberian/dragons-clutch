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
    found_general_position_v1, found_general_replay_v1, CurrentMarketAuthorityV4,
    CurrentMarketAuthorityV5, DeletableRentOwnerV1 as GeneralDeletableRentOwnerV1,
    GeneralFoundingPolicyV1, Id32,
    MarketBindingV2, MarketBindingV4, MarketBindingV5, MarketRuntimeV3AccountV1,
    Sha256BackendV1, GENERAL_POSITION_FOUNDING_GENERATION_V1,
    GENERAL_REPLAY_ACCOUNT_V1_BYTES, MARKET_BINDING_ACCOUNT_BYTES_V5,
    MARKET_RUNTIME_ACCOUNT_BYTES,
};
use clutch_product_series::{ContentId, MarketFoundationSlotV4, MarketInstanceV2Id};
use clutch_retirement::{
    DeletableRentOwnerV1 as RetirementDeletableRentOwnerV1, Identity32V1,
    PositionAccountV3, PositionPurposeV3, PositionV3Sha256Backend, RentSplitV2,
    ReplayV3Envelope, ReplayV3HashBackend, POSITION_TOMBSTONE_V3_BYTES, POSITION_V3_BYTES,
};
use clutch_solana_layout::revenue::{
    TreasuryServiceLedgerV1, TREASURY_SERVICE_LEDGER_V1_BYTES,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::revenue_policy_v2::{
    derive_revenue_market_treasury_v1, AuthenticatedRevenuePolicyRecordV2,
    RevenueMarketTreasuryDerivationV1,
};
use super::product_market_lifecycle_v3_current::{
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

impl PositionV3Sha256Backend for RuntimeSha256 {
    fn sha256(&self, domain: &[u8], body: &[u8]) -> [u8; 32] {
        solana_sha256_hasher::hashv(&[domain, body]).to_bytes()
    }
}

impl ReplayV3HashBackend for RuntimeSha256 {
    fn sha256_parts(&self, parts: &[&[u8]]) -> [u8; 32] {
        solana_sha256_hasher::hashv(parts).to_bytes()
    }
}

/// Product-owned current founding authority consumed by the General join.
///
/// A concrete implementation must retain the exact authenticated RootV2,
/// LinkV2, BundleV6, QuoteV5, AttachmentV5, ScheduleV3, GraphV3, collateral
/// founding, and General-policy evidence.  The default authentication method
/// refuses, so getters alone never confer authority.
pub(crate) trait AuthenticatedCurrentProductGeneralFoundingV4 {
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
    fn series_market_link_v2_id(&self) -> Id32;
    fn series_ordinal(&self) -> u32;
    fn compiler_bundle_v6_id(&self) -> Id32;
    fn funding_quote_v5_id(&self) -> Id32;
    fn attachment_plan_v5_id(&self) -> Id32;
    fn foundation_schedule_v3_id(&self) -> Id32;
    fn foundation_account_graph_v3_id(&self) -> Id32;
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

/// Derive and authenticate the complete current General founding graph.
pub(crate) fn prepare_general_market_founding_v4<P>(
    program_id: &Pubkey,
    product: &P,
    founding_policy_bytes: &[u8],
    base: MarketBindingV2,
    revenue: AuthenticatedRevenuePolicyRecordV2,
) -> Outcome<AuthenticatedGeneralMarketFoundingPlanV4>
where
    P: AuthenticatedCurrentProductGeneralFoundingV4 + ?Sized,
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
        product.series_market_link_v2_id(),
        product.series_ordinal(),
        product.compiler_bundle_v6_id(),
        product.funding_quote_v5_id(),
        product.attachment_plan_v5_id(),
        product.foundation_schedule_v3_id(),
        product.foundation_account_graph_v3_id(),
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

/// Private proof returned only after every General/treasury account was
/// hostile-reauthenticated from its exact postwrite.
pub(crate) trait AuthenticatedGeneralMarketFoundingPostwriteV4 {
    fn authenticate_general_market_founding_postwrite_v4(
        &self,
        _plan: &AuthenticatedGeneralMarketFoundingPlanV4,
        _binding: &MarketBindingV4,
        _runtime: &MarketRuntimeV3AccountV1,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

const GENERAL_CURRENT_FOUNDING_JOIN_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/general/current-founding-join/v5\0";
const GENERAL_PRODUCT_FUNDED_SLOT_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/general/product-funded-slot/v5\0";

/// Sole Product-current authority accepted by the General V5 founder.
/// Implementations must own the exact hostile-authenticated RootV3, LinkV3,
/// and physical FundingV5 founder. The default method refuses, so the getters
/// are descriptive and cannot mint a General account by themselves.
pub(crate) trait AuthenticatedCurrentProductGeneralFoundingV5 {
    fn authenticate_current_product_general_founding_v5(
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
    fn product_market_binding_v3_id(&self) -> Id32;
    fn product_generation(&self) -> u64;
    fn series_market_link_account(&self) -> Id32;
    fn series_market_link_v3_id(&self) -> Id32;
    fn series_ordinal(&self) -> u32;
    fn compiler_bundle_v7_id(&self) -> Id32;
    fn funding_quote_v6_id(&self) -> Id32;
    fn attachment_plan_v6_id(&self) -> Id32;
    fn foundation_schedule_v4_id(&self) -> Id32;
    fn foundation_account_graph_v4_id(&self) -> Id32;
    fn series_funding_v5_account(&self) -> Id32;
    fn physical_capitalization_receipt_id(&self) -> Id32;
    fn market_liability_founding_id(&self) -> Id32;
    fn claim_mint_founding_plan_id(&self) -> Id32;
    fn claim_issuance_binding_id(&self) -> Id32;
    fn general_founding_capability_id(&self) -> Id32;
    fn product_preauthorization_id(&self) -> Id32;
    fn realm_id(&self) -> Id32;
    fn collateral_policy_id(&self) -> Id32;
    fn collateral_release_id(&self) -> Id32;
}

/// Move-only exact V5 founding plan. It retains the full authenticated
/// Revenue record and treasury derivation rather than copying policy facts
/// into an adapter DTO.
#[derive(Debug)]
pub(crate) struct AuthenticatedGeneralMarketFoundingPlanV5 {
    founding_policy: GeneralFoundingPolicyV1,
    base: MarketBindingV2,
    authority: CurrentMarketAuthorityV5,
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

impl AuthenticatedGeneralMarketFoundingPlanV5 {
    pub(crate) const fn founding_policy(&self) -> GeneralFoundingPolicyV1 { self.founding_policy }
    pub(crate) const fn base(&self) -> &MarketBindingV2 { &self.base }
    pub(crate) const fn authority(&self) -> CurrentMarketAuthorityV5 { self.authority }
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

/// Authenticate the fresh General V5 graph from the sole Product V3/FundingV5
/// owner and immutable Realm RevenuePolicyRecordV2.
pub(crate) fn prepare_general_market_founding_v5<P>(
    program_id: &Pubkey,
    product: &P,
    founding_policy_bytes: &[u8],
    base: MarketBindingV2,
    revenue: AuthenticatedRevenuePolicyRecordV2,
) -> Outcome<AuthenticatedGeneralMarketFoundingPlanV5>
where
    P: AuthenticatedCurrentProductGeneralFoundingV5 + ?Sized,
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
    product.authenticate_current_product_general_founding_v5(
        program_id,
        market_binding_account,
        market_runtime_account,
        founding_policy,
        &base,
        &revenue,
        &treasury,
    )?;
    require(
        relation.neutral_sink.bytes() != revenue.treasury_owner().bytes(),
        ClutchError::MismatchedState,
    )?;
    let authority = CurrentMarketAuthorityV5::new(
        product.product_market_root_account(),
        product.product_market_binding_v3_id(),
        product.product_generation(),
        product.series_market_link_account(),
        product.series_market_link_v3_id(),
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
        product.series_funding_v5_account(),
        product.physical_capitalization_receipt_id(),
    )?;
    let join_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            GENERAL_CURRENT_FOUNDING_JOIN_DOMAIN_V5,
            program_id.as_ref(),
            market_binding_account.as_ref(),
            market_runtime_account.as_ref(),
            &product.product_market_binding_v3_id().bytes(),
            &product.series_market_link_v3_id().bytes(),
            &product.physical_capitalization_receipt_id().bytes(),
            &product.product_preauthorization_id().bytes(),
            &revenue.record_semantic_id().bytes(),
            treasury.treasury_position_account().as_ref(),
            treasury.treasury_replay_account().as_ref(),
            treasury.treasury_service_ledger_account().as_ref(),
        ])
        .to_bytes(),
    );
    require(join_id != ContentId::ZERO, ClutchError::MismatchedState)?;
    Ok(AuthenticatedGeneralMarketFoundingPlanV5 {
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

/// Private move-only postwrite for one Product-funded General V5 slot.
#[derive(Debug)]
pub(crate) struct AuthenticatedGeneralFoundationPostwriteV5 {
    debit_id: ContentId,
    founder_creation_receipt_id: ContentId,
    founder_preauthorization_id: ContentId,
    foundation_steps_id: ContentId,
    market_binding_id: ContentId,
    foundation_schedule_id: ContentId,
    foundation_graph_id: ContentId,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    slot: MarketFoundationSlotV4,
    root_transition_sequence_after: u64,
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

fn allocate_assign_product_funded_pda<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    account_bytes: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(account_bytes),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(&allocate, &[account.clone(), system_program.clone()], &[signer_seeds])
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(&assign, &[account.clone(), system_program.clone()], &[signer_seeds])
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))
}

fn authenticate_product_funded_debit_v5(
    plan: &AuthenticatedGeneralMarketFoundingPlanV5,
    debit: &AuthenticatedProductMarketFoundationDebitV4,
    account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    slot: MarketFoundationSlotV4,
    expected_account: Pubkey,
    expected_bytes: usize,
    rent: &RentParameters,
) -> Outcome<()> {
    require_system_program(system_program)?;
    let principal = rent.minimum_balance(expected_bytes)?;
    let authority = plan.authority;
    require(
        debit.id() != ContentId::ZERO
            && debit.slot() == slot
            && debit.root_transition_sequence_after() != 0
            && debit.destination_account() == expected_account
            && *account.key == expected_account
            && debit.market_instance_id().bytes() == plan.base.base().market_instance_v2_id.bytes()
            && debit.generation() == authority.product_generation()
            && debit.market_binding_id().bytes()
                == authority.product_market_binding_v3_id().bytes()
            && debit.foundation_schedule_id().bytes()
                == authority.foundation_schedule_v4_id().bytes()
            && debit.foundation_graph_id().bytes()
                == authority.foundation_account_graph_v4_id().bytes()
            && debit.founder_preauthorization_id().bytes()
                == authority.product_preauthorization_id().bytes()
            && debit.principal_lamports() == principal
            && debit.principal_before_lamports()
                == debit.principal_after_lamports()
                    .checked_add(principal).ok_or(ClutchError::Arithmetic)?
            && debit.destination_balance_after_lamports()
                == debit.destination_donation_floor_lamports()
                    .checked_add(principal).ok_or(ClutchError::Arithmetic)?
            && debit.vault_donation_after_lamports() == debit.vault_donation_before_lamports()
            && account.lamports() == debit.destination_balance_after_lamports()
            && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && account.is_writable
            && !account.is_signer
            && !account.executable
            && account.data_len() == 0
            && debit.foundation_vault_account() != *account.key
            && debit.rent_refund_owner() != *account.key
            && debit.neutral_lamport_sink() != *account.key
            && debit.foundation_vault_account() != debit.rent_refund_owner()
            && debit.foundation_vault_account() != debit.neutral_lamport_sink()
            && debit.rent_refund_owner() != debit.neutral_lamport_sink()
            && debit.neutral_lamport_sink().to_bytes() == plan.base.base().neutral_sink.bytes(),
        ClutchError::MismatchedState,
    )
}

pub(super) fn product_funded_slot_postwrite_v5(
    program_id: &Pubkey,
    plan: &AuthenticatedGeneralMarketFoundingPlanV5,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    account: &AccountInfo<'_>,
    slot: MarketFoundationSlotV4,
    account_data_id: ContentId,
) -> Outcome<AuthenticatedGeneralFoundationPostwriteV5> {
    let slot_index = u8::try_from(slot.index().map_err(|_| ClutchError::MismatchedState)?)
        .map_err(|_| ClutchError::Arithmetic)?;
    let accepted_poststate_receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            GENERAL_PRODUCT_FUNDED_SLOT_DOMAIN_V5,
            program_id.as_ref(),
            &plan.join_id.bytes(),
            &debit.id().bytes(),
            &[slot_index],
            &debit.root_transition_sequence_after().to_le_bytes(),
            account.key.as_ref(),
            &account_data_id.bytes(),
            &debit.principal_lamports().to_le_bytes(),
            &debit.destination_donation_floor_lamports().to_le_bytes(),
            &debit.destination_balance_after_lamports().to_le_bytes(),
            debit.rent_refund_owner().as_ref(),
            debit.neutral_lamport_sink().as_ref(),
        ])
        .to_bytes(),
    );
    require(
        account_data_id != ContentId::ZERO && accepted_poststate_receipt_id != ContentId::ZERO,
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedGeneralFoundationPostwriteV5 {
        debit_id: debit.id(),
        founder_creation_receipt_id: debit.founder_creation_receipt_id(),
        founder_preauthorization_id: debit.founder_preauthorization_id(),
        foundation_steps_id: debit.foundation_steps_id(),
        market_binding_id: debit.market_binding_id(),
        foundation_schedule_id: debit.foundation_schedule_id(),
        foundation_graph_id: debit.foundation_graph_id(),
        market_instance_id: debit.market_instance_id(),
        generation: debit.generation(),
        slot,
        root_transition_sequence_after: debit.root_transition_sequence_after(),
        account: *account.key,
        principal_lamports: debit.principal_lamports(),
        principal_before_lamports: debit.principal_before_lamports(),
        principal_after_lamports: debit.principal_after_lamports(),
        destination_donation_floor_lamports: debit.destination_donation_floor_lamports(),
        destination_balance_after_lamports: debit.destination_balance_after_lamports(),
        vault_donation_before_lamports: debit.vault_donation_before_lamports(),
        vault_donation_after_lamports: debit.vault_donation_after_lamports(),
        foundation_vault_account: debit.foundation_vault_account(),
        rent_refund_owner: debit.rent_refund_owner(),
        neutral_lamport_sink: debit.neutral_lamport_sink(),
        account_data_id,
        accepted_poststate_receipt_id,
    })
}

impl AuthenticatedProductMarketFoundationStepPostwriteV4
    for AuthenticatedGeneralFoundationPostwriteV5
{
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v4(
        self,
        debit_id: ContentId,
        founder_creation_receipt_id: ContentId,
        founder_preauthorization_id: ContentId,
        foundation_steps_id: ContentId,
        market_binding_id: ContentId,
        foundation_schedule_id: ContentId,
        foundation_graph_id: ContentId,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        slot: MarketFoundationSlotV4,
        root_transition_sequence_after: u64,
        account_id: ContentId,
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
    ) -> Outcome<(ContentId, u64)> {
        require(
            debit_id == self.debit_id
                && founder_creation_receipt_id == self.founder_creation_receipt_id
                && founder_preauthorization_id == self.founder_preauthorization_id
                && foundation_steps_id == self.foundation_steps_id
                && market_binding_id == self.market_binding_id
                && foundation_schedule_id == self.foundation_schedule_id
                && foundation_graph_id == self.foundation_graph_id
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && slot == self.slot
                && root_transition_sequence_after == self.root_transition_sequence_after
                && account_id.bytes() == self.account.to_bytes()
                && principal_lamports == self.principal_lamports
                && principal_before_lamports == self.principal_before_lamports
                && principal_after_lamports == self.principal_after_lamports
                && destination_donation_floor_lamports
                    == self.destination_donation_floor_lamports
                && destination_balance_after_lamports == self.destination_balance_after_lamports
                && vault_donation_before_lamports == self.vault_donation_before_lamports
                && vault_donation_after_lamports == self.vault_donation_after_lamports
                && foundation_vault_account == self.foundation_vault_account
                && rent_refund_owner == self.rent_refund_owner
                && neutral_lamport_sink == self.neutral_lamport_sink
                && self.account_data_id != ContentId::ZERO
                && self.accepted_poststate_receipt_id != ContentId::ZERO,
            ClutchError::MismatchedState,
        )?;
        Ok((self.accepted_poststate_receipt_id, self.vault_donation_after_lamports))
    }
}

/// Product-funded ScheduleV4 slot 1 writer for fresh MarketBindingV5.
#[inline(never)]
pub(crate) fn write_product_funded_market_binding_v5(
    program_id: &Pubkey,
    plan: &AuthenticatedGeneralMarketFoundingPlanV5,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &RentParameters,
) -> Outcome<AuthenticatedGeneralFoundationPostwriteV5> {
    authenticate_product_funded_debit_v5(
        plan,
        &debit,
        account,
        system_program,
        MarketFoundationSlotV4::MarketBinding,
        plan.market_binding_account,
        MARKET_BINDING_ACCOUNT_BYTES_V5,
        rent,
    )?;
    let rent_owner = GeneralDeletableRentOwnerV1 {
        payer: Id32::from_bytes(debit.rent_refund_owner().to_bytes()),
        refundable_principal: debit.principal_lamports(),
        donation_floor: debit.destination_donation_floor_lamports(),
    };
    rent_owner.validate()?;
    let body = MarketBindingV5::new(plan.base, plan.authority, rent_owner)?;
    let market = plan.base.base().market_instance_v2_id.bytes();
    let bump = [plan.market_binding_bump];
    allocate_assign_product_funded_pda(
        program_id,
        account,
        system_program,
        MARKET_BINDING_ACCOUNT_BYTES_V5,
        &[seeds::SEED_GENERAL_V2_MARKET_BINDING, &market, &bump],
    )?;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        body.encode(&mut data)?;
        require(MarketBindingV5::decode(&data)? == body, ClutchError::MismatchedState)?;
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let data_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/general-market-binding/data/v5\0",
            account.key.as_ref(),
            &data,
        ])
        .to_bytes(),
    );
    drop(data);
    product_funded_slot_postwrite_v5(
        program_id,
        plan,
        debit,
        account,
        MarketFoundationSlotV4::MarketBinding,
        data_id,
    )
}

/// Product-funded ScheduleV4 slot 2 writer for the canonical RuntimeV3.
#[inline(never)]
pub(crate) fn write_product_funded_market_runtime_v3_for_v5(
    program_id: &Pubkey,
    plan: &AuthenticatedGeneralMarketFoundingPlanV5,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &RentParameters,
) -> Outcome<AuthenticatedGeneralFoundationPostwriteV5> {
    authenticate_product_funded_debit_v5(
        plan,
        &debit,
        account,
        system_program,
        MarketFoundationSlotV4::MarketRuntime,
        plan.market_runtime_account,
        MARKET_RUNTIME_ACCOUNT_BYTES,
        rent,
    )?;
    let rent_owner = GeneralDeletableRentOwnerV1 {
        payer: Id32::from_bytes(debit.rent_refund_owner().to_bytes()),
        refundable_principal: debit.principal_lamports(),
        donation_floor: debit.destination_donation_floor_lamports(),
    };
    rent_owner.validate()?;
    let body = MarketRuntimeV3AccountV1 {
        market_binding: Id32::from_bytes(plan.market_binding_account.to_bytes()),
        market_instance_v2_id: plan.base.base().market_instance_v2_id,
        next_epoch_index: 0,
        next_epoch_generation: 1,
        created_epoch_count: 0,
        retired_epoch_count: 0,
        rent: rent_owner,
        stored_bump: plan.market_runtime_bump,
        flags: 0,
    };
    body.validate()?;
    let binding = plan.market_binding_account.to_bytes();
    let bump = [plan.market_runtime_bump];
    allocate_assign_product_funded_pda(
        program_id,
        account,
        system_program,
        MARKET_RUNTIME_ACCOUNT_BYTES,
        &[seeds::SEED_GENERAL_V2_MARKET_RUNTIME, &binding, &bump],
    )?;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        body.encode(&mut data)?;
        require(
            MarketRuntimeV3AccountV1::decode(&data)? == body,
            ClutchError::MismatchedState,
        )?;
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let data_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/general-market-runtime/data/v3\0",
            account.key.as_ref(),
            &data,
        ])
        .to_bytes(),
    );
    drop(data);
    product_funded_slot_postwrite_v5(
        program_id,
        plan,
        debit,
        account,
        MarketFoundationSlotV4::MarketRuntime,
        data_id,
    )
}

fn identity32(value: [u8; 32]) -> Outcome<Identity32V1> {
    Identity32V1::new(value)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
}

fn product_funded_account_data_id_v5(
    domain: &[u8],
    account: &AccountInfo<'_>,
) -> Outcome<ContentId> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[domain, account.key.as_ref(), &data]).to_bytes(),
    );
    require(id != ContentId::ZERO, ClutchError::MismatchedState)?;
    Ok(id)
}

fn reconstruct_treasury_position_v5(
    program_id: &Pubkey,
    plan: &AuthenticatedGeneralMarketFoundingPlanV5,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
) -> Outcome<clutch_general_v2_contract::GeneralPositionFoundingPlanV1> {
    let derivation = plan.treasury;
    require(
        *position_account.key == derivation.treasury_position_account()
            && *replay_account.key == derivation.treasury_replay_account()
            && position_account.owner == program_id
            && position_account.data_len() == POSITION_V3_BYTES
            && position_account.key != replay_account.key
            && !position_account.is_signer
            && !position_account.executable,
        ClutchError::MismatchedState,
    )?;
    let position_data = position_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let decoded = PositionAccountV3::decode(&position_data)?;
    let position_rent = decoded.rent();
    let expected = found_general_position_v1(
        identity32(position_account.key.to_bytes())?,
        identity32(replay_account.key.to_bytes())?,
        identity32(plan.base.base().market_instance_v2_id.bytes())?,
        identity32(plan.realm_id.bytes())?,
        identity32(plan.collateral_policy_id.bytes())?,
        identity32(plan.collateral_release_id.bytes())?,
        identity32(plan.revenue.treasury_owner().bytes())?,
        identity32(plan.market_runtime_account.to_bytes())?,
        plan.base.base().outcome_count,
        derivation.treasury_position_bump(),
        position_rent,
        &RuntimeSha256,
    )?;
    let minimum_balance = position_rent
        .refundable_live_principal
        .checked_add(position_rent.permanent_tombstone_principal)
        .and_then(|value| value.checked_add(position_rent.donation_floor))
        .ok_or(ClutchError::Arithmetic)?;
    require(
        position_data.as_ref() == expected.position_body()
            && decoded == expected.position()
            && position_account.lamports() >= minimum_balance,
        ClutchError::MismatchedState,
    )?;
    Ok(expected)
}

fn authenticate_treasury_pair_v5(
    program_id: &Pubkey,
    plan: &AuthenticatedGeneralMarketFoundingPlanV5,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
) -> Outcome<clutch_general_v2_contract::GeneralPositionFoundingPlanV1> {
    require(
        position_account.owner == program_id
            && replay_account.owner == program_id
            && replay_account.data_len() == GENERAL_REPLAY_ACCOUNT_V1_BYTES
            && !replay_account.is_signer
            && !replay_account.executable,
        ClutchError::MismatchedState,
    )?;
    let expected_position = reconstruct_treasury_position_v5(
        program_id,
        plan,
        position_account,
        replay_account,
    )?;
    let replay_data = replay_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let decoded_replay = ReplayV3Envelope::decode(&replay_data, &RuntimeSha256)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay_rent = decoded_replay.header().rent();
    let expected_replay = found_general_replay_v1(
        identity32(position_account.key.to_bytes())?,
        identity32(replay_account.key.to_bytes())?,
        identity32(plan.revenue.treasury_owner().bytes())?,
        identity32(plan.market_runtime_account.to_bytes())?,
        plan.treasury.treasury_replay_bump(),
        replay_rent,
        expected_position.position_semantic_id(),
        &RuntimeSha256,
    )?;
    let replay_minimum = replay_rent
        .refundable_principal()
        .checked_add(replay_rent.donation_floor())
        .ok_or(ClutchError::Arithmetic)?;
    require(
        replay_data.as_ref() == expected_replay.replay_body()
            && decoded_replay.header().next_sequence() == 0
            && decoded_replay.header().position_generation()
                == GENERAL_POSITION_FOUNDING_GENERATION_V1
            && replay_account.lamports() >= replay_minimum,
        ClutchError::MismatchedState,
    )?;
    Ok(expected_position)
}

/// Product-funded ScheduleV4 slot 47 writer. Product's FoundationVault pays
/// the entire live principal; predictable-PDA prefund is persisted only as a
/// donation floor and cannot discount that principal.
#[inline(never)]
pub(crate) fn write_product_funded_treasury_position_v3_for_v5(
    program_id: &Pubkey,
    plan: &AuthenticatedGeneralMarketFoundingPlanV5,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &RentParameters,
) -> Outcome<AuthenticatedGeneralFoundationPostwriteV5> {
    let derivation = plan.treasury;
    authenticate_product_funded_debit_v5(
        plan,
        &debit,
        account,
        system_program,
        MarketFoundationSlotV4::GeneralTreasuryPosition,
        derivation.treasury_position_account(),
        POSITION_V3_BYTES,
        rent,
    )?;
    require(
        account.key != &derivation.treasury_replay_account()
            && account.key != &derivation.treasury_service_ledger_account()
            && account.key != &plan.market_binding_account
            && account.key != &plan.market_runtime_account,
        ClutchError::AccountAlias,
    )?;
    let tombstone_principal = rent.minimum_balance(POSITION_TOMBSTONE_V3_BYTES)?;
    let refundable_principal = debit
        .principal_lamports()
        .checked_sub(tombstone_principal)
        .ok_or(ClutchError::WrongRentSysvar)?;
    let position_rent = RentSplitV2 {
        payer: identity32(debit.rent_refund_owner().to_bytes())?,
        refundable_live_principal: refundable_principal,
        permanent_tombstone_principal: tombstone_principal,
        donation_floor: debit.destination_donation_floor_lamports(),
    };
    position_rent
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let owner = plan.revenue.treasury_owner().bytes();
    let position = found_general_position_v1(
        identity32(account.key.to_bytes())?,
        identity32(derivation.treasury_replay_account().to_bytes())?,
        identity32(plan.base.base().market_instance_v2_id.bytes())?,
        identity32(plan.realm_id.bytes())?,
        identity32(plan.collateral_policy_id.bytes())?,
        identity32(plan.collateral_release_id.bytes())?,
        identity32(owner)?,
        identity32(plan.market_runtime_account.to_bytes())?,
        plan.base.base().outcome_count,
        derivation.treasury_position_bump(),
        position_rent,
        &RuntimeSha256,
    )?;
    let market = plan.base.base().market_instance_v2_id.bytes();
    let runtime = plan.market_runtime_account.to_bytes();
    let purpose = [u8::from(PositionPurposeV3::General)];
    let bump = [derivation.treasury_position_bump()];
    allocate_assign_product_funded_pda(
        program_id,
        account,
        system_program,
        POSITION_V3_BYTES,
        &[
            clutch_retirement::POSITION_V3_PDA_PREFIX,
            &market,
            &owner,
            &purpose,
            &runtime,
            &bump,
        ],
    )?;
    {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        data.copy_from_slice(position.position_body());
        require(
            PositionAccountV3::decode(&data)? == position.position(),
            ClutchError::MismatchedState,
        )?;
    }
    let data_id = product_funded_account_data_id_v5(
        b"dragons-clutch/sbf/general-treasury-position/data/v3\0",
        account,
    )?;
    product_funded_slot_postwrite_v5(
        program_id,
        plan,
        debit,
        account,
        MarketFoundationSlotV4::GeneralTreasuryPosition,
        data_id,
    )
}

/// Product-funded ScheduleV4 slot 48 writer. It hostile-reconstructs the
/// exact slot-47 Position before deriving the Replay; no caller supplies a
/// Position semantic ID.
#[inline(never)]
pub(crate) fn write_product_funded_treasury_replay_v3_for_v5(
    program_id: &Pubkey,
    plan: &AuthenticatedGeneralMarketFoundingPlanV5,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &RentParameters,
) -> Outcome<AuthenticatedGeneralFoundationPostwriteV5> {
    let derivation = plan.treasury;
    authenticate_product_funded_debit_v5(
        plan,
        &debit,
        replay_account,
        system_program,
        MarketFoundationSlotV4::GeneralTreasuryReplay,
        derivation.treasury_replay_account(),
        GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        rent,
    )?;
    let expected_position = reconstruct_treasury_position_v5(
        program_id,
        plan,
        position_account,
        replay_account,
    )?;
    let replay_rent = RetirementDeletableRentOwnerV1::from_persisted(
        identity32(debit.rent_refund_owner().to_bytes())?,
        debit.principal_lamports(),
        debit.destination_donation_floor_lamports(),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let replay = found_general_replay_v1(
        identity32(position_account.key.to_bytes())?,
        identity32(replay_account.key.to_bytes())?,
        identity32(plan.revenue.treasury_owner().bytes())?,
        identity32(plan.market_runtime_account.to_bytes())?,
        derivation.treasury_replay_bump(),
        replay_rent,
        expected_position.position_semantic_id(),
        &RuntimeSha256,
    )?;
    let position = position_account.key.to_bytes();
    let runtime = plan.market_runtime_account.to_bytes();
    let purpose = [u8::from(PositionPurposeV3::General)];
    let bump = [derivation.treasury_replay_bump()];
    allocate_assign_product_funded_pda(
        program_id,
        replay_account,
        system_program,
        GENERAL_REPLAY_ACCOUNT_V1_BYTES,
        &[
            clutch_retirement::PURPOSE_REPLAY_V3_PDA_PREFIX,
            &position,
            &purpose,
            &runtime,
            &bump,
        ],
    )?;
    {
        let mut data = replay_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        data.copy_from_slice(replay.replay_body());
        let persisted = ReplayV3Envelope::decode(&data, &RuntimeSha256)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            persisted.header().next_sequence() == 0
                && persisted.header().position_generation()
                    == GENERAL_POSITION_FOUNDING_GENERATION_V1
                && persisted.header().position_account().bytes()
                    == position_account.key.to_bytes()
                && persisted.header().replay_account().bytes()
                    == replay_account.key.to_bytes()
                && persisted.header().purpose_binding_id().bytes()
                    == plan.market_runtime_account.to_bytes(),
            ClutchError::MismatchedState,
        )?;
    }
    let data_id = product_funded_account_data_id_v5(
        b"dragons-clutch/sbf/general-treasury-replay/data/v3\0",
        replay_account,
    )?;
    product_funded_slot_postwrite_v5(
        program_id,
        plan,
        debit,
        replay_account,
        MarketFoundationSlotV4::GeneralTreasuryReplay,
        data_id,
    )
}

/// Product-funded ScheduleV4 slot 49 writer. Both preceding ordinary
/// Position/Replay accounts are reauthenticated from their full bodies before
/// the Revenue-owned zero-count ledger is created.
#[inline(never)]
pub(crate) fn write_product_funded_treasury_service_ledger_v1_for_v5(
    program_id: &Pubkey,
    plan: &AuthenticatedGeneralMarketFoundingPlanV5,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    position_account: &AccountInfo<'_>,
    replay_account: &AccountInfo<'_>,
    ledger_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent: &RentParameters,
) -> Outcome<AuthenticatedGeneralFoundationPostwriteV5> {
    let derivation = plan.treasury;
    authenticate_product_funded_debit_v5(
        plan,
        &debit,
        ledger_account,
        system_program,
        MarketFoundationSlotV4::TreasuryServiceLedger,
        derivation.treasury_service_ledger_account(),
        TREASURY_SERVICE_LEDGER_V1_BYTES,
        rent,
    )?;
    require(
        position_account.key != replay_account.key
            && position_account.key != ledger_account.key
            && replay_account.key != ledger_account.key,
        ClutchError::AccountAlias,
    )?;
    authenticate_treasury_pair_v5(
        program_id,
        plan,
        position_account,
        replay_account,
    )?;
    let body = TreasuryServiceLedgerV1 {
        realm: clutch_solana_layout::Hash32::from_bytes(plan.realm_id.bytes()),
        revenue_policy_record_account: clutch_solana_layout::Hash32::from_bytes(
            plan.revenue.record_account().to_bytes(),
        ),
        revenue_policy_record_v2_id: plan.revenue.record_semantic_id(),
        market_instance_v2_id: clutch_solana_layout::Hash32::from_bytes(
            plan.base.base().market_instance_v2_id.bytes(),
        ),
        treasury_owner: plan.revenue.treasury_owner(),
        treasury_position_account: clutch_solana_layout::Hash32::from_bytes(
            position_account.key.to_bytes(),
        ),
        treasury_position_generation: GENERAL_POSITION_FOUNDING_GENERATION_V1,
        admitted_epoch_count: 0,
        settled_epoch_count: 0,
        rent_payer: clutch_solana_layout::Hash32::from_bytes(
            debit.rent_refund_owner().to_bytes(),
        ),
        refundable_rent_principal: debit.principal_lamports(),
        donation_floor: debit.destination_donation_floor_lamports(),
        stored_bump: derivation.treasury_service_ledger_bump(),
        flags: 0,
    };
    body.validate()?;
    let market = plan.base.base().market_instance_v2_id.bytes();
    let position = position_account.key.to_bytes();
    let bump = [derivation.treasury_service_ledger_bump()];
    allocate_assign_product_funded_pda(
        program_id,
        ledger_account,
        system_program,
        TREASURY_SERVICE_LEDGER_V1_BYTES,
        &[
            seeds::SEED_TREASURY_SERVICE_LEDGER_V1,
            &market,
            &position,
            &bump,
        ],
    )?;
    {
        let mut data = ledger_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        body.encode(&mut data)?;
        require(
            TreasuryServiceLedgerV1::decode(&data)? == body,
            ClutchError::MismatchedState,
        )?;
    }
    let data_id = product_funded_account_data_id_v5(
        b"dragons-clutch/sbf/treasury-service-ledger/data/v1\0",
        ledger_account,
    )?;
    product_funded_slot_postwrite_v5(
        program_id,
        plan,
        debit,
        ledger_account,
        MarketFoundationSlotV4::TreasuryServiceLedger,
        data_id,
    )
}

#[cfg(test)]
mod v5_product_funded_source_tests {
    #[test]
    fn current_general_slots_have_no_signer_payer_or_v4_binding_writer() {
        let source = include_str!("general_market_foundation_v4.rs");
        for writer in [
            "write_product_funded_market_binding_v5",
            "write_product_funded_market_runtime_v3_for_v5",
            "write_product_funded_treasury_position_v3_for_v5",
            "write_product_funded_treasury_replay_v3_for_v5",
            "write_product_funded_treasury_service_ledger_v1_for_v5",
        ] {
            let body = source
                .split(&format!("pub(crate) fn {writer}"))
                .nth(1)
                .and_then(|value| value.split("\n}\n").next())
                .expect("bounded Product-funded writer");
            assert!(body.contains("AuthenticatedProductMarketFoundationDebitV4"));
            assert!(!body.contains("require_signer"));
        }
        assert!(!source.contains("write_product_funded_market_binding_v4"));
    }

    #[test]
    fn treasury_successors_hostile_reauthenticate_predecessor_bodies() {
        let source = include_str!("general_market_foundation_v4.rs");
        assert!(source.contains("PositionAccountV3::decode"));
        assert!(source.contains("ReplayV3Envelope::decode"));
        assert!(source.contains("found_general_position_v1"));
        assert!(source.contains("found_general_replay_v1"));
        assert!(source.contains("TreasuryServiceLedgerV1::decode"));
    }
}
