// SPDX-License-Identifier: AGPL-3.0-or-later

//! Current Product-to-General founding authority.
//!
//! This module owns the narrow join between one Product-current pre-root
//! authorization, the exact General MarketBinding V2 body it authenticated,
//! and one immutable Realm RevenuePolicyRecord V2.  It derives every General
//! and treasury PDA before any account is created.  The eventual atomic
//! founder consumes the private plan and returns an authenticated postwrite to
//! Product; no public field tuple can stand in for that authority.

use crate::accounts::{require, require_signer, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{require_creatable, require_system_program, RentParameters};
use crate::seeds;
use clutch_general_v2_contract::{
    CurrentMarketAuthorityV4, GeneralFoundingPolicyV1, Id32,
    MarketBindingV2, MarketBindingV4, MarketRuntimeV3AccountV1, Sha256BackendV1,
    GENERAL_REPLAY_ACCOUNT_V1_BYTES, MARKET_BINDING_ACCOUNT_BYTES_V4,
    MARKET_RUNTIME_ACCOUNT_BYTES,
};
use clutch_product_series::ContentId;
use clutch_retirement::POSITION_V3_BYTES;
use clutch_solana_layout::revenue::TREASURY_SERVICE_LEDGER_V1_BYTES;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::revenue_policy_v2::{
    derive_revenue_market_treasury_v1, found_revenue_market_treasury_v1,
    AuthenticatedRevenuePolicyRecordV2, AuthenticatedTreasuryMarketFactsV1,
    RevenueMarketTreasuryDerivationV1, RevenueMarketTreasuryFoundationV1,
};
use super::general_v2_settlement_producer_v5::{create_from_payer, rent_owner};
use super::collateral_position_v3::authenticate_general_market_v4_with_data_ids;

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
/// A concrete implementation must retain the exact authenticated RootV2,
/// LinkV2, BundleV6, QuoteV5, AttachmentV5, ScheduleV3, GraphV3, collateral
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

/// Exact writable accounts created by the current Product-to-General founder.
#[derive(Clone, Copy, Debug)]
pub(crate) struct GeneralMarketFoundationAccountFrameV4<'a, 'info> {
    /// Explicit signer and principal owner for MarketBindingV4 and RuntimeV3.
    pub(crate) general_rent_payer: &'a AccountInfo<'info>,
    /// Separately named signer and principal owner for Position/Replay/0xbb.
    pub(crate) treasury_rent_payer: &'a AccountInfo<'info>,
    /// Fresh canonical MarketBinding `0x79/v4` PDA.
    pub(crate) market_binding: &'a AccountInfo<'info>,
    /// Fresh canonical General MarketRuntime PDA.
    pub(crate) market_runtime: &'a AccountInfo<'info>,
    /// Fresh Market-scoped ordinary treasury PositionV3.
    pub(crate) treasury_position: &'a AccountInfo<'info>,
    /// Fresh mandatory GEN1 ReplayV3 for the treasury Position.
    pub(crate) treasury_replay: &'a AccountInfo<'info>,
    /// Fresh counted treasury-service ledger `0xbb/v1`.
    pub(crate) treasury_service_ledger: &'a AccountInfo<'info>,
    /// Canonical System program.
    pub(crate) system_program: &'a AccountInfo<'info>,
}

/// Private authenticated postwrite for the complete current General market
/// state and its separately funded treasury custody graph.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AuthenticatedGeneralMarketFoundingPostwriteV4 {
    binding_account: Pubkey,
    binding: MarketBindingV4,
    binding_data_id: Id32,
    runtime_account: Pubkey,
    runtime: MarketRuntimeV3AccountV1,
    runtime_data_id: Id32,
    treasury: RevenueMarketTreasuryFoundationV1,
    join_id: ContentId,
}

impl AuthenticatedGeneralMarketFoundingPostwriteV4 {
    pub(crate) const fn binding_account(self) -> Pubkey { self.binding_account }
    pub(crate) const fn binding(self) -> MarketBindingV4 { self.binding }
    pub(crate) const fn binding_data_id(self) -> Id32 { self.binding_data_id }
    pub(crate) const fn runtime_account(self) -> Pubkey { self.runtime_account }
    pub(crate) const fn runtime(self) -> MarketRuntimeV3AccountV1 { self.runtime }
    pub(crate) const fn runtime_data_id(self) -> Id32 { self.runtime_data_id }
    pub(crate) const fn treasury(self) -> RevenueMarketTreasuryFoundationV1 { self.treasury }
    pub(crate) const fn join_id(self) -> ContentId { self.join_id }
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

fn require_foundation_accounts_distinct(
    frame: GeneralMarketFoundationAccountFrameV4<'_, '_>,
) -> Outcome<()> {
    let accounts = [
        frame.market_binding,
        frame.market_runtime,
        frame.treasury_position,
        frame.treasury_replay,
        frame.treasury_service_ledger,
        frame.system_program,
    ];
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }
    for payer in [frame.general_rent_payer, frame.treasury_rent_payer] {
        for target in accounts {
            require(payer.key != target.key, ClutchError::AccountAlias)?;
        }
    }
    Ok(())
}

fn require_aggregate_principal(
    frame: GeneralMarketFoundationAccountFrameV4<'_, '_>,
    rent: &RentParameters,
) -> Outcome<()> {
    let general = rent
        .minimum_balance(MARKET_BINDING_ACCOUNT_BYTES_V4)?
        .checked_add(rent.minimum_balance(MARKET_RUNTIME_ACCOUNT_BYTES)?)
        .ok_or(ClutchError::Arithmetic)?;
    let service_ledger_principal = rent.minimum_balance(TREASURY_SERVICE_LEDGER_V1_BYTES)?;
    let treasury = rent
        .minimum_balance(POSITION_V3_BYTES)?
        .checked_add(rent.minimum_balance(GENERAL_REPLAY_ACCOUNT_V1_BYTES)?)
        .and_then(|value| value.checked_add(service_ledger_principal))
        .ok_or(ClutchError::Arithmetic)?;
    if frame.general_rent_payer.key == frame.treasury_rent_payer.key {
        require(
            frame.general_rent_payer.lamports()
                >= general.checked_add(treasury).ok_or(ClutchError::Arithmetic)?,
            ClutchError::AccountCreationFailed,
        )
    } else {
        require(
            frame.general_rent_payer.lamports() >= general
                && frame.treasury_rent_payer.lamports() >= treasury,
            ClutchError::AccountCreationFailed,
        )
    }
}

/// Atomically create and hostile-reauthenticate MarketBindingV4, RuntimeV3,
/// the ordinary treasury Position, mandatory Replay, and counted `0xbb`.
/// Both named payers transfer full rent principal; hostile prefunds remain
/// account-owned donation floors and never discount either payer.
#[inline(never)]
pub(crate) fn found_general_market_v4<P>(
    program_id: &Pubkey,
    frame: GeneralMarketFoundationAccountFrameV4<'_, '_>,
    rent: &RentParameters,
    plan: AuthenticatedGeneralMarketFoundingPlanV4,
    product: &P,
) -> Outcome<AuthenticatedGeneralMarketFoundingPostwriteV4>
where
    P: AuthenticatedCurrentProductGeneralFoundingV4,
{
    require_signer(frame.general_rent_payer)?;
    require_signer(frame.treasury_rent_payer)?;
    require_system_program(frame.system_program)?;
    require_foundation_accounts_distinct(frame)?;
    require_aggregate_principal(frame, rent)?;
    for account in [
        frame.market_binding,
        frame.market_runtime,
        frame.treasury_position,
        frame.treasury_replay,
        frame.treasury_service_ledger,
    ] {
        require_creatable(account)?;
    }
    require(
        *frame.market_binding.key == plan.market_binding_account
            && *frame.market_runtime.key == plan.market_runtime_account
            && *frame.treasury_position.key == plan.treasury.treasury_position_account()
            && *frame.treasury_replay.key == plan.treasury.treasury_replay_account()
            && *frame.treasury_service_ledger.key
                == plan.treasury.treasury_service_ledger_account(),
        ClutchError::WrongPda,
    )?;
    let binding_rent = rent_owner(
        frame.general_rent_payer,
        frame.market_binding,
        rent,
        MARKET_BINDING_ACCOUNT_BYTES_V4,
    )?;
    let runtime_rent = rent_owner(
        frame.general_rent_payer,
        frame.market_runtime,
        rent,
        MARKET_RUNTIME_ACCOUNT_BYTES,
    )?;
    let binding = MarketBindingV4::new(plan.base, plan.authority, binding_rent)?;
    let runtime = MarketRuntimeV3AccountV1 {
        market_binding: Id32::from_bytes(frame.market_binding.key.to_bytes()),
        market_instance_v2_id: plan.base.base().market_instance_v2_id,
        next_epoch_index: 0,
        next_epoch_generation: 1,
        created_epoch_count: 0,
        retired_epoch_count: 0,
        rent: runtime_rent,
        stored_bump: plan.market_runtime_bump,
        flags: 0,
    };
    runtime.validate()?;
    create_from_payer(
        program_id,
        frame.general_rent_payer,
        frame.market_binding,
        frame.system_program,
        rent,
        MARKET_BINDING_ACCOUNT_BYTES_V4,
        binding_rent,
        &[
            seeds::SEED_GENERAL_V2_MARKET_BINDING,
            &plan.base.base().market_instance_v2_id.bytes(),
            &[plan.market_binding_bump],
        ],
    )?;
    create_from_payer(
        program_id,
        frame.general_rent_payer,
        frame.market_runtime,
        frame.system_program,
        rent,
        MARKET_RUNTIME_ACCOUNT_BYTES,
        runtime_rent,
        &[
            seeds::SEED_GENERAL_V2_MARKET_RUNTIME,
            &frame.market_binding.key.to_bytes(),
            &[plan.market_runtime_bump],
        ],
    )?;
    {
        let mut output = frame
            .market_binding
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        binding.encode(&mut output)?;
    }
    {
        let mut output = frame
            .market_runtime
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        runtime.encode(&mut output)?;
    }
    let treasury = found_revenue_market_treasury_v1(
        program_id,
        frame.treasury_rent_payer,
        frame.treasury_position,
        frame.treasury_replay,
        frame.treasury_service_ledger,
        frame.system_program,
        rent,
        plan.treasury,
        product,
    )?;
    let persisted = authenticate_general_market_v4_with_data_ids(
        program_id,
        frame.market_binding,
        frame.market_runtime,
    )?;
    require(
        persisted.binding() == binding
            && persisted.runtime() == runtime
            && treasury.revenue_policy_record_account()
                == plan.revenue.record_account()
            && treasury.revenue_policy_record_v2_id().bytes()
                == plan.revenue.record_semantic_id().bytes()
            && treasury.revenue_policy_v2_digest().bytes()
                == plan.revenue.policy_digest().bytes()
            && treasury.treasury_owner().bytes() == plan.revenue.treasury_owner().bytes()
            && treasury.treasury_position_derivation_policy_v2_id().bytes()
                == plan
                    .revenue
                    .treasury_position_derivation_policy_id()
                    .bytes()
            && treasury.treasury_position_account()
                == plan.treasury.treasury_position_account()
            && treasury.treasury_service_ledger_account()
                == plan.treasury.treasury_service_ledger_account(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedGeneralMarketFoundingPostwriteV4 {
        binding_account: *frame.market_binding.key,
        binding,
        binding_data_id: Id32::from_bytes(persisted.binding_data_id().bytes()),
        runtime_account: *frame.market_runtime.key,
        runtime,
        runtime_data_id: Id32::from_bytes(persisted.runtime_data_id().bytes()),
        treasury,
        join_id: plan.join_id,
    })
}
