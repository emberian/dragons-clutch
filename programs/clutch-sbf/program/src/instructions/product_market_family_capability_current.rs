//! Current immutable Product-family capability authority.
//!
//! One hostile content-addressed policy owns the exact five-bit family mask.
//! The five family roots are non-persisted namespace anchors derived from the
//! Market/generation/family tuple; the embedded RootV3 aggregator remains the
//! sole persisted count and status owner.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedProductArtifactV1,
};
use crate::instructions::product_market_replay_current::AuthenticatedMarketLifecycleReplayV2;
use crate::instructions::product_series::physical_v5::AuthenticatedSeriesPhysicalFounderV5;
use crate::instructions::product_series_current::AuthenticatedRegistryCapabilityV5;
use crate::seeds;
use clutch_product_series::{
    AuthenticatedMarketFamilyAuthorityV1, ContentId, MarketFamilyAggregatorBindingV1,
    MarketFamilyAggregatorV1, MarketFamilyCapabilityPolicyV1,
    MarketFamilyCapabilityPolicyV1Id, MarketFamilyV1, MarketInstanceV2Id,
    RegistryCapabilityProfileV3Id, RegistryProgramReleaseV1Id,
    SeriesLinkObligationConfigurationV3, MARKET_FAMILIES_V1, MARKET_FAMILY_COUNT_V1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::product_market_lifecycle_v3_current::AuthenticatedMarketLifecycleRootV3;

const PRODUCT_MARKET_FAMILY_CAPABILITY_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-market-family-capability-authentication/v1\0";
const PRODUCT_MARKET_FAMILY_CAPABILITY_ARTIFACT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-market-family-capability-artifact-authentication/v1\0";
const PRODUCT_MARKET_FAMILY_CAPABILITY_CURRENT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-market-family-capability-current-authentication/v1\0";

struct ExactMarketFamilyInitializationV1 {
    policy_id: ContentId,
    binding_id: ContentId,
}

/// Move-only pre-generation authority over the exact immutable policy and its
/// current RegistryV5/physical-FundingV5 joins.
#[derive(Debug)]
pub(crate) struct AuthenticatedMarketFamilyCapabilityPolicyArtifactV1 {
    id: ContentId,
    policy: AuthenticatedProductArtifactV1<MarketFamilyCapabilityPolicyV1>,
    physical_founder_id: ContentId,
    physical_capitalization_id: ContentId,
    registry_capability_id: ContentId,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    attachment_plan_id: ContentId,
}

impl AuthenticatedMarketFamilyCapabilityPolicyArtifactV1 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn policy_id(&self) -> ContentId { self.policy.semantic_id() }
    pub(crate) const fn registry_release_id(&self) -> ContentId {
        self.registry_release_id
    }
    pub(crate) const fn capability_profile_id(&self) -> ContentId {
        self.capability_profile_id
    }
    pub(crate) const fn physical_founder_id(&self) -> ContentId {
        self.physical_founder_id
    }
    pub(crate) const fn physical_capitalization_id(&self) -> ContentId {
        self.physical_capitalization_id
    }
    pub(crate) const fn registry_capability_id(&self) -> ContentId {
        self.registry_capability_id
    }
    pub(crate) const fn attachment_plan_id(&self) -> ContentId {
        self.attachment_plan_id
    }
}

impl AuthenticatedMarketFamilyAuthorityV1 for ExactMarketFamilyInitializationV1 {
    fn authenticate_initialization(
        &self,
        binding: &MarketFamilyAggregatorBindingV1,
    ) -> clutch_product_series::Result<()> {
        if self.policy_id.is_zero()
            || binding.id()?.content_id() != self.binding_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Move-only authority over the exact family policy, anchors, aggregator, and
/// attachment-bound initial obligation statuses.
#[derive(Debug)]
pub(crate) struct AuthenticatedMarketFamilyCapabilityPolicyV1 {
    id: ContentId,
    policy: AuthenticatedProductArtifactV1<MarketFamilyCapabilityPolicyV1>,
    family_namespace_anchors: [ContentId; MARKET_FAMILY_COUNT_V1],
    aggregator: MarketFamilyAggregatorV1,
    founder_artifact_authentication_id: ContentId,
}

impl AuthenticatedMarketFamilyCapabilityPolicyV1 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn policy_id(&self) -> ContentId {
        self.policy.semantic_id()
    }

    pub(crate) const fn policy_account(&self) -> Pubkey {
        self.policy.account()
    }

    pub(crate) const fn family_namespace_anchors(
        &self,
    ) -> &[ContentId; MARKET_FAMILY_COUNT_V1] {
        &self.family_namespace_anchors
    }

    pub(crate) const fn founder_artifact_authentication_id(&self) -> ContentId {
        self.founder_artifact_authentication_id
    }

    pub(crate) const fn aggregator(&self) -> MarketFamilyAggregatorV1 {
        self.aggregator
    }

    pub(crate) fn obligation_configuration(
        &self,
        attachment_plan_id: ContentId,
    ) -> Outcome<SeriesLinkObligationConfigurationV3> {
        self.policy
            .value()
            .obligation_configuration_v3(attachment_plan_id)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))
    }
}

/// Hostile-authenticate the immutable family policy before generation exists.
#[inline(never)]
pub(crate) fn authenticate_market_family_capability_policy_artifact_v1(
    program_id: &Pubkey,
    physical: &AuthenticatedSeriesPhysicalFounderV5,
    registry: &AuthenticatedRegistryCapabilityV5,
    policy_account: &AccountInfo<'_>,
    expected_policy_id: MarketFamilyCapabilityPolicyV1Id,
) -> Outcome<AuthenticatedMarketFamilyCapabilityPolicyArtifactV1> {
    let policy = authenticate_product_artifact_v1::<MarketFamilyCapabilityPolicyV1>(
        program_id,
        policy_account,
        expected_policy_id.content_id(),
    )?;
    let value = policy.value();
    require(
        physical.registry_capability_after_id() == registry.id()
            && physical.capability_profile_id() == registry.capability_profile_id()
            && physical.registry_release_id() == registry.registry_release_id()
            && physical.collateral_realm_id() == value.realm_id
            && physical.collateral_profile_id() == value.collateral_profile_id
            && value.registry_capability_profile_id.content_id()
                == registry.capability_profile_id()
            && physical.attachment_plan_id() != policy.semantic_id()
            && policy.account() != physical.capitalization().registry_account(),
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_MARKET_FAMILY_CAPABILITY_ARTIFACT_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            &physical.id().bytes(),
            &physical.capitalization_id().bytes(),
            &registry.id().bytes(),
            &registry.registry_release_id().bytes(),
            &registry.capability_profile_id().bytes(),
            policy.account().as_ref(),
            &policy.semantic_id().bytes(),
            &value.realm_id.bytes(),
            &value.collateral_profile_id.bytes(),
            &value.registry_capability_profile_id.bytes(),
            &[value.enabled_family_mask],
            &physical.attachment_plan_id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedMarketFamilyCapabilityPolicyArtifactV1 {
        id,
        policy,
        physical_founder_id: physical.id(),
        physical_capitalization_id: physical.capitalization_id(),
        registry_capability_id: registry.id(),
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        attachment_plan_id: physical.attachment_plan_id(),
    })
}

/// Consume the pre-generation authority only after the persisted replay owner
/// fixes the nonzero Market generation, then derive every namespace anchor and
/// the exact LinkV3 obligation configuration.
#[inline(never)]
pub(crate) fn complete_market_family_capability_policy_v1(
    program_id: &Pubkey,
    artifact: AuthenticatedMarketFamilyCapabilityPolicyArtifactV1,
    market_instance_id: MarketInstanceV2Id,
    replay: &AuthenticatedMarketLifecycleReplayV2,
) -> Outcome<AuthenticatedMarketFamilyCapabilityPolicyV1> {
    let replay_binding = replay.state().binding();
    let generation = replay.generation();
    require(
        replay_binding.market_instance_id == market_instance_id
            && replay_binding.market_family_capability_policy_id == artifact.policy_id()
            && replay_binding.market_family_capability_authentication_id == artifact.id()
            && replay_binding.physical_capitalization_receipt_id
                == artifact.physical_capitalization_id
            && replay_binding.registry_release_id.content_id() == artifact.registry_release_id
            && replay_binding.capability_profile_id.content_id()
                == artifact.capability_profile_id
            && generation != 0,
        ClutchError::MismatchedState,
    )?;
    let value = artifact.policy.value();
    let mut family_namespace_anchors = [ContentId::ZERO; MARKET_FAMILY_COUNT_V1];
    let market = market_instance_id.bytes();
    let mut index = 0usize;
    while index < MARKET_FAMILY_COUNT_V1 {
        let family = MARKET_FAMILIES_V1[index];
        let anchor = seeds::product_market_family_root_v1_pda(
            program_id,
            &market,
            generation,
            family.byte(),
        )
        .0;
        require(anchor != artifact.policy.account(), ClutchError::AccountAlias)?;
        family_namespace_anchors[index] = ContentId::from_bytes(anchor.to_bytes());
        index = index.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    }
    let binding = MarketFamilyAggregatorBindingV1 {
        market_instance_id,
        generation,
        registry_release_id: RegistryProgramReleaseV1Id::from_bytes(
            artifact.registry_release_id.bytes(),
        ),
        capability_profile_id: RegistryCapabilityProfileV3Id::from_bytes(
            artifact.capability_profile_id.bytes(),
        ),
        enabled_family_mask: value.enabled_family_mask,
        family_root_ids: family_namespace_anchors,
    };
    let binding_id = binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let initialization = ExactMarketFamilyInitializationV1 {
        policy_id: artifact.policy.semantic_id(),
        binding_id: binding_id.content_id(),
    };
    let aggregator = MarketFamilyAggregatorV1::initialize(&initialization, binding)
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    let obligation_configuration = value
        .obligation_configuration_v3(artifact.attachment_plan_id)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let statuses = obligation_configuration.initial_statuses;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_MARKET_FAMILY_CAPABILITY_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            &artifact.id.bytes(),
            &artifact.physical_founder_id.bytes(),
            &artifact.physical_capitalization_id.bytes(),
            &artifact.registry_capability_id.bytes(),
            artifact.policy.account().as_ref(),
            &artifact.policy.semantic_id().bytes(),
            &value.realm_id.bytes(),
            &value.collateral_profile_id.bytes(),
            &value.registry_capability_profile_id.bytes(),
            &[value.enabled_family_mask],
            &market_instance_id.bytes(),
            &generation.to_le_bytes(),
            &family_namespace_anchors[MarketFamilyV1::General.index()].bytes(),
            &family_namespace_anchors[MarketFamilyV1::Direct.index()].bytes(),
            &family_namespace_anchors[MarketFamilyV1::Fractional.index()].bytes(),
            &family_namespace_anchors[MarketFamilyV1::Dealer.index()].bytes(),
            &family_namespace_anchors[MarketFamilyV1::Structured.index()].bytes(),
            &binding_id.bytes(),
            &obligation_configuration
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes(),
            &[
                statuses[0].wire_byte(),
                statuses[1].wire_byte(),
                statuses[2].wire_byte(),
                statuses[3].wire_byte(),
            ],
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedMarketFamilyCapabilityPolicyV1 {
        id,
        policy: artifact.policy,
        family_namespace_anchors,
        aggregator,
        founder_artifact_authentication_id: artifact.id,
    })
}

/// Hostile-reconstruct the immutable family policy after foundation.
///
/// The policy ID is read only from the authenticated replay binding. The
/// content-addressed policy body, canonical namespace anchors, and the exact
/// current embedded RootV3 aggregator are then recomputed and cross-checked;
/// no founder-only artifact receipt or caller attachment ID is accepted.
#[inline(never)]
pub(crate) fn authenticate_current_market_family_capability_policy_v1(
    program_id: &Pubkey,
    root: &AuthenticatedMarketLifecycleRootV3<'_>,
    replay: &AuthenticatedMarketLifecycleReplayV2,
    policy_account: &AccountInfo<'_>,
) -> Outcome<AuthenticatedMarketFamilyCapabilityPolicyV1> {
    let replay_binding = replay.state().binding();
    let expected_policy_id = replay_binding.market_family_capability_policy_id;
    let policy = authenticate_product_artifact_v1::<MarketFamilyCapabilityPolicyV1>(
        program_id,
        policy_account,
        expected_policy_id,
    )?;
    let value = policy.value();
    let root_binding = root.binding();
    let replay_binding_id = replay_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut family_namespace_anchors = [ContentId::ZERO; MARKET_FAMILY_COUNT_V1];
    let market = root_binding.market_instance_id.bytes();
    let mut index = 0usize;
    while index < MARKET_FAMILY_COUNT_V1 {
        let family = MARKET_FAMILIES_V1[index];
        family_namespace_anchors[index] = ContentId::from_bytes(
            seeds::product_market_family_root_v1_pda(
                program_id,
                &market,
                root_binding.generation,
                family.byte(),
            )
            .0
            .to_bytes(),
        );
        index = index.checked_add(1).ok_or(ClutchError::Arithmetic)?;
    }
    let aggregator = *root.state().product_families();
    let aggregator_binding = aggregator.binding();
    let aggregator_id = aggregator
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root.owner_program() == *program_id
            && replay.account().to_bytes()
                == root_binding.market_lifecycle_replay_account_id.bytes()
            && replay_binding_id == root_binding.market_lifecycle_generation_binding_id
            && replay_binding.market_instance_id == root_binding.market_instance_id
            && replay_binding.generation == root_binding.generation
            && replay_binding.market_family_capability_authentication_id != ContentId::ZERO
            && policy.semantic_id() == expected_policy_id
            && value.realm_id == root_binding.realm_id
            && value.collateral_profile_id == root_binding.collateral_profile_id
            && value.registry_capability_profile_id.content_id()
                == root_binding.capability_profile_id
            && replay_binding.registry_release_id.content_id()
                == root_binding.registry_release_id
            && replay_binding.capability_profile_id.content_id()
                == root_binding.capability_profile_id
            && aggregator_binding.market_instance_id == root_binding.market_instance_id
            && aggregator_binding.generation == root_binding.generation
            && aggregator_binding.registry_release_id.content_id()
                == root_binding.registry_release_id
            && aggregator_binding.capability_profile_id.content_id()
                == root_binding.capability_profile_id
            && aggregator_binding.enabled_family_mask == value.enabled_family_mask
            && aggregator_binding.family_root_ids == family_namespace_anchors,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_MARKET_FAMILY_CAPABILITY_CURRENT_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            root.account().as_ref(),
            &root.binding_id().bytes(),
            &root.authentication_id().bytes(),
            &root.semantic_id().bytes(),
            replay.account().as_ref(),
            &replay.authentication_id().bytes(),
            &replay_binding_id.bytes(),
            &replay_binding.market_family_capability_authentication_id.bytes(),
            policy.account().as_ref(),
            &policy.semantic_id().bytes(),
            &aggregator_id.bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedMarketFamilyCapabilityPolicyV1 {
        id,
        policy,
        family_namespace_anchors,
        aggregator,
        founder_artifact_authentication_id:
            replay_binding.market_family_capability_authentication_id,
    })
}
