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
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::revenue_policy_v2::{
    derive_revenue_market_treasury_v1, AuthenticatedRevenuePolicyRecordV2,
    AuthenticatedTreasuryMarketFactsV1, RevenueMarketTreasuryDerivationV1,
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

/// Atomically create and hostile-reauthenticate MarketBindingV4, RuntimeV3,
/// the ordinary treasury Position, mandatory Replay, and counted `0xbb`.
/// Both named payers transfer full rent principal; hostile prefunds remain
/// account-owned donation floors and never discount either payer.
#[cfg(any())]
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

fn authenticate_core_debit_v4(
    plan: &AuthenticatedGeneralMarketFoundingPlanV4,
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
            && debit.destination_account() == expected_account
            && *account.key == expected_account
            && debit.market_instance_id().bytes() == plan.base.base().market_instance_v2_id.bytes()
            && debit.generation() == authority.product_generation()
            && debit.market_binding_id().bytes() == authority.product_market_binding_id().bytes()
            && debit.foundation_schedule_id().bytes() == authority.foundation_schedule_v4_id().bytes()
            && debit.foundation_graph_id().bytes() == authority.foundation_account_graph_v4_id().bytes()
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

fn core_slot_postwrite_v4(
    program_id: &Pubkey,
    plan: &AuthenticatedGeneralMarketFoundingPlanV4,
    debit: AuthenticatedProductMarketFoundationDebitV4,
    account: &AccountInfo<'_>,
    slot: MarketFoundationSlotV4,
    account_data_id: ContentId,
) -> Outcome<AuthenticatedGeneralCoreFoundationPostwriteV4> {
    let slot_index = u8::try_from(slot.index().map_err(|_| ClutchError::MismatchedState)?)
        .map_err(|_| ClutchError::Arithmetic)?;
    let accepted_poststate_receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            b"dragons-clutch/sbf/general/product-funded-slot/v4\0",
            program_id.as_ref(), &plan.join_id.bytes(), &debit.id().bytes(), &[slot_index],
            account.key.as_ref(), &account_data_id.bytes(),
            &debit.principal_lamports().to_le_bytes(),
            &debit.destination_donation_floor_lamports().to_le_bytes(),
            &debit.destination_balance_after_lamports().to_le_bytes(),
            debit.rent_refund_owner().as_ref(), debit.neutral_lamport_sink().as_ref(),
        ]).to_bytes(),
    );
    require(accepted_poststate_receipt_id != ContentId::ZERO, ClutchError::MismatchedState)?;
    Ok(AuthenticatedGeneralCoreFoundationPostwriteV4 {
        debit_id: debit.id(),
        founder_creation_receipt_id: debit.founder_creation_receipt_id(),
        founder_preauthorization_id: debit.founder_preauthorization_id(),
        foundation_steps_id: debit.foundation_steps_id(),
        market_binding_id: debit.market_binding_id(),
        foundation_schedule_id: debit.foundation_schedule_id(),
        foundation_graph_id: debit.foundation_graph_id(),
        market_instance_id: debit.market_instance_id(), generation: debit.generation(), slot,
        account: *account.key, principal_lamports: debit.principal_lamports(),
        principal_before_lamports: debit.principal_before_lamports(),
        principal_after_lamports: debit.principal_after_lamports(),
        destination_donation_floor_lamports: debit.destination_donation_floor_lamports(),
        destination_balance_after_lamports: debit.destination_balance_after_lamports(),
        vault_donation_before_lamports: debit.vault_donation_before_lamports(),
        vault_donation_after_lamports: debit.vault_donation_after_lamports(),
        foundation_vault_account: debit.foundation_vault_account(),
        rent_refund_owner: debit.rent_refund_owner(),
        neutral_lamport_sink: debit.neutral_lamport_sink(), account_data_id,
        accepted_poststate_receipt_id,
    })
}

impl AuthenticatedProductMarketFoundationStepPostwriteV4
    for AuthenticatedGeneralCoreFoundationPostwriteV4
{
    #[allow(clippy::too_many_arguments)]
    fn consume_product_market_foundation_step_postwrite_v4(
        self, debit_id: ContentId, founder_creation_receipt_id: ContentId,
        founder_preauthorization_id: ContentId, foundation_steps_id: ContentId,
        market_binding_id: ContentId, foundation_schedule_id: ContentId,
        foundation_graph_id: ContentId,
        market_instance_id: clutch_product_series::MarketInstanceV2Id,
        generation: u64, slot: MarketFoundationSlotV4, account_id: ContentId,
        principal_lamports: u64, principal_before_lamports: u64,
        principal_after_lamports: u64, destination_donation_floor_lamports: u64,
        destination_balance_after_lamports: u64, vault_donation_before_lamports: u64,
        vault_donation_after_lamports: u64, foundation_vault_account: Pubkey,
        rent_refund_owner: Pubkey, neutral_lamport_sink: Pubkey,
    ) -> Outcome<(ContentId, u64)> {
        require(
            debit_id == self.debit_id
                && founder_creation_receipt_id == self.founder_creation_receipt_id
                && founder_preauthorization_id == self.founder_preauthorization_id
                && foundation_steps_id == self.foundation_steps_id
                && market_binding_id == self.market_binding_id
                && foundation_schedule_id == self.foundation_schedule_id
                && foundation_graph_id == self.foundation_graph_id
                && market_instance_id == self.market_instance_id && generation == self.generation
                && slot == self.slot && account_id.bytes() == self.account.to_bytes()
                && principal_lamports == self.principal_lamports
                && principal_before_lamports == self.principal_before_lamports
                && principal_after_lamports == self.principal_after_lamports
                && destination_donation_floor_lamports == self.destination_donation_floor_lamports
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

/// Product-funded ScheduleV4 slot 1 writer. No signer payer is admitted.
#[inline(never)]
pub(crate) fn write_product_funded_market_binding_v4(
    program_id: &Pubkey, plan: &AuthenticatedGeneralMarketFoundingPlanV4,
    debit: AuthenticatedProductMarketFoundationDebitV4, account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>, rent: &RentParameters,
) -> Outcome<AuthenticatedGeneralCoreFoundationPostwriteV4> {
    authenticate_core_debit_v4(plan, &debit, account, system_program,
        MarketFoundationSlotV4::MarketBinding, plan.market_binding_account,
        MARKET_BINDING_ACCOUNT_BYTES_V4, rent)?;
    let owner = DeletableRentOwnerV1::from_persisted(
        Id32::from_bytes(debit.rent_refund_owner().to_bytes()), debit.principal_lamports(),
        debit.destination_donation_floor_lamports())?;
    let body = MarketBindingV4::new(plan.base, plan.authority, owner)?;
    let market = plan.base.base().market_instance_v2_id.bytes();
    let bump = [plan.market_binding_bump];
    allocate_assign_product_funded_pda(program_id, account, system_program,
        MARKET_BINDING_ACCOUNT_BYTES_V4,
        &[seeds::SEED_GENERAL_V2_MARKET_BINDING, &market, &bump])?;
    {
        let mut data = account.try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        body.encode(&mut data)?;
        require(MarketBindingV4::decode(&data)? == body, ClutchError::MismatchedState)?;
    }
    let data = account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[
        b"dragons-clutch/sbf/general-market-binding/data/v4\0", account.key.as_ref(), &data,
    ]).to_bytes());
    drop(data);
    core_slot_postwrite_v4(program_id, plan, debit, account,
        MarketFoundationSlotV4::MarketBinding, data_id)
}

/// Product-funded ScheduleV4 slot 2 writer. No signer payer is admitted.
#[inline(never)]
pub(crate) fn write_product_funded_market_runtime_v3(
    program_id: &Pubkey, plan: &AuthenticatedGeneralMarketFoundingPlanV4,
    debit: AuthenticatedProductMarketFoundationDebitV4, account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>, rent: &RentParameters,
) -> Outcome<AuthenticatedGeneralCoreFoundationPostwriteV4> {
    authenticate_core_debit_v4(plan, &debit, account, system_program,
        MarketFoundationSlotV4::MarketRuntime, plan.market_runtime_account,
        MARKET_RUNTIME_ACCOUNT_BYTES, rent)?;
    let owner = DeletableRentOwnerV1::from_persisted(
        Id32::from_bytes(debit.rent_refund_owner().to_bytes()), debit.principal_lamports(),
        debit.destination_donation_floor_lamports())?;
    let body = MarketRuntimeV3AccountV1 {
        market_binding: Id32::from_bytes(plan.market_binding_account.to_bytes()),
        market_instance_v2_id: plan.base.base().market_instance_v2_id,
        next_epoch_index: 0, next_epoch_generation: 1,
        created_epoch_count: 0, retired_epoch_count: 0, rent: owner,
        stored_bump: plan.market_runtime_bump, flags: 0,
    };
    body.validate()?;
    let binding = plan.market_binding_account.to_bytes();
    let bump = [plan.market_runtime_bump];
    allocate_assign_product_funded_pda(program_id, account, system_program,
        MARKET_RUNTIME_ACCOUNT_BYTES,
        &[seeds::SEED_GENERAL_V2_MARKET_RUNTIME, &binding, &bump])?;
    {
        let mut data = account.try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        body.encode(&mut data)?;
        require(MarketRuntimeV3AccountV1::decode(&data)? == body, ClutchError::MismatchedState)?;
    }
    let data = account.try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let data_id = ContentId::from_bytes(solana_sha256_hasher::hashv(&[
        b"dragons-clutch/sbf/general-market-runtime/data/v3\0", account.key.as_ref(), &data,
    ]).to_bytes());
    drop(data);
    core_slot_postwrite_v4(program_id, plan, debit, account,
        MarketFoundationSlotV4::MarketRuntime, data_id)
}
