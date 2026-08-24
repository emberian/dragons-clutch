//! Two-phase current Product family admission into RootV3.
//!
//! Product first derives the exact predicted RootV3 successor from a
//! default-refusing child prewrite. The family owner then performs and hostile
//! authenticates its physical creation in the same instruction. Product
//! consumes that move-only postwrite and persists RootV3 last. A caller ID or
//! physical account address alone never authorizes admission.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use clutch_product_series::{
    AuthenticatedMarketFamilyAuthorityV1, ContentId, MarketFamilyAggregatorV1,
    MarketFamilyV1, MarketInstanceV2Id, MarketLifecyclePhaseV3, MarketLifecycleRootV3,
};
use clutch_solana_layout::product_series::MarketLifecycleRootAccountV3;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::product_market_family_capability_current::
    AuthenticatedMarketFamilyCapabilityPolicyV1;
use super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, AuthenticatedMarketLifecycleRootV3,
};
use super::product_market_replay_current::AuthenticatedMarketLifecycleReplayV2;

const PRODUCT_FAMILY_ADMISSION_PREAUTHORIZATION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-family-admission-preauthorization/v3\0";
const PRODUCT_FAMILY_ADMISSION_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-family-admission-receipt/v3\0";
const PRODUCT_FAMILY_ADMISSION_POSTWRITE_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/product-family-admission-postwrite/v3\0";

/// Default-refusing physical family prewrite. Concrete Direct, General,
/// Fractional, Dealer, and Structured plans implement this only after deriving
/// their child account beneath the exact Product namespace anchor.
pub(crate) trait AuthenticatedProductFamilyAdmissionOwnerV3 {
    fn family(&self) -> Outcome<MarketFamilyV1> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn child_account(&self) -> Outcome<Pubkey> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    fn owner_prewrite_id(&self) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
    #[allow(clippy::too_many_arguments)]
    fn authenticate_product_family_admission_owner_v3(
        &self,
        _program_id: &Pubkey,
        _root_account: Pubkey,
        _root_binding_id: ContentId,
        _root_authentication_id: ContentId,
        _root_semantic_id: ContentId,
        _root_transition_sequence: u64,
        _market_instance_id: MarketInstanceV2Id,
        _generation: u64,
        _family_policy_id: ContentId,
        _family_policy_authentication_id: ContentId,
        _family: MarketFamilyV1,
        _family_namespace_anchor_id: ContentId,
        _family_admission_sequence: u32,
        _child_account: Pubkey,
        _owner_prewrite_id: ContentId,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Default-refusing move-only physical family postwrite.
pub(crate) trait AuthenticatedProductFamilyAdmissionPostwriteV3: Sized {
    #[allow(clippy::too_many_arguments)]
    fn consume_product_family_admission_postwrite_v3(
        self,
        _plan_id: ContentId,
        _root_account: Pubkey,
        _root_binding_id: ContentId,
        _root_semantic_before_id: ContentId,
        _root_semantic_after_id: ContentId,
        _root_transition_sequence_before: u64,
        _root_transition_sequence_after: u64,
        _family: MarketFamilyV1,
        _family_namespace_anchor_id: ContentId,
        _family_prestate_id: ContentId,
        _family_poststate_id: ContentId,
        _family_admission_sequence: u32,
        _family_admission_receipt_id: ContentId,
        _child_account: Pubkey,
        _owner_prewrite_id: ContentId,
    ) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

struct ExactFamilyAdmissionAuthorityV3 {
    binding_id: ContentId,
    family_prestate_id: ContentId,
    family: MarketFamilyV1,
    family_namespace_anchor_id: ContentId,
    family_admission_sequence: u32,
    family_admission_receipt_id: ContentId,
}

impl AuthenticatedMarketFamilyAuthorityV1 for ExactFamilyAdmissionAuthorityV3 {
    fn authenticate_admission(
        &self,
        current: &MarketFamilyAggregatorV1,
        family: MarketFamilyV1,
        family_root_id: ContentId,
        family_admission_sequence: u32,
        admission_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if current.binding().id()?.content_id() != self.binding_id
            || current.semantic_id()?.content_id() != self.family_prestate_id
            || family != self.family
            || family_root_id != self.family_namespace_anchor_id
            || family_admission_sequence != self.family_admission_sequence
            || admission_receipt_id != self.family_admission_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Instruction-local preauthorization. The predicted successor is recomputed
/// from hostile RootV3 during commit; this compact receipt is not persisted
/// authority and cannot mutate RootV3 by itself.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductFamilyAdmissionPlanV3 {
    id: ContentId,
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_direct_global_liveness_binding_id: ContentId,
    root_authentication_id: ContentId,
    root_data_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
    family_policy_id: ContentId,
    family_policy_authentication_id: ContentId,
    family: MarketFamilyV1,
    family_namespace_anchor_id: ContentId,
    family_prestate_id: ContentId,
    family_poststate_id: ContentId,
    family_admission_sequence: u32,
    family_admission_receipt_id: ContentId,
    child_account: Pubkey,
    owner_prewrite_id: ContentId,
}

impl AuthenticatedProductFamilyAdmissionPlanV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.root_binding_id }
    pub(crate) const fn root_direct_global_liveness_binding_id(&self) -> ContentId {
        self.root_direct_global_liveness_binding_id
    }
    pub(crate) const fn root_semantic_before_id(&self) -> ContentId {
        self.root_semantic_before_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_before(&self) -> u64 {
        self.root_transition_sequence_before
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
    pub(crate) const fn market_instance_id(&self) -> MarketInstanceV2Id {
        self.market_instance_id
    }
    pub(crate) const fn generation(&self) -> u64 { self.generation }
    pub(crate) const fn family(&self) -> MarketFamilyV1 { self.family }
    pub(crate) const fn family_namespace_anchor_id(&self) -> ContentId {
        self.family_namespace_anchor_id
    }
    pub(crate) const fn family_prestate_id(&self) -> ContentId {
        self.family_prestate_id
    }
    pub(crate) const fn family_poststate_id(&self) -> ContentId {
        self.family_poststate_id
    }
    pub(crate) const fn family_admission_sequence(&self) -> u32 {
        self.family_admission_sequence
    }
    pub(crate) const fn family_admission_receipt_id(&self) -> ContentId {
        self.family_admission_receipt_id
    }
    pub(crate) const fn child_account(&self) -> Pubkey { self.child_account }
    pub(crate) const fn owner_prewrite_id(&self) -> ContentId { self.owner_prewrite_id }
}

/// Move-only RootV3 postwrite consumed by the family-specific outer.
#[derive(Debug)]
pub(crate) struct AuthenticatedProductFamilyAdmissionV3 {
    id: ContentId,
    physical_postwrite_id: ContentId,
    root_account: Pubkey,
    root_binding_id: ContentId,
    root_authentication_before_id: ContentId,
    root_authentication_after_id: ContentId,
    root_semantic_before_id: ContentId,
    root_semantic_after_id: ContentId,
    root_transition_sequence_before: u64,
    root_transition_sequence_after: u64,
    family: MarketFamilyV1,
    family_namespace_anchor_id: ContentId,
    family_prestate_id: ContentId,
    family_poststate_id: ContentId,
    family_admission_sequence: u32,
    family_admission_receipt_id: ContentId,
    child_account: Pubkey,
}

impl AuthenticatedProductFamilyAdmissionV3 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn physical_postwrite_id(&self) -> ContentId {
        self.physical_postwrite_id
    }
    pub(crate) const fn root_account(&self) -> Pubkey { self.root_account }
    pub(crate) const fn root_binding_id(&self) -> ContentId { self.root_binding_id }
    pub(crate) const fn root_authentication_after_id(&self) -> ContentId {
        self.root_authentication_after_id
    }
    pub(crate) const fn root_semantic_after_id(&self) -> ContentId {
        self.root_semantic_after_id
    }
    pub(crate) const fn root_transition_sequence_after(&self) -> u64 {
        self.root_transition_sequence_after
    }
    pub(crate) const fn family(&self) -> MarketFamilyV1 { self.family }
    pub(crate) const fn family_admission_receipt_id(&self) -> ContentId {
        self.family_admission_receipt_id
    }
    pub(crate) const fn child_account(&self) -> Pubkey { self.child_account }
}

/// Prepare one exact Product family admission without writing RootV3.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn prepare_product_family_admission_v3<A>(
    program_id: &Pubkey,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    replay: &AuthenticatedMarketLifecycleReplayV2,
    family_policy: &AuthenticatedMarketFamilyCapabilityPolicyV1,
    owner: &A,
    successor_output: &mut MarketLifecycleRootV3,
) -> Outcome<AuthenticatedProductFamilyAdmissionPlanV3>
where
    A: AuthenticatedProductFamilyAdmissionOwnerV3 + ?Sized,
{
    let binding = root.binding();
    let replay_binding = replay.state().binding();
    let family = owner.family()?;
    let child_account = owner.child_account()?;
    let owner_prewrite_id = owner.owner_prewrite_id()?;
    require_live(owner_prewrite_id)?;
    let family_namespace_anchor_id = family_policy.family_namespace_anchors()[family.index()];
    let current_families = root.state().product_families();
    let family_prestate_id = current_families
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let family_admission_sequence = current_families.family(family).counts().admitted;
    let root_transition_sequence_before = root.state().transition_sequence();
    let root_transition_sequence_after = root_transition_sequence_before
        .checked_add(1)
        .ok_or(ClutchError::Arithmetic)?;
    require(
        root.is_writable()
            && matches!(
                root.state().phase(),
                MarketLifecyclePhaseV3::Founding | MarketLifecyclePhaseV3::Active
            )
            && replay.state().phase()
                == clutch_product_series::MarketLifecycleReplayPhaseV2::FoundationSettled
            && replay_binding.market_instance_id == binding.market_instance_id
            && replay_binding.generation == binding.generation
            && binding.market_lifecycle_replay_account_id.bytes()
                == replay.account().to_bytes()
            && binding.market_lifecycle_generation_binding_id
                == replay_binding
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            && replay_binding.market_family_capability_policy_id == family_policy.policy_id()
            && replay_binding.market_family_capability_authentication_id
                == family_policy.founder_artifact_authentication_id()
            && current_families.binding() == family_policy.aggregator().binding()
            && current_families.admits_new_child(family)
            && family_namespace_anchor_id
                == current_families.binding().family_root_id(family)
            && child_account != root.account()
            && child_account.to_bytes() != family_namespace_anchor_id.bytes(),
        ClutchError::MismatchedState,
    )?;
    owner.authenticate_product_family_admission_owner_v3(
        program_id,
        root.account(),
        root.binding_id(),
        root.authentication_id(),
        root.semantic_id(),
        root_transition_sequence_before,
        binding.market_instance_id,
        binding.generation,
        family_policy.policy_id(),
        family_policy.id(),
        family,
        family_namespace_anchor_id,
        family_admission_sequence,
        child_account,
        owner_prewrite_id,
    )?;
    let family_admission_receipt_id = hashv(&[
        PRODUCT_FAMILY_ADMISSION_RECEIPT_DOMAIN_V3,
        program_id.as_ref(),
        root.account().as_ref(),
        &root.binding_id().bytes(),
        &binding.direct_global_liveness_binding_id.bytes(),
        &root.authentication_id().bytes(),
        &root.semantic_id().bytes(),
        &root_transition_sequence_before.to_le_bytes(),
        &root_transition_sequence_after.to_le_bytes(),
        &family_policy.policy_id().bytes(),
        &family_policy.id().bytes(),
        &[family.byte()],
        &family_namespace_anchor_id.bytes(),
        &family_admission_sequence.to_le_bytes(),
        child_account.as_ref(),
        &owner_prewrite_id.bytes(),
    ]);
    require_live(family_admission_receipt_id)?;
    let authority = ExactFamilyAdmissionAuthorityV3 {
        binding_id: current_families
            .binding()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .content_id(),
        family_prestate_id,
        family,
        family_namespace_anchor_id,
        family_admission_sequence,
        family_admission_receipt_id,
    };
    root.state()
        .admit_product_family_child_into(
            &authority,
            family,
            family_admission_sequence,
            family_admission_receipt_id,
            successor_output,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let root_semantic_after_id = successor_output
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let family_poststate_id = successor_output
        .product_families()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let id = hashv(&[
        PRODUCT_FAMILY_ADMISSION_PREAUTHORIZATION_DOMAIN_V3,
        &family_admission_receipt_id.bytes(),
        &root.semantic_id().bytes(),
        &root_semantic_after_id.bytes(),
        &family_prestate_id.bytes(),
        &family_poststate_id.bytes(),
        &owner_prewrite_id.bytes(),
    ]);
    require_live(id)?;
    Ok(AuthenticatedProductFamilyAdmissionPlanV3 {
        id,
        root_account: root.account(),
        root_binding_id: root.binding_id(),
        root_direct_global_liveness_binding_id:
            binding.direct_global_liveness_binding_id,
        root_authentication_id: root.authentication_id(),
        root_data_id: root.data_id(),
        root_semantic_before_id: root.semantic_id(),
        root_semantic_after_id,
        root_transition_sequence_before,
        root_transition_sequence_after,
        market_instance_id: binding.market_instance_id,
        generation: binding.generation,
        family_policy_id: family_policy.policy_id(),
        family_policy_authentication_id: family_policy.id(),
        family,
        family_namespace_anchor_id,
        family_prestate_id,
        family_poststate_id,
        family_admission_sequence,
        family_admission_receipt_id,
        child_account,
        owner_prewrite_id,
    })
}

/// Consume the physical family postwrite, persist RootV3 last, and hostile
/// reopen the exact predicted successor.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn commit_product_family_admission_v3<'next, P>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    plan: AuthenticatedProductFamilyAdmissionPlanV3,
    physical: P,
    root_before_output: &mut MarketLifecycleRootAccountV3,
    successor_output: &mut MarketLifecycleRootV3,
    rebound_output: &'next mut MarketLifecycleRootAccountV3,
) -> Outcome<(
    AuthenticatedMarketLifecycleRootV3<'next>,
    AuthenticatedProductFamilyAdmissionV3,
)>
where
    P: AuthenticatedProductFamilyAdmissionPostwriteV3,
{
    let root_before = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        plan.market_instance_id,
        plan.generation,
        true,
        root_before_output,
    )?;
    require(
        root_before.account() == plan.root_account
            && root_before.binding_id() == plan.root_binding_id
            && root_before.authentication_id() == plan.root_authentication_id
            && root_before.data_id() == plan.root_data_id
            && root_before.semantic_id() == plan.root_semantic_before_id
            && root_before.state().transition_sequence()
                == plan.root_transition_sequence_before
            && root_before.binding().market_instance_id == plan.market_instance_id
            && root_before.binding().generation == plan.generation,
        ClutchError::MismatchedState,
    )?;
    let current_families = root_before.state().product_families();
    let authority = ExactFamilyAdmissionAuthorityV3 {
        binding_id: current_families
            .binding()
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .content_id(),
        family_prestate_id: plan.family_prestate_id,
        family: plan.family,
        family_namespace_anchor_id: plan.family_namespace_anchor_id,
        family_admission_sequence: plan.family_admission_sequence,
        family_admission_receipt_id: plan.family_admission_receipt_id,
    };
    root_before
        .state()
        .admit_product_family_child_into(
            &authority,
            plan.family,
            plan.family_admission_sequence,
            plan.family_admission_receipt_id,
            successor_output,
        )
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let successor_semantic_id = successor_output
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let successor_family_id = successor_output
        .product_families()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require(
        successor_semantic_id == plan.root_semantic_after_id
            && successor_family_id == plan.family_poststate_id
            && successor_output.transition_sequence() == plan.root_transition_sequence_after,
        ClutchError::MismatchedState,
    )?;
    let physical_postwrite_id = physical.consume_product_family_admission_postwrite_v3(
        plan.id,
        plan.root_account,
        plan.root_binding_id,
        plan.root_semantic_before_id,
        plan.root_semantic_after_id,
        plan.root_transition_sequence_before,
        plan.root_transition_sequence_after,
        plan.family,
        plan.family_namespace_anchor_id,
        plan.family_prestate_id,
        plan.family_poststate_id,
        plan.family_admission_sequence,
        plan.family_admission_receipt_id,
        plan.child_account,
        plan.owner_prewrite_id,
    )?;
    require_live(physical_postwrite_id)?;
    let before_authentication_id = root_before.authentication_id();
    let before_semantic_id = root_before.semantic_id();
    let before_data_id = root_before.data_id();
    let observed_lamports = root_before.observed_lamports();
    let rent_principal_lamports = root_before.value().rent_principal_lamports;
    let stored_bump = root_before.value().stored_bump;
    {
        let mut data = root_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        MarketLifecycleRootAccountV3::encode_parts(
            successor_output,
            rent_principal_lamports,
            stored_bump,
            &mut data,
        )?;
    }
    let rebound = authenticate_market_lifecycle_root_v3(
        program_id,
        root_account,
        plan.market_instance_id,
        plan.generation,
        true,
        rebound_output,
    )?;
    let after_authentication_id = rebound.authentication_id();
    require(
        rebound.state() == successor_output
            && rebound.observed_lamports() == observed_lamports
            && rebound.binding_id() == plan.root_binding_id
            && rebound.semantic_id() == plan.root_semantic_after_id
            && after_authentication_id != before_authentication_id
            && rebound.data_id() != before_data_id,
        ClutchError::MismatchedState,
    )?;
    let id = hashv(&[
        PRODUCT_FAMILY_ADMISSION_POSTWRITE_DOMAIN_V3,
        &plan.id.bytes(),
        &physical_postwrite_id.bytes(),
        root_account.key.as_ref(),
        &plan.root_binding_id.bytes(),
        &before_authentication_id.bytes(),
        &after_authentication_id.bytes(),
        &before_semantic_id.bytes(),
        &plan.root_semantic_after_id.bytes(),
        &plan.family_prestate_id.bytes(),
        &plan.family_poststate_id.bytes(),
        &plan.family_admission_receipt_id.bytes(),
    ]);
    require_live(id)?;
    Ok((
        rebound,
        AuthenticatedProductFamilyAdmissionV3 {
            id,
            physical_postwrite_id,
            root_account: plan.root_account,
            root_binding_id: plan.root_binding_id,
            root_authentication_before_id: before_authentication_id,
            root_authentication_after_id: after_authentication_id,
            root_semantic_before_id: before_semantic_id,
            root_semantic_after_id: plan.root_semantic_after_id,
            root_transition_sequence_before: plan.root_transition_sequence_before,
            root_transition_sequence_after: plan.root_transition_sequence_after,
            family: plan.family,
            family_namespace_anchor_id: plan.family_namespace_anchor_id,
            family_prestate_id: plan.family_prestate_id,
            family_poststate_id: plan.family_poststate_id,
            family_admission_sequence: plan.family_admission_sequence,
            family_admission_receipt_id: plan.family_admission_receipt_id,
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
