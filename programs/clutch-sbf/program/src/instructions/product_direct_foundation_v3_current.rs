//! Concrete Product owner for current Direct action 1.
//!
//! The prepared Product family successor is authorized only from hostile
//! RootV3/LinkV3 and General V5 state. After Product activates and allocates
//! the exact `0xba/v2` range, this module constructs the complete Direct b1/v3
//! binding and authenticates its physical rent/schedule inputs. No caller
//! semantic ID or historical General/Product projection is accepted.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::artifact::read_clock_slot;
use crate::instructions::genesis::read_rent;
use crate::seeds;
use clutch_direct_market_runtime::current_v3::{
    DirectCurrentGeneralAuthorityV3, DirectCurrentProductAuthorityV4,
    DirectMarketBindingV3,
};
use clutch_direct_market_runtime::fee_v2::DirectFeePolicyV2;
use clutch_direct_market_runtime::lifecycle_v2::
    AuthenticatedDirectFoundationV3 as RuntimeAuthenticatedDirectFoundationV3;
use clutch_direct_market_runtime::{
    direct_schedule_policy_id_v2, DirectMarketErrorV1, DirectRentOwnerV1,
    DirectScheduleV1,
};
use clutch_product_series::{
    ContentId, MarketFamilyV1, MarketInstanceV2Id, MarketLifecyclePhaseV3,
    SeriesMarketLinkPhaseV3,
};
use clutch_solana_layout::registry::{
    DIRECT_ACTION_REPLAY_ACCOUNT_BYTES, DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;
use std::boxed::Box;

use super::direct_market_v2::{
    create_direct_foundation_physical_v3, AuthenticatedProductDirectFoundationV3,
    DirectRuntimeSha256V2,
};
use super::general_market_current_v5::{
    authenticate_general_market_current_for_product_activation_v5,
    AuthenticatedGeneralMarketCurrentV5, GeneralMarketCurrentAccountFrameV5,
};
use super::product_direct_global_liveness::
    {activate_product_direct_global_liveness_for_family_v3,
    allocate_product_direct_candidate_v3, AuthenticatedProductDirectCandidateAllocationV3};
use super::product_market_activation_v3_current::activate_current_product_market_v3;
use super::product_market_family_admission_v3_current::{
    commit_product_family_admission_v3, prepare_product_family_admission_v3,
    AuthenticatedProductFamilyAdmissionOwnerV3, AuthenticatedProductFamilyAdmissionPlanV3,
};
use super::product_market_family_capability_current::
    {authenticate_current_market_family_capability_policy_v1,
    AuthenticatedMarketFamilyCapabilityPolicyV1};
use super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
    AuthenticatedMarketLifecycleRootV3, AuthenticatedSeriesMarketLinkV3,
};
use super::product_market_replay_current::{
    activate_current_product_market_replay_v3, authenticate_market_lifecycle_replay_v2,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};

/// Reconstructed Product preauthorization used only to prepare the Direct
/// family successor. Its owner ID is the immutable preauthorization already
/// persisted by General V5 and `0xba/v2`, never a new caller receipt.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductDirectFoundationPreauthorizationV3 {
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_transition_sequence: u64,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    family_policy_id: ContentId,
    family_policy_authentication_id: ContentId,
    direct_root_account: Pubkey,
    product_preauthorization_id: ContentId,
}

impl AuthenticatedProductFamilyAdmissionOwnerV3
    for AuthenticatedProductDirectFoundationPreauthorizationV3
{
    fn family(&self) -> Outcome<MarketFamilyV1> { Ok(MarketFamilyV1::Direct) }
    fn child_account(&self) -> Outcome<Pubkey> { Ok(self.direct_root_account) }
    fn owner_prewrite_id(&self) -> Outcome<ContentId> {
        Ok(self.product_preauthorization_id)
    }

    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_family_admission_owner_v3(
        &self,
        _program_id: &Pubkey,
        root_account: Pubkey,
        root_binding_id: ContentId,
        root_authentication_id: ContentId,
        root_semantic_id: ContentId,
        root_transition_sequence: u64,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        family_policy_id: ContentId,
        family_policy_authentication_id: ContentId,
        family: MarketFamilyV1,
        _family_namespace_anchor_id: ContentId,
        _family_admission_sequence: u32,
        child_account: Pubkey,
        owner_prewrite_id: ContentId,
    ) -> Outcome<()> {
        require(
            root_account == self.root_account
                && root_binding_id == self.root_binding_id
                && root_authentication_id == self.root_authentication_id
                && root_semantic_id == self.root_semantic_id
                && root_transition_sequence == self.root_transition_sequence
                && market_instance_id == self.market_instance_id
                && generation == self.generation
                && family_policy_id == self.family_policy_id
                && family_policy_authentication_id
                    == self.family_policy_authentication_id
                && family == MarketFamilyV1::Direct
                && child_account == self.direct_root_account
                && owner_prewrite_id == self.product_preauthorization_id,
            ClutchError::MismatchedState,
        )
    }
}

/// Reconstruct the sole Direct action-1 preauthorization from current state.
pub(crate) fn authenticate_product_direct_foundation_preauthorization_v3(
    program_id: &Pubkey,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    link: &AuthenticatedSeriesMarketLinkV3<'_>,
    family_policy: &AuthenticatedMarketFamilyCapabilityPolicyV1,
    general: &AuthenticatedGeneralMarketCurrentV5,
    direct_root_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedProductDirectFoundationPreauthorizationV3> {
    let root_binding = root.binding();
    let link_binding = link.binding();
    let current = general.binding().authority();
    let (expected_direct_root, _) = seeds::direct_market_root_v3_pda(
        program_id,
        &root_binding.market_instance_id.bytes(),
        root_binding.generation,
    );
    let product_preauthorization_id =
        ContentId::from_bytes(current.product_preauthorization_id().bytes());
    require(
        root.is_writable()
            && link.is_writable()
            && root.state().phase() == MarketLifecyclePhaseV3::Founding
            && link.state().phase() == SeriesMarketLinkPhaseV3::PendingMarket
            && root.state().foundation().complete()
            && root.state().capital().principal_remaining_lamports == 0
            && root.state().product_families().admits_new_child(MarketFamilyV1::Direct)
            && root.account() == general.product_root_account()
            && root.binding_id() == general.product_root_binding_id()
            && root.authentication_id() == general.product_root_authentication_id()
            && root.semantic_id() == general.product_root_semantic_id()
            && general.product_root_phase() == MarketLifecyclePhaseV3::Founding
            && link.account() == general.product_link_account()
            && link.binding_id() == general.product_link_binding_id()
            && link.authentication_id() == general.product_link_authentication_id()
            && link.semantic_id().content_id() == general.product_link_semantic_id()
            && general.product_link_phase() == SeriesMarketLinkPhaseV3::PendingMarket
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id == root.binding_id()
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && family_policy.aggregator() == *root.state().product_families()
            && *direct_root_account.key == expected_direct_root
            && direct_root_account.key != &root.account()
            && direct_root_account.key != &link.account()
            && !product_preauthorization_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedProductDirectFoundationPreauthorizationV3 {
        root_account: root.account(),
        root_binding_id: root.binding_id(),
        root_authentication_id: root.authentication_id(),
        root_semantic_id: root.semantic_id(),
        root_transition_sequence: root.state().transition_sequence(),
        market_instance_id: root_binding.market_instance_id,
        generation: root_binding.generation,
        family_policy_id: family_policy.policy_id(),
        family_policy_authentication_id: family_policy.id(),
        direct_root_account: *direct_root_account.key,
        product_preauthorization_id,
    })
}

/// Exact instruction-local Direct foundation owner. The complete binding is
/// borrowed by Direct's physical writer and then persisted in b1/v3.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductDirectFoundationOwnerV3 {
    preauthorization: AuthenticatedProductDirectFoundationPreauthorizationV3,
    binding: DirectMarketBindingV3,
    schedule: DirectScheduleV1,
    root_rent: DirectRentOwnerV1,
    replay_rent: DirectRentOwnerV1,
    observed_slot: u64,
}

impl AuthenticatedProductFamilyAdmissionOwnerV3
    for AuthenticatedProductDirectFoundationOwnerV3
{
    fn family(&self) -> Outcome<MarketFamilyV1> { Ok(MarketFamilyV1::Direct) }
    fn child_account(&self) -> Outcome<Pubkey> {
        Ok(self.preauthorization.direct_root_account)
    }
    fn owner_prewrite_id(&self) -> Outcome<ContentId> {
        Ok(self.preauthorization.product_preauthorization_id)
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_family_admission_owner_v3(
        &self,
        program_id: &Pubkey,
        root_account: Pubkey,
        root_binding_id: ContentId,
        root_authentication_id: ContentId,
        root_semantic_id: ContentId,
        root_transition_sequence: u64,
        market_instance_id: MarketInstanceV2Id,
        generation: u64,
        family_policy_id: ContentId,
        family_policy_authentication_id: ContentId,
        family: MarketFamilyV1,
        family_namespace_anchor_id: ContentId,
        family_admission_sequence: u32,
        child_account: Pubkey,
        owner_prewrite_id: ContentId,
    ) -> Outcome<()> {
        self.preauthorization
            .authenticate_product_family_admission_owner_v3(
                program_id,
                root_account,
                root_binding_id,
                root_authentication_id,
                root_semantic_id,
                root_transition_sequence,
                market_instance_id,
                generation,
                family_policy_id,
                family_policy_authentication_id,
                family,
                family_namespace_anchor_id,
                family_admission_sequence,
                child_account,
                owner_prewrite_id,
            )
    }
}

impl RuntimeAuthenticatedDirectFoundationV3
    for AuthenticatedProductDirectFoundationOwnerV3
{
    fn authenticate_foundation_v3(
        &self,
        binding: &DirectMarketBindingV3,
        schedule: DirectScheduleV1,
        root_rent: DirectRentOwnerV1,
        action_replay_rent: DirectRentOwnerV1,
        observed_slot: u64,
    ) -> Result<(), DirectMarketErrorV1> {
        if binding == &self.binding
            && schedule == self.schedule
            && root_rent == self.root_rent
            && action_replay_rent == self.replay_rent
            && observed_slot == self.observed_slot
        {
            Ok(())
        } else {
            Err(DirectMarketErrorV1::UnauthenticatedAuthority)
        }
    }
}

impl AuthenticatedProductDirectFoundationV3
    for AuthenticatedProductDirectFoundationOwnerV3
{
    fn direct_market_binding_v3(&self) -> Outcome<&DirectMarketBindingV3> {
        Ok(&self.binding)
    }
}

/// Bind the exact Product, General V5, Revenue, Candidate allocation, Clock,
/// and fresh Direct account prestates into Direct's sole physical owner.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn bind_product_direct_foundation_owner_v3(
    program_id: &Pubkey,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    link: &AuthenticatedSeriesMarketLinkV3<'_>,
    general: &AuthenticatedGeneralMarketCurrentV5,
    preauthorization: AuthenticatedProductDirectFoundationPreauthorizationV3,
    plan: &AuthenticatedProductFamilyAdmissionPlanV3,
    allocation: &AuthenticatedProductDirectCandidateAllocationV3,
    direct_accounts: &[AccountInfo<'_>],
) -> Outcome<AuthenticatedProductDirectFoundationOwnerV3> {
    require(direct_accounts.len() == 6, ClutchError::AccountCount)?;
    let relation = general.binding().base().base();
    let current = general.binding().authority();
    let root_binding = root.binding();
    let link_binding = link.binding();
    let candidate = allocation.candidate_binding();
    let revenue = general.revenue();
    let revenue_policy = revenue.policy();
    let treasury = general.treasury();
    let observed_slot = read_clock_slot(&direct_accounts[5])?;
    let schedule = DirectScheduleV1::canonical_from_foundation_slot(observed_slot)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rent = read_rent(&direct_accounts[4])?;
    let root_rent = DirectRentOwnerV1 {
        payer: direct_accounts[2].key.to_bytes(),
        principal_lamports: rent.minimum_balance(DIRECT_MARKET_ROOT_ACCOUNT_BYTES_V3)?,
        donation_floor_lamports: direct_accounts[0].lamports(),
    };
    let replay_rent = DirectRentOwnerV1 {
        payer: direct_accounts[2].key.to_bytes(),
        principal_lamports: rent.minimum_balance(DIRECT_ACTION_REPLAY_ACCOUNT_BYTES)?,
        donation_floor_lamports: direct_accounts[1].lamports(),
    };
    let fee = DirectFeePolicyV2 {
        batch_policy_id: general.binding().base().batch_policy_id().bytes(),
        revenue_policy_v2_digest: revenue.policy_digest().bytes(),
        revenue_policy_record_v2_id: revenue.record_semantic_id().bytes(),
        treasury_owner: revenue.treasury_owner().bytes(),
        treasury_position_derivation_policy_v2_id:
            revenue.treasury_position_derivation_policy_id().bytes(),
        dispersion_bps: revenue_policy.dispersion_bps,
        floor_range_bps: revenue_policy.floor_range_bps,
        maker_rebate_num: revenue_policy.maker_rebate_num,
        treasury_num: revenue_policy.treasury_num,
        split_den: revenue_policy.split_den,
    };
    fee.validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let product = DirectCurrentProductAuthorityV4 {
        product_root_account: root.account().to_bytes(),
        product_market_binding_v3_id: root.binding_id().bytes(),
        product_generation: root_binding.generation,
        product_family_prestate_id: plan.family_prestate_id().bytes(),
        product_family_poststate_id: plan.family_poststate_id().bytes(),
        product_family_admission_receipt_id: plan.family_admission_receipt_id().bytes(),
        family_admission_sequence: plan.family_admission_sequence(),
        series_link_account: link.account().to_bytes(),
        series_plan_v5_id: link_binding.series_plan_id.bytes(),
        series_link_binding_v3_id: link.binding_id().bytes(),
        series_ordinal: link_binding.ordinal,
        compiler_bundle_v7_id: link_binding.compiler_bundle_id.bytes(),
        funding_quote_v6_id: link_binding.funding_quote_id.bytes(),
        attachment_plan_v6_id: link_binding.attachment_plan_id.bytes(),
        foundation_schedule_v4_id: root_binding.foundation_schedule_id.bytes(),
        foundation_graph_v4_id: root_binding.foundation_account_graph_id.bytes(),
        market_liability_founding_id: root_binding.market_liability_founding_id.bytes(),
        claim_mint_founding_plan_id: root_binding.claim_mint_founding_plan_id.bytes(),
        claim_issuance_binding_id: root_binding.claim_issuance_binding_id.bytes(),
        general_founding_capability_v3_id:
            root_binding.general_founding_capability_id.bytes(),
        product_preauthorization_id: allocation.product_preauthorization_id().bytes(),
        product_direct_global_liveness_account: allocation.manifest_account().to_bytes(),
        product_direct_global_liveness_binding_id:
            root_binding.direct_global_liveness_binding_id.bytes(),
        product_direct_global_liveness_allocation_authentication_id:
            allocation.manifest_authentication_after_id().bytes(),
        activated_product_market_binding_id: root.binding_id().bytes(),
        direct_work_quote_id: allocation.direct_work_quote_id().bytes(),
    };
    let general_authority = DirectCurrentGeneralAuthorityV3 {
        general_market_binding_account: general.binding_account().to_bytes(),
        general_market_binding_v5_data_id: general.binding_data_id().bytes(),
        general_market_runtime_account: general.runtime_account().to_bytes(),
        general_market_runtime_data_id: general.runtime_data_id().bytes(),
        revenue_policy_record_account: revenue.record_account().to_bytes(),
        revenue_policy_record_v2_id: revenue.record_semantic_id().bytes(),
        revenue_policy_v2_digest: revenue.policy_digest().bytes(),
        treasury_owner: revenue.treasury_owner().bytes(),
        treasury_position_derivation_policy_v2_id:
            revenue.treasury_position_derivation_policy_id().bytes(),
        treasury_position_account: treasury.treasury_position_account().to_bytes(),
        treasury_replay_account: treasury.treasury_replay_account().to_bytes(),
        treasury_service_ledger_account:
            treasury.treasury_service_ledger_account().to_bytes(),
    };
    let direct_schedule_policy_id = direct_schedule_policy_id_v2(
        allocation.candidate_lifecycle_policy_id().bytes(),
        allocation.candidate_liveness_policy_id().bytes(),
        candidate,
        &DirectRuntimeSha256V2,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut binding = DirectMarketBindingV3 {
        market_instance_id: root_binding.market_instance_id.bytes(),
        generation: root_binding.generation,
        outcome_count: root_binding.outcome_count,
        realm_id: root_binding.realm_id.bytes(),
        collateral_profile_id: root_binding.collateral_profile_id.bytes(),
        collateral_policy_id: root_binding.collateral_policy_id.bytes(),
        collateral_release_id: root_binding.collateral_release_id.bytes(),
        resolution_account: root_binding.resolution_account_id.bytes(),
        direct_epoch_semantics_id: [0; 32],
        revenue_policy_id: fee.revenue_policy_v2_digest,
        batch_policy_id: fee.batch_policy_id,
        direct_fee_shape_id: fee
            .semantic_id(&DirectRuntimeSha256V2)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        fee_treasury_owner: fee.treasury_owner,
        fee_dispersion_bps: fee.dispersion_bps,
        fee_floor_range_bps: fee.floor_range_bps,
        fee_maker_rebate_num: fee.maker_rebate_num,
        fee_treasury_num: fee.treasury_num,
        fee_split_den: fee.split_den,
        candidate_lifecycle_policy_id: allocation.candidate_lifecycle_policy_id().bytes(),
        candidate_liveness_policy_id: allocation.candidate_liveness_policy_id().bytes(),
        candidate_liveness: candidate,
        direct_schedule_policy_id,
        product,
        general: general_authority,
        direct_root_account: allocation.direct_root_account().to_bytes(),
        action_replay_account: allocation.direct_action_replay_account().to_bytes(),
        neutral_lamport_sink: allocation.neutral_lamport_sink().to_bytes(),
        relation_policy_id: relation.relation_policy_id.bytes(),
        price_policy_id: relation.price_measure_policy_v1_id.bytes(),
        price_scale: relation.price_scale,
    };
    binding.direct_epoch_semantics_id = binding
        .expected_direct_epoch_semantics_id(schedule, &DirectRuntimeSha256V2)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    binding
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        preauthorization.root_account == root.account()
            && preauthorization.root_binding_id == root.binding_id()
            && preauthorization.root_authentication_id == root.authentication_id()
            && preauthorization.root_semantic_id == root.semantic_id()
            && preauthorization.root_transition_sequence
                == root.state().transition_sequence()
            && preauthorization.market_instance_id == root_binding.market_instance_id
            && preauthorization.generation == root_binding.generation
            && preauthorization.direct_root_account == allocation.direct_root_account()
            && preauthorization.product_preauthorization_id
                == allocation.product_preauthorization_id()
            && plan.root_account() == root.account()
            && plan.root_binding_id() == root.binding_id()
            && plan.root_semantic_before_id() == root.semantic_id()
            && plan.child_account() == allocation.direct_root_account()
            && plan.owner_prewrite_id() == allocation.product_preauthorization_id()
            && allocation.product_root_semantic_after_family_id()
                == plan.root_semantic_after_id()
            && allocation.market_instance_id() == root_binding.market_instance_id
            && allocation.generation() == root_binding.generation
            && allocation.realm_id() == root_binding.realm_id
            && allocation.neutral_lamport_sink().to_bytes()
                == root.state().capital().neutral_lamport_sink.bytes()
            && allocation.neutral_lamport_sink().to_bytes()
                == link_binding.neutral_lamport_sink.bytes()
            && general.product_root_binding_id() == root.binding_id()
            && general.product_link_binding_id() == link.binding_id()
            && current.product_preauthorization_id().bytes()
                == allocation.product_preauthorization_id().bytes()
            && current.series_physical_founder_v5_id().bytes() != [0; 32]
            && relation.market_instance_v2_id.bytes()
                == root_binding.market_instance_id.bytes()
            && relation.outcome_count == root_binding.outcome_count
            && direct_accounts[0].key == &allocation.direct_root_account()
            && direct_accounts[1].key == &allocation.direct_action_replay_account(),
        ClutchError::MismatchedState,
    )?;
    Ok(AuthenticatedProductDirectFoundationOwnerV3 {
        preauthorization,
        binding,
        schedule,
        root_rent,
        replay_rent,
        observed_slot,
    })
}

/// Exact current action-1 physical frame beyond General's 25-account graph.
/// ProductReplay and policy are followed by writable `0xba`, seven read-only
/// liveness rows, and Direct's exact six-account b1/b3 creation suffix.
pub(crate) struct ProductDirectInitializeMarketAccountFrameV3<'frame, 'info> {
    pub(crate) general: GeneralMarketCurrentAccountFrameV5<'frame, 'info>,
    pub(crate) product_replay: &'frame AccountInfo<'info>,
    pub(crate) family_policy: &'frame AccountInfo<'info>,
    pub(crate) liveness_manifest: &'frame AccountInfo<'info>,
    pub(crate) liveness_compartments: &'frame [AccountInfo<'info>],
    pub(crate) direct: &'frame [AccountInfo<'info>],
}

/// Sole current Product/Direct action-1 outer. Every instruction-local token
/// is consumed before return; durable authority is only the hostile-reopened
/// b1/v3, RootV3, LinkV3, `0xba/v2`, and ProductReplay poststate tuple.
#[inline(never)]
pub(crate) fn compose_product_direct_initialize_market_v3(
    program_id: &Pubkey,
    frame: &ProductDirectInitializeMarketAccountFrameV3<'_, '_>,
    sequence: u64,
    payload: &[u8],
) -> Outcome<()> {
    require(
        sequence == 0
            && payload.is_empty()
            && frame.liveness_compartments.len() == 7
            && frame.direct.len() == 6
            && frame.general.product_root.is_writable
            && frame.general.series_link.is_writable
            && frame.product_replay.is_writable
            && frame.liveness_manifest.is_writable,
        ClutchError::MismatchedState,
    )?;
    let mut general_root_output = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut general_link_output = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let general = authenticate_general_market_current_for_product_activation_v5(
        program_id,
        &frame.general,
        &mut general_root_output,
        &mut general_link_output,
    )?;
    let current = general.binding().authority();
    let relation = general.binding().base().base();
    let market_instance_id = MarketInstanceV2Id::from_bytes(
        relation.market_instance_v2_id.bytes(),
    );
    let series_plan_id = clutch_product_series::SeriesPlanV5Id::from_bytes(
        relation.series_plan_v5_id.bytes(),
    );
    let mut root_before_output = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let root_before = authenticate_market_lifecycle_root_v3(
        program_id,
        frame.general.product_root,
        market_instance_id,
        current.product_generation(),
        true,
        &mut root_before_output,
    )?;
    let mut link_before_output = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let link_before = authenticate_series_market_link_v3(
        program_id,
        frame.general.series_link,
        series_plan_id,
        current.series_ordinal(),
        market_instance_id,
        current.product_generation(),
        *frame.general.product_root.key,
        true,
        &mut link_before_output,
    )?;
    let replay_before = authenticate_market_lifecycle_replay_v2(
        program_id,
        frame.product_replay,
        market_instance_id,
        true,
    )?;
    let family_policy = authenticate_current_market_family_capability_policy_v1(
        program_id,
        &root_before,
        &replay_before,
        frame.family_policy,
    )?;
    let preauthorization = authenticate_product_direct_foundation_preauthorization_v3(
        program_id,
        &root_before,
        &link_before,
        &family_policy,
        &general,
        &frame.direct[0],
    )?;
    let mut family_successor = *root_before.state();
    let family_plan = prepare_product_family_admission_v3(
        program_id,
        &root_before,
        &replay_before,
        &family_policy,
        &preauthorization,
        &mut family_successor,
    )?;
    let liveness_activation = activate_product_direct_global_liveness_for_family_v3(
        program_id,
        frame.liveness_manifest,
        &family_plan,
    )?;
    let allocation = allocate_product_direct_candidate_v3(
        program_id,
        frame.liveness_manifest,
        frame.liveness_compartments,
        &family_plan,
        liveness_activation,
    )?;
    let owner = bind_product_direct_foundation_owner_v3(
        program_id,
        &root_before,
        &link_before,
        &general,
        preauthorization,
        &family_plan,
        &allocation,
        frame.direct,
    )?;
    let direct_postwrite = create_direct_foundation_physical_v3(
        program_id,
        &owner,
        &family_plan,
        frame.direct,
        sequence,
        payload,
    )?;
    drop(owner);
    drop(root_before);
    let mut commit_root_before_output =
        Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut root_after_family_output =
        Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let (root_after_family, family_admission) = commit_product_family_admission_v3(
        program_id,
        frame.general.product_root,
        family_plan,
        direct_postwrite,
        &mut commit_root_before_output,
        &mut family_successor,
        &mut root_after_family_output,
    )?;
    let schedule = general.foundation_schedule();
    let mut root_admission_state = *root_after_family.state();
    let mut link_activation_state = *link_before.state();
    let mut root_activation_state = *root_after_family.state();
    let mut root_admission_output = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let mut link_activation_output = Box::new(SeriesMarketLinkAccountV3::decode_buffer());
    let mut root_activation_output = Box::new(MarketLifecycleRootAccountV3::decode_buffer());
    let market_activation = activate_current_product_market_v3(
        program_id,
        frame.general.product_root,
        frame.general.series_link,
        root_after_family,
        link_before,
        family_admission,
        allocation,
        &schedule,
        &mut root_admission_state,
        &mut root_admission_output,
        &mut link_activation_state,
        &mut link_activation_output,
        &mut root_activation_state,
        &mut root_activation_output,
    )?;
    let final_activation = activate_current_product_market_replay_v3(
        program_id,
        frame.product_replay,
        replay_before,
        market_activation,
    )?;
    require(!final_activation.id().is_zero(), ClutchError::MismatchedState)
}
