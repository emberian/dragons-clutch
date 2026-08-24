//! Two-phase current Product obligation admission into LinkV3.
//!
//! A family-specific prewrite fixes the concrete child account and owner
//! receipt. Product predicts the exact LinkV3 successor, the family performs
//! and hostile-authenticates its physical creation, and Product persists the
//! LinkV3 successor last. This composes beside the RootV3 family admission
//! plan without either authority depending on the other's postwrite.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_product_series::{
    ContentId, MarketInstanceV2Id, MarketLifecyclePhaseV3,
    SeriesLinkObligationAdmissionProjectionV3, SeriesLinkObligationStatusV3,
    SeriesLinkObligationV3, SeriesMarketLinkPhaseV3, SeriesMarketLinkV3,
    SeriesMarketLinkV3Id, SeriesPlanV5Id,
};
use clutch_solana_layout::product_series::SeriesMarketLinkAccountV3;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::product_market_family_capability_current::
    AuthenticatedMarketFamilyCapabilityPolicyV1;
use super::product_market_lifecycle_v3_current::{
    authenticate_series_market_link_v3, AuthenticatedMarketLifecycleRootV3,
    AuthenticatedSeriesMarketLinkV3,
};

const PRODUCT_LINK_OBLIGATION_ADMISSION_PREAUTHORIZATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-link-obligation-admission-preauthorization/v3\0";
const PRODUCT_LINK_OBLIGATION_OWNER_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-link-obligation-owner-receipt/v3\0";
const PRODUCT_LINK_OBLIGATION_ADMISSION_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-link-obligation-admission-postwrite/v3\0";

/// Default-refusing physical obligation prewrite.
pub(crate) trait AuthenticatedProductLinkObligationAdmissionOwnerV3 {
    fn obligation(&self) -> Outcome<SeriesLinkObligationV3> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn child_account(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn owner_prewrite_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_link_obligation_admission_owner_v3(
        &self,
        _program_id: &Pubkey,
        _root_account: Pubkey,
        _root_binding_id: ContentId,
        _root_authentication_id: ContentId,
        _root_semantic_id: ContentId,
        _root_transition_sequence: u64,
        _link_account: Pubkey,
        _link_binding_id: ContentId,
        _link_authentication_id: ContentId,
        _link_semantic_id: SeriesMarketLinkV3Id,
        _link_transition_sequence_before: u64,
        _link_transition_sequence_after: u64,
        _series_plan_id: SeriesPlanV5Id,
        _ordinal: u32,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _obligation_configuration_id: ContentId,
        _obligation: SeriesLinkObligationV3,
        _child_account: Pubkey,
        _owner_prewrite_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Default-refusing move-only physical child postwrite.
pub(crate) trait AuthenticatedProductLinkObligationAdmissionPostwriteV3: Sized {
    #[allow(clippy::too_many_arguments)]
    fn consume_product_link_obligation_admission_postwrite_v3(
        self,
        _plan_id: ContentId,
        _root_account: Pubkey,
        _root_binding_id: ContentId,
        _root_transition_sequence: u64,
        _link_account: Pubkey,
        _link_binding_id: ContentId,
        _link_semantic_before_id: SeriesMarketLinkV3Id,
        _link_semantic_after_id: SeriesMarketLinkV3Id,
        _link_transition_sequence_before: u64,
        _link_transition_sequence_after: u64,
        _obligation: SeriesLinkObligationV3,
        _owner_admission_receipt_id: ContentId,
        _product_admission_projection_id: ContentId,
        _child_account: Pubkey,
        _owner_prewrite_id: ContentId,
    ) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Instruction-local exact LinkV3 admission preauthorization.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductLinkObligationAdmissionPlanV3 {
    id: ContentId,
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_authentication_id: ContentId,
    root_semantic_id: ContentId,
    root_transition_sequence: u64,
    link_account: Pubkey,
    link_binding_id: ContentId,
    link_authentication_id: ContentId,
    link_data_id: ContentId,
    link_semantic_before_id: SeriesMarketLinkV3Id,
    link_semantic_after_id: SeriesMarketLinkV3Id,
    link_transition_sequence_before: u64,
    link_transition_sequence_after: u64,
    series_plan_id: SeriesPlanV5Id,
    ordinal: u32,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    obligation_configuration_id: ContentId,
    obligation: SeriesLinkObligationV3,
    owner_admission_receipt_id: ContentId,
    product_admission_projection_id: ContentId,
    child_account: Pubkey,
    owner_prewrite_id: ContentId,
}

impl AuthenticatedProductLinkObligationAdmissionPlanV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.root_binding_id }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_binding_id(&self) -> ContentId { self.link_binding_id }
    pub(crate) const fn link_semantic_before_id(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_before_id
    }
    pub(crate) const fn link_semantic_after_id(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_after_id
    }
    pub(crate) const fn link_transition_sequence_before(&self) -> u64 {
        self.link_transition_sequence_before
    }
    pub(crate) const fn link_transition_sequence_after(&self) -> u64 {
        self.link_transition_sequence_after
    }
    pub(crate) const fn obligation(&self) -> SeriesLinkObligationV3 { self.obligation }
    pub(crate) const fn owner_admission_receipt_id(&self) -> ContentId {
        self.owner_admission_receipt_id
    }
    pub(crate) const fn product_admission_projection_id(&self) -> ContentId {
        self.product_admission_projection_id
    }
    pub(crate) const fn child_account(&self) -> Pubkey { self.child_account }
}

/// Move-only LinkV3 postwrite returned to the family-specific outer.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductLinkObligationAdmissionV3 {
    id: ContentId,
    physical_postwrite_id: ContentId,
    link_account: Pubkey,
    link_binding_id: ContentId,
    link_authentication_before_id: ContentId,
    link_authentication_after_id: ContentId,
    link_semantic_before_id: SeriesMarketLinkV3Id,
    link_semantic_after_id: SeriesMarketLinkV3Id,
    link_transition_sequence_before: u64,
    link_transition_sequence_after: u64,
    obligation: SeriesLinkObligationV3,
    owner_admission_receipt_id: ContentId,
    product_admission_projection_id: ContentId,
    child_account: Pubkey,
}

impl AuthenticatedProductLinkObligationAdmissionV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn physical_postwrite_id(&self) -> ContentId {
        self.physical_postwrite_id
    }
    pub(crate) const fn link_account(&self) -> Pubkey { self.link_account }
    pub(crate) const fn link_binding_id(&self) -> ContentId { self.link_binding_id }
    pub(crate) const fn link_authentication_after_id(&self) -> ContentId {
        self.link_authentication_after_id
    }
    pub(crate) const fn link_semantic_after_id(&self) -> SeriesMarketLinkV3Id {
        self.link_semantic_after_id
    }
    pub(crate) const fn link_transition_sequence_after(&self) -> u64 {
        self.link_transition_sequence_after
    }
    pub(crate) const fn obligation(&self) -> SeriesLinkObligationV3 { self.obligation }
    pub(crate) const fn product_admission_projection_id(&self) -> ContentId {
        self.product_admission_projection_id
    }
}

/// Predict one exact LinkV3 obligation successor without writing it.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn prepare_product_link_obligation_admission_v3<A>(
    program_id: &Pubkey,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    link: &AuthenticatedSeriesMarketLinkV3<'_>,
    family_policy: &AuthenticatedMarketFamilyCapabilityPolicyV1,
    owner: &A,
    successor_output: &mut SeriesMarketLinkV3,
) -> Outcome<AuthenticatedProductLinkObligationAdmissionPlanV3>
where
    A: AuthenticatedProductLinkObligationAdmissionOwnerV3 + ?Sized,
{
    let root_binding = root.binding();
    let link_binding = link.binding();
    let obligation_configuration = family_policy
        .obligation_configuration(link_binding.attachment_plan_id.content_id())?;
    let obligation_configuration_id = obligation_configuration
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let obligation = owner.obligation()?;
    let child_account = owner.child_account()?;
    let owner_prewrite_id = owner.owner_prewrite_id()?;
    require_live(owner_prewrite_id)?;
    let link_semantic_before_id = link.semantic_id();
    let link_transition_sequence_before = link.state().transition_sequence();
    let link_transition_sequence_after = link_transition_sequence_before
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        root.state().phase() == MarketLifecyclePhaseV3::Active
            && link.is_writable()
            && link.state().phase() == SeriesMarketLinkPhaseV3::Active
            && link.state().obligation_status(obligation)
                == SeriesLinkObligationStatusV3::EnabledNeverFounded
            && link_binding.market_root_account_id.bytes() == root.account().to_bytes()
            && link_binding.market_binding_id == root.binding_id()
            && link_binding.market_instance_id == root_binding.market_instance_id
            && link_binding.generation == root_binding.generation
            && link_binding.capability_profile_id == root_binding.capability_profile_id
            && link_binding.capability_profile_id == family_policy
                .aggregator()
                .binding()
                .capability_profile_id
                .content_id()
            && link_binding.obligation_configuration_id == obligation_configuration_id
            && link_binding.attachment_plan_id.content_id()
                == obligation_configuration.attachment_plan_id
            && child_account != root.account()
            && child_account != link.account(),
        ClutchError::MismatchedState,
    )?;
    owner.authenticate_product_link_obligation_admission_owner_v3(
        program_id,
        root.account(),
        root.binding_id(),
        root.authentication_id(),
        root.semantic_id(),
        root.state().transition_sequence(),
        link.account(),
        link.binding_id(),
        link.authentication_id(),
        link_semantic_before_id,
        link_transition_sequence_before,
        link_transition_sequence_after,
        link_binding.series_plan_id,
        link_binding.ordinal,
        link_binding.market_instance_id,
        link_binding.generation,
        obligation_configuration_id.content_id(),
        obligation,
        child_account,
        owner_prewrite_id,
    )?;
    let owner_admission_receipt_id = hashv(&[
        PRODUCT_LINK_OBLIGATION_OWNER_RECEIPT_DOMAIN_V3,
        program_id.as_ref(),
        root.account().as_ref(),
        &root.binding_id().bytes(),
        &root.state().transition_sequence().to_le_bytes(),
        link.account().as_ref(),
        &link.binding_id().bytes(),
        &link.authentication_id().bytes(),
        &link_semantic_before_id.bytes(),
        &link_transition_sequence_before.to_le_bytes(),
        &link_transition_sequence_after.to_le_bytes(),
        &[obligation.byte()],
        child_account.as_ref(),
        &owner_prewrite_id.bytes(),
    ]);
    require_live(owner_admission_receipt_id)?;
    let projection = SeriesLinkObligationAdmissionProjectionV3 {
        link_semantic_id: link_semantic_before_id,
        obligation,
        link_transition_sequence: link_transition_sequence_after,
        owner_admission_receipt_id,
    };
    let product_admission_projection_id = projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    *successor_output = link
        .state()
        .admit_obligation(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let link_semantic_after_id = successor_output
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let id = hashv(&[
        PRODUCT_LINK_OBLIGATION_ADMISSION_PREAUTHORIZATION_DOMAIN_V3,
        &owner_admission_receipt_id.bytes(),
        &product_admission_projection_id.bytes(),
        &link_semantic_before_id.bytes(),
        &link_semantic_after_id.bytes(),
        child_account.as_ref(),
        &owner_prewrite_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductLinkObligationAdmissionPlanV3 {
        id,
        root_account: root.account(),
        root_binding_id: root.binding_id(),
        root_authentication_id: root.authentication_id(),
        root_semantic_id: root.semantic_id(),
        root_transition_sequence: root.state().transition_sequence(),
        link_account: link.account(),
        link_binding_id: link.binding_id(),
        link_authentication_id: link.authentication_id(),
        link_data_id: link.data_id(),
        link_semantic_before_id,
        link_semantic_after_id,
        link_transition_sequence_before,
        link_transition_sequence_after,
        series_plan_id: link_binding.series_plan_id,
        ordinal: link_binding.ordinal,
        market_instance_id: link_binding.market_instance_id,
        generation: link_binding.generation,
        obligation_configuration_id: obligation_configuration_id.content_id(),
        obligation,
        owner_admission_receipt_id,
        product_admission_projection_id,
        child_account,
        owner_prewrite_id,
    })
}

/// Consume the physical child postwrite and persist LinkV3 last.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn commit_product_link_obligation_admission_v3<'next, P>(
    program_id: &Pubkey,
    link_account: &AccountInfo<'_>,
    plan: AuthenticatedProductLinkObligationAdmissionPlanV3,
    physical: P,
    link_before_output: &mut SeriesMarketLinkAccountV3,
    rebound_output: &'next mut SeriesMarketLinkAccountV3,
) -> Outcome<(
    AuthenticatedSeriesMarketLinkV3<'next>,
    AuthenticatedProductLinkObligationAdmissionV3,
)>
where
    P: AuthenticatedProductLinkObligationAdmissionPostwriteV3,
{
    let link_before = authenticate_series_market_link_v3(
        program_id,
        link_account,
        plan.series_plan_id,
        plan.ordinal,
        plan.market_instance_id,
        plan.generation,
        plan.root_account,
        true,
        link_before_output,
    )?;
    require(
        link_before.binding_id() == plan.link_binding_id
            && link_before.authentication_id() == plan.link_authentication_id
            && link_before.data_id() == plan.link_data_id
            && link_before.semantic_id() == plan.link_semantic_before_id
            && link_before.state().transition_sequence()
                == plan.link_transition_sequence_before,
        ClutchError::MismatchedState,
    )?;
    let projection = SeriesLinkObligationAdmissionProjectionV3 {
        link_semantic_id: plan.link_semantic_before_id,
        obligation: plan.obligation,
        link_transition_sequence: plan.link_transition_sequence_after,
        owner_admission_receipt_id: plan.owner_admission_receipt_id,
    };
    require(
        projection
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == plan.product_admission_projection_id,
        ClutchError::MismatchedState,
    )?;
    let successor = link_before
        .state()
        .admit_obligation(projection)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let successor_semantic_id = successor
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        successor_semantic_id == plan.link_semantic_after_id
            && successor.transition_sequence() == plan.link_transition_sequence_after,
        ClutchError::MismatchedState,
    )?;
    let physical_postwrite_id = physical.consume_product_link_obligation_admission_postwrite_v3(
        plan.id,
        plan.root_account,
        plan.root_binding_id,
        plan.root_transition_sequence,
        plan.link_account,
        plan.link_binding_id,
        plan.link_semantic_before_id,
        plan.link_semantic_after_id,
        plan.link_transition_sequence_before,
        plan.link_transition_sequence_after,
        plan.obligation,
        plan.owner_admission_receipt_id,
        plan.product_admission_projection_id,
        plan.child_account,
        plan.owner_prewrite_id,
    )?;
    require_live(physical_postwrite_id)?;
    let authentication_before_id = link_before.authentication_id();
    let data_before_id = link_before.data_id();
    let stored_bump = link_before.value().stored_bump;
    let observed_lamports = link_before.observed_lamports();
    {
        let mut data = link_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        SeriesMarketLinkAccountV3::encode_parts(&successor, stored_bump, &mut data)?;
    }
    let rebound = authenticate_series_market_link_v3(
        program_id,
        link_account,
        plan.series_plan_id,
        plan.ordinal,
        plan.market_instance_id,
        plan.generation,
        plan.root_account,
        true,
        rebound_output,
    )?;
    let authentication_after_id = rebound.authentication_id();
    require(
        rebound.state() == &successor
            && rebound.binding_id() == plan.link_binding_id
            && rebound.observed_lamports() == observed_lamports
            && rebound.semantic_id() == plan.link_semantic_after_id
            && authentication_after_id != authentication_before_id
            && rebound.data_id() != data_before_id,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_LINK_OBLIGATION_ADMISSION_POSTWRITE_DOMAIN_V3,
        &plan.id.bytes(),
        &physical_postwrite_id.bytes(),
        link_account.key.as_ref(),
        &plan.link_binding_id.bytes(),
        &authentication_before_id.bytes(),
        &authentication_after_id.bytes(),
        &plan.link_semantic_before_id.bytes(),
        &plan.link_semantic_after_id.bytes(),
        &plan.product_admission_projection_id.bytes(),
    ]);
    require_live(id)?;
    Ok((
        rebound,
        AuthenticatedProductLinkObligationAdmissionV3 {
            id,
            physical_postwrite_id,
            link_account: plan.link_account,
            link_binding_id: plan.link_binding_id,
            link_authentication_before_id: authentication_before_id,
            link_authentication_after_id: authentication_after_id,
            link_semantic_before_id: plan.link_semantic_before_id,
            link_semantic_after_id: plan.link_semantic_after_id,
            link_transition_sequence_before: plan.link_transition_sequence_before,
            link_transition_sequence_after: plan.link_transition_sequence_after,
            obligation: plan.obligation,
            owner_admission_receipt_id: plan.owner_admission_receipt_id,
            product_admission_projection_id: plan.product_admission_projection_id,
            child_account: plan.child_account,
        },
    ))
}

fn hashv(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}
