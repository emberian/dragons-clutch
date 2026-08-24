//! Atomic current Product RootV3/LinkV3 activation after Direct foundation.
//!
//! Direct first writes and hostile-reopens its b1/v3+b3 child against the
//! prepared Product family plan. Product commits that family successor, then
//! this module admits and activates the founder LinkV3 and activates RootV3.
//! The exact Product `0xba/v2` Candidate allocation is consumed by value, so a
//! Direct child cannot make the shared Market live without the prepaid work
//! range that its persisted binding authenticates.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_product_series::{
    ContentId, MarketFamilyV1, MarketFoundationScheduleV4, MarketInstanceV2Id,
    MarketLifecyclePhaseV3, MarketLifecycleRootV3, SeriesMarketAdmissionProjectionV3,
    SeriesMarketLinkPhaseV3, SeriesMarketLinkV3,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::product_direct_global_liveness::
    AuthenticatedProductDirectCandidateAllocationV3;
use super::product_market_family_admission_v3_current::
    AuthenticatedProductFamilyAdmissionV3;
use super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
    AuthenticatedMarketLifecycleRootV3, AuthenticatedSeriesMarketLinkV3,
};

const PRODUCT_CURRENT_MARKET_CORE_ACCEPTANCE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-current-market-core-acceptance/v3\0";
const PRODUCT_CURRENT_MARKET_ACTIVATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-current-market-activation/v3\0";

/// Final move-only activation lineage. The embedded Direct family and 0xba
/// receipts cannot be reused by another RootV3 or Direct child.
#[derive(Debug)]
pub(crate) struct AuthenticatedCurrentProductMarketActivationV3<'root, 'link> {
    id: ContentId,
    accepted_market_core_receipt_id: ContentId,
    series_admission_receipt_id: ContentId,
    family_admission: AuthenticatedProductFamilyAdmissionV3,
    direct_allocation: AuthenticatedProductDirectCandidateAllocationV3,
    root_authentication_before_id: ContentId,
    root_admission_authentication_id: ContentId,
    link_authentication_before_id: ContentId,
    root_after: AuthenticatedMarketLifecycleRootV3<'root>,
    link_after: AuthenticatedSeriesMarketLinkV3<'link>,
}

impl<'root, 'link> AuthenticatedCurrentProductMarketActivationV3<'root, 'link> {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn accepted_market_core_receipt_id(&self) -> ContentId {
        self.accepted_market_core_receipt_id
    }
    pub(crate) const fn series_admission_receipt_id(&self) -> ContentId {
        self.series_admission_receipt_id
    }
    pub(crate) const fn root(&self) -> &AuthenticatedMarketLifecycleRootV3<'root> {
        &self.root_after
    }
    pub(crate) const fn link(&self) -> &AuthenticatedSeriesMarketLinkV3<'link> {
        &self.link_after
    }
    pub(crate) const fn direct_allocation(
        &self,
    ) -> &AuthenticatedProductDirectCandidateAllocationV3 {
        &self.direct_allocation
    }
}

/// Consume the committed Direct family postwrite and its exact `0xba`
/// allocation, then persist Root-admission -> Link-activation -> Root-active
/// in that order with a hostile reopen after every write.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn activate_current_product_market_v3<'root, 'link>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    root_after_family: AuthenticatedMarketLifecycleRootV3<'_>,
    link_before: AuthenticatedSeriesMarketLinkV3<'_>,
    family_admission: AuthenticatedProductFamilyAdmissionV3,
    direct_allocation: AuthenticatedProductDirectCandidateAllocationV3,
    schedule: &MarketFoundationScheduleV4,
    root_admission_state: &mut MarketLifecycleRootV3,
    root_admission_output: &mut MarketLifecycleRootAccountV3,
    link_activation_state: &mut SeriesMarketLinkV3,
    link_activation_output: &'link mut SeriesMarketLinkAccountV3,
    root_activation_state: &mut MarketLifecycleRootV3,
    root_activation_output: &'root mut MarketLifecycleRootAccountV3,
) -> Outcome<AuthenticatedCurrentProductMarketActivationV3<'root, 'link>> {
    let binding = root_after_family.binding();
    let link_binding = link_before.binding();
    let schedule_id = schedule
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let candidate = direct_allocation.candidate_binding();
    require(
        root_after_family.is_writable()
            && link_before.is_writable()
            && root_after_family.account() == *root_account.key
            && link_before.account() == *link_account.key
            && root_after_family.owner_program() == *program_id
            && link_before.owner_program() == *program_id
            && root_after_family.state().phase() == MarketLifecyclePhaseV3::Founding
            && link_before.state().phase() == SeriesMarketLinkPhaseV3::PendingMarket
            && root_after_family.state().foundation().complete()
            && root_after_family.state().capital().principal_remaining_lamports == 0
            && binding.foundation_schedule_id == schedule_id
            && family_admission.family() == MarketFamilyV1::Direct
            && family_admission.root_account() == root_after_family.account()
            && family_admission.root_binding_id() == root_after_family.binding_id()
            && family_admission.root_authentication_after_id()
                == root_after_family.authentication_id()
            && family_admission.root_semantic_after_id() == root_after_family.semantic_id()
            && family_admission.root_transition_sequence_after()
                == root_after_family.state().transition_sequence()
            && family_admission.child_account() == direct_allocation.direct_root_account()
            && direct_allocation.product_root_account() == root_after_family.account()
            && direct_allocation.product_root_binding_id() == root_after_family.binding_id()
            && direct_allocation.product_root_semantic_after_family_id()
                == root_after_family.semantic_id()
            && direct_allocation.market_instance_id() == binding.market_instance_id
            && direct_allocation.generation() == binding.generation
            && ContentId::from_bytes(candidate.global_bundle_binding_id)
                == binding.direct_global_liveness_binding_id
            && ContentId::from_bytes(candidate.candidate_semantic_owner)
                == ContentId::from_bytes(direct_allocation.manifest_account().to_bytes())
            && ContentId::from_bytes(candidate.allocation_receipt_id)
                == direct_allocation.allocation_receipt_id()
            && link_binding.market_root_account_id.bytes() == root_account.key.to_bytes()
            && link_binding.market_binding_id == root_after_family.binding_id()
            && link_binding.market_instance_id == binding.market_instance_id
            && link_binding.generation == binding.generation
            && link_binding.capability_profile_id == binding.capability_profile_id,
        ClutchError::MismatchedState,
    )?;

    let admission_sequence = u64::from(root_after_family.state().admitted_series_links())
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    let admission = SeriesMarketAdmissionProjectionV3::new_from_ref(
        binding,
        link_before.state(),
        admission_sequence,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    root_after_family
        .state()
        .admit_series_link_into(admission, root_admission_state)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    link_before
        .state()
        .activate_into(admission_sequence, admission.id(), link_activation_state)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let accepted_market_core_receipt_id = hashv(&[
        PRODUCT_CURRENT_MARKET_CORE_ACCEPTANCE_DOMAIN_V3,
        program_id.as_ref(),
        root_account.key.as_ref(),
        link_account.key.as_ref(),
        &root_after_family.binding_id().bytes(),
        &root_after_family.authentication_id().bytes(),
        &root_after_family.semantic_id().bytes(),
        &root_after_family.state().foundation().transcript_id.bytes(),
        &schedule_id.bytes(),
        &family_admission.id().bytes(),
        &direct_allocation.id().bytes(),
        &admission.id().bytes(),
    ]);
    require_live(accepted_market_core_receipt_id)?;
    root_admission_state
        .activate_into(
            schedule,
            accepted_market_core_receipt_id,
            root_activation_state,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;

    let root_authentication_before_id = root_after_family.authentication_id();
    let root_data_before_id = root_after_family.data_id();
    let root_observed_lamports = root_after_family.observed_lamports();
    let root_rent_principal_lamports = root_after_family.value().rent_principal_lamports;
    let root_stored_bump = root_after_family.value().stored_bump;
    write_root(
        root_account,
        root_admission_state,
        root_rent_principal_lamports,
        root_stored_bump,
    )?;
    let root_admitted = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        binding.market_instance_id,
        binding.generation,
        true,
        root_admission_output,
    )?;
    require(
        root_admitted.state() == root_admission_state
            && root_admitted.observed_lamports() == root_observed_lamports
            && root_admitted.authentication_id() != root_authentication_before_id
            && root_admitted.data_id() != root_data_before_id,
        ClutchError::MismatchedState,
    )?;

    let link_authentication_before_id = link_before.authentication_id();
    let link_data_before_id = link_before.data_id();
    let link_observed_lamports = link_before.observed_lamports();
    let link_stored_bump = link_before.value().stored_bump;
    write_link(link_account, link_activation_state, link_stored_bump)?;
    let link_after = authenticate_series_market_link_v3(
        program_id,
        link_account,
        link_binding.series_plan_id,
        link_binding.ordinal,
        link_binding.market_instance_id,
        link_binding.generation,
        root_after_family.account(),
        true,
        link_activation_output,
    )?;
    require(
        link_after.state() == link_activation_state
            && link_after.observed_lamports() == link_observed_lamports
            && link_after.authentication_id() != link_authentication_before_id
            && link_after.data_id() != link_data_before_id
            && link_after.state().market_admission_receipt_id() == admission.id(),
        ClutchError::MismatchedState,
    )?;

    let root_admission_authentication_id = root_admitted.authentication_id();
    let root_admission_data_id = root_admitted.data_id();
    write_root(
        root_account,
        root_activation_state,
        root_rent_principal_lamports,
        root_stored_bump,
    )?;
    let root_after = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        binding.market_instance_id,
        binding.generation,
        true,
        root_activation_output,
    )?;
    require(
        root_after.state() == root_activation_state
            && root_after.state().phase() == MarketLifecyclePhaseV3::Active
            && root_after.observed_lamports() == root_observed_lamports
            && root_after.authentication_id() != root_admission_authentication_id
            && root_after.data_id() != root_admission_data_id,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_CURRENT_MARKET_ACTIVATION_DOMAIN_V3,
        program_id.as_ref(),
        &family_admission.id().bytes(),
        &direct_allocation.id().bytes(),
        &accepted_market_core_receipt_id.bytes(),
        &admission.id().bytes(),
        root_account.key.as_ref(),
        &root_authentication_before_id.bytes(),
        &root_admission_authentication_id.bytes(),
        &root_after.authentication_id().bytes(),
        link_account.key.as_ref(),
        &link_authentication_before_id.bytes(),
        &link_after.authentication_id().bytes(),
        &root_after.state().transition_sequence().to_le_bytes(),
        &link_after.state().transition_sequence().to_le_bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedCurrentProductMarketActivationV3 {
        id,
        accepted_market_core_receipt_id,
        series_admission_receipt_id: admission.id(),
        family_admission,
        direct_allocation,
        root_authentication_before_id,
        root_admission_authentication_id,
        link_authentication_before_id,
        root_after,
        link_after,
    })
}

fn write_root(
    account: &AccountInfo<'_>,
    state: &MarketLifecycleRootV3,
    rent_principal_lamports: u64,
    stored_bump: u8,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV3::encode_parts(
        state,
        rent_principal_lamports,
        stored_bump,
        &mut data,
    )
}

fn write_link(
    account: &AccountInfo<'_>,
    state: &SeriesMarketLinkV3,
    stored_bump: u8,
) -> Outcome<()> {
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV3::encode_parts(state, stored_bump, &mut data)
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}
