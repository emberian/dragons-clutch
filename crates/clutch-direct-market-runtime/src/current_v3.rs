//! Current Direct root authority over General V4 and Product V3 state.
//!
//! `DirectMarketRootV1` is a historical BundleV5/General-V3 owner.  This
//! module deliberately does not persist or expose that DTO. The V3 root
//! stores every current Product, General, Revenue, and global-liveness
//! coordinate under a fresh domain. Until the transition arithmetic is
//! mechanically generalized, a crate-private total projection may invoke the
//! already-reviewed V1 arithmetic. The projection commits the complete V3
//! Product and General authorities, and every poststate is reconstructed into
//! V3 and revalidated before it can leave this module.

use crate::fee_v2::DirectFeePolicyV2;
use crate::liveness_v1::DirectCandidateLivenessBindingV1;
use crate::{
    DirectActionReplayV1, DirectHashBackendV1, DirectMarketBindingV1,
    DirectMarketErrorV1, DirectMarketRootV1, DirectRentOwnerV1, DirectRootPhaseV1,
    DirectScheduleV1, DirectTerminalReasonV1, MAX_DIRECT_RESERVATIONS_V1,
};

const PRODUCT_AUTHORITY_DOMAIN_V3: &[u8] =
    b"dragons-clutch/direct/current-product-authority/v3\0";
const GENERAL_AUTHORITY_DOMAIN_V2: &[u8] =
    b"dragons-clutch/direct/current-general-authority/v2\0";
pub(crate) const BINDING_DOMAIN_V3: &[u8] = b"dragons-clutch/direct/market-binding/v3\0";
pub(crate) const ROOT_STATE_DOMAIN_V3: &[u8] = b"dragons-clutch/direct/root-state/v3\0";
const DIRECT_EPOCH_SEMANTICS_DOMAIN_V3: &[u8] =
    b"dragons-clutch/direct/epoch-semantics/v3\0";

/// Exact current Product authority retained by Direct b1/v3.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCurrentProductAuthorityV3 {
    pub product_root_account: [u8; 32],
    /// Exact immutable `MarketLifecycleBindingV3::id()` authenticated from
    /// Product RootV3. Direct does not duplicate that large binding.
    pub product_market_binding_v3_id: [u8; 32],
    pub product_generation: u64,
    pub product_family_prestate_id: [u8; 32],
    pub product_family_poststate_id: [u8; 32],
    pub product_family_admission_receipt_id: [u8; 32],
    pub family_admission_sequence: u32,
    pub series_link_account: [u8; 32],
    /// Exact Product-owned SeriesPlanV5 identity used to authenticate the
    /// canonical `0xad/v3` PDA without trusting caller material.
    pub series_plan_v5_id: [u8; 32],
    /// Exact immutable `SeriesMarketLinkBindingV3::id()`. This commits the
    /// entire SeriesPlanV5/ordinal/Market/Source/FundingV5 tuple without
    /// persisting a detachable copy of those Product-owned facts or a mutable
    /// LinkV3 state identity which would become stale as obligations close.
    pub series_link_binding_v3_id: [u8; 32],
    pub series_ordinal: u32,
    pub compiler_bundle_v7_id: [u8; 32],
    pub funding_quote_v6_id: [u8; 32],
    pub attachment_plan_v6_id: [u8; 32],
    pub foundation_schedule_v4_id: [u8; 32],
    pub foundation_graph_v4_id: [u8; 32],
    pub market_liability_founding_id: [u8; 32],
    pub claim_mint_founding_plan_id: [u8; 32],
    pub claim_issuance_binding_id: [u8; 32],
    pub general_founding_capability_v3_id: [u8; 32],
    pub product_preauthorization_id: [u8; 32],
    pub product_direct_global_liveness_account: [u8; 32],
    pub product_direct_global_liveness_binding_id: [u8; 32],
    pub product_direct_global_liveness_activation_id: [u8; 32],
    pub activated_product_market_binding_id: [u8; 32],
    pub direct_work_quote_id: [u8; 32],
}

impl DirectCurrentProductAuthorityV3 {
    /// Refuse zero identities, generation zero, and cross-role account aliases.
    pub fn validate(&self) -> Result<(), DirectMarketErrorV1> {
        if self.product_generation == 0 {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        for id in self.ids() {
            require_live_v2(id)?;
        }
        require_distinct_v2(&[
            self.product_root_account,
            self.series_link_account,
            self.product_direct_global_liveness_account,
        ])
    }

    /// Complete domain-separated identity of the current Product authority.
    pub fn semantic_id<B: DirectHashBackendV1>(
        &self,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        let id = backend.sha256_parts(&[
            PRODUCT_AUTHORITY_DOMAIN_V3,
            &self.product_root_account,
            &self.product_market_binding_v3_id,
            &self.product_generation.to_le_bytes(),
            &self.product_family_prestate_id,
            &self.product_family_poststate_id,
            &self.product_family_admission_receipt_id,
            &self.family_admission_sequence.to_le_bytes(),
            &self.series_link_account,
            &self.series_plan_v5_id,
            &self.series_link_binding_v3_id,
            &self.series_ordinal.to_le_bytes(),
            &self.compiler_bundle_v7_id,
            &self.funding_quote_v6_id,
            &self.attachment_plan_v6_id,
            &self.foundation_schedule_v4_id,
            &self.foundation_graph_v4_id,
            &self.market_liability_founding_id,
            &self.claim_mint_founding_plan_id,
            &self.claim_issuance_binding_id,
            &self.general_founding_capability_v3_id,
            &self.product_preauthorization_id,
            &self.product_direct_global_liveness_account,
            &self.product_direct_global_liveness_binding_id,
            &self.product_direct_global_liveness_activation_id,
            &self.activated_product_market_binding_id,
            &self.direct_work_quote_id,
        ]);
        require_live_v2(id)?;
        Ok(id)
    }

    pub(crate) fn ids(&self) -> [[u8; 32]; 23] {
        [
            self.product_root_account,
            self.product_market_binding_v3_id,
            self.product_family_prestate_id,
            self.product_family_poststate_id,
            self.product_family_admission_receipt_id,
            self.series_link_account,
            self.series_plan_v5_id,
            self.series_link_binding_v3_id,
            self.compiler_bundle_v7_id,
            self.funding_quote_v6_id,
            self.attachment_plan_v6_id,
            self.foundation_schedule_v4_id,
            self.foundation_graph_v4_id,
            self.market_liability_founding_id,
            self.claim_mint_founding_plan_id,
            self.claim_issuance_binding_id,
            self.general_founding_capability_v3_id,
            self.product_preauthorization_id,
            self.product_direct_global_liveness_account,
            self.product_direct_global_liveness_binding_id,
            self.product_direct_global_liveness_activation_id,
            self.activated_product_market_binding_id,
            self.direct_work_quote_id,
        ]
    }
}

/// Exact current General and Revenue authority retained by Direct b1/v3.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCurrentGeneralAuthorityV2 {
    pub general_market_binding_account: [u8; 32],
    pub general_market_binding_v4_data_id: [u8; 32],
    pub general_market_runtime_account: [u8; 32],
    pub general_market_runtime_data_id: [u8; 32],
    pub revenue_policy_record_account: [u8; 32],
    pub revenue_policy_record_v2_id: [u8; 32],
    pub revenue_policy_v2_digest: [u8; 32],
    pub treasury_owner: [u8; 32],
    pub treasury_position_derivation_policy_v2_id: [u8; 32],
    pub treasury_position_account: [u8; 32],
    pub treasury_replay_account: [u8; 32],
    pub treasury_service_ledger_account: [u8; 32],
}

impl DirectCurrentGeneralAuthorityV2 {
    /// Refuse zero identities and physical-account aliases.
    pub fn validate(&self) -> Result<(), DirectMarketErrorV1> {
        for id in self.ids() {
            require_live_v2(id)?;
        }
        require_distinct_v2(&[
            self.general_market_binding_account,
            self.general_market_runtime_account,
            self.revenue_policy_record_account,
            self.treasury_owner,
            self.treasury_position_account,
            self.treasury_replay_account,
            self.treasury_service_ledger_account,
        ])
    }

    /// Complete domain-separated identity of the current General authority.
    pub fn semantic_id<B: DirectHashBackendV1>(
        &self,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        let ids = self.ids();
        let id = backend.sha256_parts(&[
            GENERAL_AUTHORITY_DOMAIN_V2,
            &ids[0], &ids[1], &ids[2], &ids[3], &ids[4], &ids[5],
            &ids[6], &ids[7], &ids[8], &ids[9], &ids[10], &ids[11],
        ]);
        require_live_v2(id)?;
        Ok(id)
    }

    pub(crate) const fn ids(&self) -> [[u8; 32]; 12] {
        [
            self.general_market_binding_account,
            self.general_market_binding_v4_data_id,
            self.general_market_runtime_account,
            self.general_market_runtime_data_id,
            self.revenue_policy_record_account,
            self.revenue_policy_record_v2_id,
            self.revenue_policy_v2_digest,
            self.treasury_owner,
            self.treasury_position_derivation_policy_v2_id,
            self.treasury_position_account,
            self.treasury_replay_account,
            self.treasury_service_ledger_account,
        ]
    }
}

/// Fresh current Direct binding.  No V1/V3 Product field is reinterpreted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectMarketBindingV3 {
    pub market_instance_id: [u8; 32],
    pub generation: u64,
    pub outcome_count: u8,
    pub realm_id: [u8; 32],
    pub collateral_profile_id: [u8; 32],
    pub collateral_policy_id: [u8; 32],
    pub collateral_release_id: [u8; 32],
    pub resolution_account: [u8; 32],
    pub direct_epoch_semantics_id: [u8; 32],
    pub revenue_policy_id: [u8; 32],
    pub batch_policy_id: [u8; 32],
    pub direct_fee_shape_id: [u8; 32],
    pub fee_treasury_owner: [u8; 32],
    pub fee_dispersion_bps: u32,
    pub fee_floor_range_bps: u32,
    pub fee_maker_rebate_num: u32,
    pub fee_treasury_num: u32,
    pub fee_split_den: u32,
    pub candidate_lifecycle_policy_id: [u8; 32],
    pub candidate_liveness_policy_id: [u8; 32],
    pub candidate_liveness: DirectCandidateLivenessBindingV1,
    pub direct_schedule_policy_id: [u8; 32],
    pub product: DirectCurrentProductAuthorityV3,
    pub general: DirectCurrentGeneralAuthorityV2,
    pub direct_root_account: [u8; 32],
    pub action_replay_account: [u8; 32],
    pub neutral_lamport_sink: [u8; 32],
    pub relation_policy_id: [u8; 32],
    pub price_policy_id: [u8; 32],
    pub price_scale: u64,
}

impl DirectMarketBindingV3 {
    /// Validate the complete current cross-plane authority and fee/liveness facts.
    pub fn validate(&self) -> Result<(), DirectMarketErrorV1> {
        if self.generation == 0
            || self.generation != self.product.product_generation
            || !(2..=16).contains(&usize::from(self.outcome_count))
            || self.price_scale == 0
        {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        for id in [
            self.market_instance_id,
            self.realm_id,
            self.collateral_profile_id,
            self.collateral_policy_id,
            self.collateral_release_id,
            self.resolution_account,
            self.direct_epoch_semantics_id,
            self.revenue_policy_id,
            self.batch_policy_id,
            self.direct_fee_shape_id,
            self.candidate_lifecycle_policy_id,
            self.candidate_liveness_policy_id,
            self.direct_schedule_policy_id,
            self.direct_root_account,
            self.action_replay_account,
            self.neutral_lamport_sink,
            self.relation_policy_id,
            self.price_policy_id,
        ] {
            require_live_v2(id)?;
        }
        self.product.validate()?;
        self.general.validate()?;
        self.fee_policy().validate()?;
        self.candidate_liveness.validate()?;
        if self.fee_treasury_owner != self.general.treasury_owner
            || self.revenue_policy_id != self.general.revenue_policy_v2_digest
            || self.product.product_market_binding_v3_id
                != self.product.activated_product_market_binding_id
            || self.candidate_liveness.policy_account
                == self.product.product_direct_global_liveness_account
            || self.candidate_liveness.work_schedule_id != self.product.direct_work_quote_id
        {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        require_distinct_v2(&[
            self.resolution_account,
            self.product.product_root_account,
            self.product.series_link_account,
            self.product.product_direct_global_liveness_account,
            self.general.general_market_binding_account,
            self.general.general_market_runtime_account,
            self.general.revenue_policy_record_account,
            self.general.treasury_position_account,
            self.general.treasury_replay_account,
            self.general.treasury_service_ledger_account,
            self.direct_root_account,
            self.action_replay_account,
            self.neutral_lamport_sink,
            self.candidate_liveness.policy_account,
            self.candidate_liveness.candidate_account,
        ])
    }

    /// Exact copied fee arithmetic facts, authenticated from RevenuePolicyV2.
    pub const fn fee_policy(&self) -> DirectFeePolicyV2 {
        DirectFeePolicyV2 {
            batch_policy_id: self.batch_policy_id,
            revenue_policy_v2_digest: self.revenue_policy_id,
            revenue_policy_record_v2_id: self.general.revenue_policy_record_v2_id,
            treasury_owner: self.fee_treasury_owner,
            treasury_position_derivation_policy_v2_id:
                self.general.treasury_position_derivation_policy_v2_id,
            dispersion_bps: self.fee_dispersion_bps,
            floor_range_bps: self.fee_floor_range_bps,
            maker_rebate_num: self.fee_maker_rebate_num,
            treasury_num: self.fee_treasury_num,
            split_den: self.fee_split_den,
        }
    }

    /// One-based Direct occurrence coordinate.
    pub fn direct_window_index(&self) -> Result<u64, DirectMarketErrorV1> {
        u64::from(self.product.family_admission_sequence)
            .checked_add(1)
            .ok_or(DirectMarketErrorV1::Arithmetic)
    }

    /// Complete V3 binding identity.
    pub fn semantic_id<B: DirectHashBackendV1>(
        &self,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        let product_id = self.product.semantic_id(backend)?;
        let general_id = self.general.semantic_id(backend)?;
        let fee_policy_id = self.fee_policy().semantic_id(backend)?;
        if product_id == general_id {
            return Err(DirectMarketErrorV1::IdentityAlias);
        }
        if fee_policy_id != self.direct_fee_shape_id {
            return Err(DirectMarketErrorV1::MismatchedBinding);
        }
        let candidate_id = candidate_liveness_id_v2(self.candidate_liveness, backend)?;
        let id = backend.sha256_parts(&[
            BINDING_DOMAIN_V3,
            &self.market_instance_id,
            &self.generation.to_le_bytes(),
            &[self.outcome_count],
            &self.realm_id,
            &self.collateral_profile_id,
            &self.collateral_policy_id,
            &self.collateral_release_id,
            &self.resolution_account,
            &self.direct_epoch_semantics_id,
            &self.revenue_policy_id,
            &self.batch_policy_id,
            &self.direct_fee_shape_id,
            &self.fee_treasury_owner,
            &self.fee_dispersion_bps.to_le_bytes(),
            &self.fee_floor_range_bps.to_le_bytes(),
            &self.fee_maker_rebate_num.to_le_bytes(),
            &self.fee_treasury_num.to_le_bytes(),
            &self.fee_split_den.to_le_bytes(),
            &self.candidate_lifecycle_policy_id,
            &self.candidate_liveness_policy_id,
            &candidate_id,
            &self.direct_schedule_policy_id,
            &product_id,
            &general_id,
            &self.direct_root_account,
            &self.action_replay_account,
            &self.neutral_lamport_sink,
            &self.relation_policy_id,
            &self.price_policy_id,
            &self.price_scale.to_le_bytes(),
        ]);
        require_live_v2(id)?;
        Ok(id)
    }

    /// Derive the only current Direct epoch identity from current authorities.
    pub fn expected_direct_epoch_semantics_id<B: DirectHashBackendV1>(
        &self,
        schedule: DirectScheduleV1,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        schedule.validate()?;
        let product_id = self.product.semantic_id(backend)?;
        let general_id = self.general.semantic_id(backend)?;
        let id = backend.sha256_parts(&[
            DIRECT_EPOCH_SEMANTICS_DOMAIN_V3,
            &self.market_instance_id,
            &self.generation.to_le_bytes(),
            &self.direct_root_account,
            &self.direct_schedule_policy_id,
            &product_id,
            &general_id,
            &self.candidate_liveness.allocation_receipt_id,
            &self.product.direct_work_quote_id,
            &schedule.admission_opens_slot.to_le_bytes(),
            &schedule.admission_closes_slot.to_le_bytes(),
            &schedule.submission_closes_slot.to_le_bytes(),
            &schedule.selection_deadline_slot.to_le_bytes(),
            &schedule.settlement_deadline_slot.to_le_bytes(),
        ]);
        require_live_v2(id)?;
        Ok(id)
    }

    /// Private, injective-at-the-protocol-ID-boundary transition projection.
    ///
    /// This value is never encoded and never leaves the crate.  The two fields
    /// whose V1 names refer to withdrawn Product artifacts instead carry the
    /// complete domain-separated Product and General authority identities.
    pub(crate) fn transition_projection<B: DirectHashBackendV1>(
        &self,
        backend: &B,
    ) -> Result<DirectMarketBindingV1, DirectMarketErrorV1> {
        self.validate()?;
        let product_id = self.product.semantic_id(backend)?;
        let general_id = self.general.semantic_id(backend)?;
        let value = DirectMarketBindingV1 {
            market_instance_id: self.market_instance_id,
            generation: self.generation,
            outcome_count: self.outcome_count,
            realm_id: self.realm_id,
            collateral_profile_id: self.collateral_profile_id,
            collateral_policy_id: self.collateral_policy_id,
            collateral_release_id: self.collateral_release_id,
            resolution_account: self.resolution_account,
            direct_epoch_semantics_id: self.direct_epoch_semantics_id,
            revenue_policy_id: self.revenue_policy_id,
            batch_policy_id: self.batch_policy_id,
            direct_fee_shape_id: self.direct_fee_shape_id,
            fee_treasury_owner: self.fee_treasury_owner,
            fee_dispersion_bps: self.fee_dispersion_bps,
            fee_floor_range_bps: self.fee_floor_range_bps,
            fee_maker_rebate_num: self.fee_maker_rebate_num,
            fee_treasury_num: self.fee_treasury_num,
            fee_split_den: self.fee_split_den,
            candidate_lifecycle_policy_id: self.candidate_lifecycle_policy_id,
            candidate_liveness_policy_id: self.candidate_liveness_policy_id,
            candidate_liveness: self.candidate_liveness,
            direct_schedule_policy_id: self.direct_schedule_policy_id,
            product_root_account: self.product.product_root_account,
            product_market_binding_id: self.product.product_market_binding_v3_id,
            product_family_prestate_id: self.product.product_family_prestate_id,
            general_product_preauthorization_id: self.product.product_preauthorization_id,
            family_admission_sequence: self.product.family_admission_sequence,
            founder_series_link_account: self.product.series_link_account,
            founder_series_link_binding_id: self.product.series_link_binding_v3_id,
            compiler_bundle_v5_id: product_id,
            founder_series_plan_id: general_id,
            founder_series_ordinal: self.product.series_ordinal,
            direct_root_account: self.direct_root_account,
            action_replay_account: self.action_replay_account,
            general_market_binding: self.general.general_market_binding_account,
            general_market_runtime: self.general.general_market_runtime_account,
            neutral_lamport_sink: self.neutral_lamport_sink,
            relation_policy_id: self.relation_policy_id,
            price_policy_id: self.price_policy_id,
            price_scale: self.price_scale,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Current b1/v3 root. Dynamic lifecycle geometry is unchanged and remains
/// owned by the reviewed Direct arithmetic, while the authority is V3-only.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectMarketRootV3 {
    pub(crate) binding: DirectMarketBindingV3,
    pub(crate) schedule: DirectScheduleV1,
    pub(crate) root_rent: DirectRentOwnerV1,
    pub(crate) phase: DirectRootPhaseV1,
    pub(crate) terminal_reason: Option<DirectTerminalReasonV1>,
    pub(crate) admitted_reservations: u8,
    pub(crate) live_reservations: u8,
    pub(crate) retired_reservations: u8,
    pub(crate) reservation_accounts: [[u8; 32]; 2],
    pub(crate) reservation_semantic_ids: [[u8; 32]; 2],
    pub(crate) selection_account: [u8; 32],
}

impl DirectMarketRootV3 {
    /// Construct the only fresh open current root.
    pub fn new_open(
        binding: DirectMarketBindingV3,
        schedule: DirectScheduleV1,
        root_rent: DirectRentOwnerV1,
    ) -> Result<Self, DirectMarketErrorV1> {
        let value = Self {
            binding,
            schedule,
            root_rent,
            phase: DirectRootPhaseV1::Open,
            terminal_reason: None,
            admitted_reservations: 0,
            live_reservations: 0,
            retired_reservations: 0,
            reservation_accounts: [[0; 32]; 2],
            reservation_semantic_ids: [[0; 32]; 2],
            selection_account: [0; 32],
        };
        value.validate()?;
        Ok(value)
    }

    pub const fn binding(&self) -> &DirectMarketBindingV3 { &self.binding }
    pub const fn schedule(&self) -> DirectScheduleV1 { self.schedule }
    pub const fn root_rent(&self) -> DirectRentOwnerV1 { self.root_rent }
    pub const fn phase(&self) -> DirectRootPhaseV1 { self.phase }
    pub const fn terminal_reason(&self) -> Option<DirectTerminalReasonV1> {
        self.terminal_reason
    }
    pub const fn admitted_reservations(&self) -> u8 { self.admitted_reservations }
    pub const fn live_reservations(&self) -> u8 { self.live_reservations }
    pub const fn retired_reservations(&self) -> u8 { self.retired_reservations }
    pub const fn selection_account(&self) -> [u8; 32] { self.selection_account }

    pub fn reservation_account(&self, index: u8) -> Result<[u8; 32], DirectMarketErrorV1> {
        let at = usize::from(index);
        if at >= usize::from(self.live_reservations) {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        Ok(self.reservation_accounts[at])
    }

    pub fn reservation_semantic_id(
        &self,
        index: u8,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        let at = usize::from(index);
        if at >= usize::from(self.live_reservations) {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        Ok(self.reservation_semantic_ids[at])
    }

    /// Validate the exhaustive root partition under current authority.
    pub fn validate(&self) -> Result<(), DirectMarketErrorV1> {
        self.binding.validate()?;
        self.schedule.validate()?;
        self.root_rent.validate()?;
        if self.admitted_reservations > MAX_DIRECT_RESERVATIONS_V1
            || self.live_reservations > self.admitted_reservations
            || self.retired_reservations > self.admitted_reservations
            || self.live_reservations.checked_add(self.retired_reservations)
                != Some(self.admitted_reservations)
        {
            return Err(DirectMarketErrorV1::InvalidCount);
        }
        let mut index = 0usize;
        while index < 2 {
            if index < usize::from(self.live_reservations) {
                require_fresh_child_account_v2(&self.binding, self.reservation_accounts[index])?;
                require_live_v2(self.reservation_semantic_ids[index])?;
                if index != 0 && self.reservation_accounts[index - 1] == self.reservation_accounts[index] {
                    return Err(DirectMarketErrorV1::IdentityAlias);
                }
            } else if self.reservation_accounts[index] != [0; 32]
                || self.reservation_semantic_ids[index] != [0; 32]
            {
                return Err(DirectMarketErrorV1::InvalidCount);
            }
            index += 1;
        }
        match self.phase {
            DirectRootPhaseV1::Open => {
                if self.selection_account != [0; 32] || self.terminal_reason.is_some() {
                    return Err(DirectMarketErrorV1::WrongPhase);
                }
            }
            DirectRootPhaseV1::FrozenEmpty
            | DirectRootPhaseV1::SubmissionOpen
            | DirectRootPhaseV1::Verifying
            | DirectRootPhaseV1::Selected => {
                require_live_v2(self.selection_account)?;
                if self.terminal_reason.is_some() {
                    return Err(DirectMarketErrorV1::WrongPhase);
                }
            }
            DirectRootPhaseV1::Terminal => {
                require_live_v2(self.selection_account)?;
                if self.terminal_reason.is_none() {
                    return Err(DirectMarketErrorV1::WrongPhase);
                }
            }
        }
        Ok(())
    }

    /// Fresh domain-separated identity of the complete b1/v3 state.
    pub fn semantic_id<B: DirectHashBackendV1>(
        &self,
        backend: &B,
    ) -> Result<[u8; 32], DirectMarketErrorV1> {
        self.validate()?;
        let binding_id = self.binding.semantic_id(backend)?;
        let terminal = self.terminal_reason.map_or(0, DirectTerminalReasonV1::byte);
        let id = backend.sha256_parts(&[
            ROOT_STATE_DOMAIN_V3,
            &binding_id,
            &self.schedule.admission_opens_slot.to_le_bytes(),
            &self.schedule.admission_closes_slot.to_le_bytes(),
            &self.schedule.submission_closes_slot.to_le_bytes(),
            &self.schedule.selection_deadline_slot.to_le_bytes(),
            &self.schedule.settlement_deadline_slot.to_le_bytes(),
            &self.root_rent.payer,
            &self.root_rent.principal_lamports.to_le_bytes(),
            &self.root_rent.donation_floor_lamports.to_le_bytes(),
            &[self.phase.byte()],
            &[terminal],
            &[self.admitted_reservations],
            &[self.live_reservations],
            &[self.retired_reservations],
            &self.reservation_accounts[0],
            &self.reservation_accounts[1],
            &self.reservation_semantic_ids[0],
            &self.reservation_semantic_ids[1],
            &self.selection_account,
        ]);
        require_live_v2(id)?;
        Ok(id)
    }

    /// Private historical-arithmetic input; never encoded or returned publicly.
    pub(crate) fn transition_projection<B: DirectHashBackendV1>(
        &self,
        backend: &B,
    ) -> Result<DirectMarketRootV1, DirectMarketErrorV1> {
        self.validate()?;
        let value = DirectMarketRootV1 {
            binding: self.binding.transition_projection(backend)?,
            schedule: self.schedule,
            root_rent: self.root_rent,
            phase: self.phase,
            terminal_reason: self.terminal_reason,
            admitted_reservations: self.admitted_reservations,
            live_reservations: self.live_reservations,
            retired_reservations: self.retired_reservations,
            reservation_accounts: self.reservation_accounts,
            reservation_semantic_ids: self.reservation_semantic_ids,
            selection_account: self.selection_account,
        };
        value.validate()?;
        Ok(value)
    }

    /// Reconstruct and check one V1-arithmetic successor under the same V3 authority.
    pub(crate) fn accept_transition_projection<B: DirectHashBackendV1>(
        &self,
        before: DirectMarketRootV1,
        after: DirectMarketRootV1,
        backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        if before != self.transition_projection(backend)?
            || after.binding != before.binding
            || after.schedule != before.schedule
            || after.root_rent != before.root_rent
        {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        let value = Self {
            binding: self.binding.clone(),
            schedule: self.schedule,
            root_rent: self.root_rent,
            phase: after.phase,
            terminal_reason: after.terminal_reason,
            admitted_reservations: after.admitted_reservations,
            live_reservations: after.live_reservations,
            retired_reservations: after.retired_reservations,
            reservation_accounts: after.reservation_accounts,
            reservation_semantic_ids: after.reservation_semantic_ids,
            selection_account: after.selection_account,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Derive the exact semantic identity of the fresh action-1 root without
/// materializing the 2.5KiB current root by value. The caller must stream the
/// same binding, schedule, and rent into the canonical V3 codec.
pub fn direct_foundation_root_semantic_id_v3<B: DirectHashBackendV1>(
    binding: &DirectMarketBindingV3,
    schedule: DirectScheduleV1,
    root_rent: DirectRentOwnerV1,
    backend: &B,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    binding.validate()?;
    schedule.validate()?;
    root_rent.validate()?;
    let binding_id = binding.semantic_id(backend)?;
    let id = backend.sha256_parts(&[
        ROOT_STATE_DOMAIN_V3,
        &binding_id,
        &schedule.admission_opens_slot.to_le_bytes(),
        &schedule.admission_closes_slot.to_le_bytes(),
        &schedule.submission_closes_slot.to_le_bytes(),
        &schedule.selection_deadline_slot.to_le_bytes(),
        &schedule.settlement_deadline_slot.to_le_bytes(),
        &root_rent.payer,
        &root_rent.principal_lamports.to_le_bytes(),
        &root_rent.donation_floor_lamports.to_le_bytes(),
        &[DirectRootPhaseV1::Open.byte()],
        &[0],
        &[0],
        &[0],
        &[0],
        &[0; 32],
        &[0; 32],
        &[0; 32],
        &[0; 32],
        &[0; 32],
    ]);
    require_live_v2(id)?;
    Ok(id)
}

/// Current root plus unchanged permanent b3/v1 replay owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectRootReplayPostV2 {
    pub root: DirectMarketRootV3,
    pub replay: DirectActionReplayV1,
}

impl DirectRootReplayPostV2 {
    /// Reauthenticate b3 against the exact private arithmetic projection.
    pub fn validate<B: DirectHashBackendV1>(&self, backend: &B) -> Result<(), DirectMarketErrorV1> {
        self.root.validate()?;
        self.replay.validate_against(self.root.transition_projection(backend)?)
    }

    /// Private input to the reviewed lifecycle arithmetic. No projected V1
    /// root is returned across the crate boundary.
    pub(crate) fn transition_projection<B: DirectHashBackendV1>(
        &self,
        backend: &B,
    ) -> Result<crate::DirectRootReplayPostV1, DirectMarketErrorV1> {
        self.validate(backend)?;
        Ok(crate::DirectRootReplayPostV1 {
            root: self.root.transition_projection(backend)?,
            replay: self.replay,
        })
    }

    /// Reconstruct one current successor and prove that the private V1 call
    /// changed only the lifecycle fields owned by that arithmetic.
    pub(crate) fn accept_transition_projection<B: DirectHashBackendV1>(
        self,
        before: crate::DirectRootReplayPostV1,
        after: crate::DirectRootReplayPostV1,
        backend: &B,
    ) -> Result<Self, DirectMarketErrorV1> {
        if before != self.transition_projection(backend)? {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        let root = self.root.accept_transition_projection(
            before.root,
            after.root,
            backend,
        )?;
        after.replay.validate_against(root.transition_projection(backend)?)?;
        Ok(Self {
            root,
            replay: after.replay,
        })
    }

    /// Apply one reviewed arithmetic successor directly into caller-owned
    /// current storage. This avoids materializing a second 2.5KiB V3 root in
    /// an SBF frame while preserving the same injective pre/post checks.
    pub(crate) fn accept_transition_projection_in_place<B: DirectHashBackendV1>(
        &mut self,
        before: crate::DirectRootReplayPostV1,
        after: crate::DirectRootReplayPostV1,
        backend: &B,
    ) -> Result<(), DirectMarketErrorV1> {
        if before != self.transition_projection(backend)?
            || after.root.binding != before.root.binding
            || after.root.schedule != before.root.schedule
            || after.root.root_rent != before.root.root_rent
        {
            return Err(DirectMarketErrorV1::UnauthenticatedAuthority);
        }
        self.root.phase = after.root.phase;
        self.root.terminal_reason = after.root.terminal_reason;
        self.root.admitted_reservations = after.root.admitted_reservations;
        self.root.live_reservations = after.root.live_reservations;
        self.root.retired_reservations = after.root.retired_reservations;
        self.root.reservation_accounts = after.root.reservation_accounts;
        self.root.reservation_semantic_ids = after.root.reservation_semantic_ids;
        self.root.selection_account = after.root.selection_account;
        after
            .replay
            .validate_against(self.root.transition_projection(backend)?)?;
        self.replay = after.replay;
        self.validate(backend)
    }
}

pub(crate) fn candidate_liveness_id_v2<B: DirectHashBackendV1>(
    value: DirectCandidateLivenessBindingV1,
    backend: &B,
) -> Result<[u8; 32], DirectMarketErrorV1> {
    value.validate()?;
    let id = backend.sha256_parts(&[
        b"dragons-clutch/direct/candidate-liveness-binding/v2\0",
        &value.policy_account,
        &value.policy_data_id,
        &value.global_lifecycle_id,
        &value.global_bundle_binding_id,
        &value.global_capitalization_receipt_id,
        &value.global_bundle_commitment_id,
        &value.candidate_account,
        &value.candidate_data_id,
        &value.candidate_semantic_owner,
        &value.candidate_quote_schedule_id,
        &value.candidate_receipt_program_id,
        &value.candidate_generation.to_le_bytes(),
        &value.first_call_ordinal.to_le_bytes(),
        &value.reserved_calls.to_le_bytes(),
        &value.reserved_work_lamports.to_le_bytes(),
        &value.allocation_receipt_id,
        &value.work_schedule.freeze_book_lamports.to_le_bytes(),
        &value.work_schedule.begin_verification_lamports.to_le_bytes(),
        &value.work_schedule.verify_candidate_lamports.to_le_bytes(),
        &value.work_schedule.finalize_selection_lamports.to_le_bytes(),
        &value.work_schedule.economic_terminal_lamports.to_le_bytes(),
        &value.work_schedule.retire_terminal_lamports.to_le_bytes(),
        &value.work_schedule.retained_candidate_bond_lamports.to_le_bytes(),
        &value.work_schedule_id,
    ]);
    require_live_v2(id)?;
    Ok(id)
}

fn require_fresh_child_account_v2(
    binding: &DirectMarketBindingV3,
    child: [u8; 32],
) -> Result<(), DirectMarketErrorV1> {
    require_live_v2(child)?;
    for account in [
        binding.resolution_account,
        binding.product.product_root_account,
        binding.product.series_link_account,
        binding.product.product_direct_global_liveness_account,
        binding.general.general_market_binding_account,
        binding.general.general_market_runtime_account,
        binding.general.treasury_position_account,
        binding.general.treasury_replay_account,
        binding.general.treasury_service_ledger_account,
        binding.direct_root_account,
        binding.action_replay_account,
        binding.neutral_lamport_sink,
    ] {
        if child == account {
            return Err(DirectMarketErrorV1::IdentityAlias);
        }
    }
    Ok(())
}

fn require_live_v2(value: [u8; 32]) -> Result<(), DirectMarketErrorV1> {
    if value == [0; 32] {
        Err(DirectMarketErrorV1::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn require_distinct_v2(values: &[[u8; 32]]) -> Result<(), DirectMarketErrorV1> {
    let mut left = 0usize;
    while left < values.len() {
        require_live_v2(values[left])?;
        let mut right = left + 1;
        while right < values.len() {
            if values[left] == values[right] {
                return Err(DirectMarketErrorV1::IdentityAlias);
            }
            right += 1;
        }
        left += 1;
    }
    Ok(())
}
