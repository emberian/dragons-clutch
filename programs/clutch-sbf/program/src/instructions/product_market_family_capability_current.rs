//! Current immutable Product-family capability authority.
//!
//! One hostile content-addressed policy owns the exact five-bit family mask.
//! The five family roots are non-persisted namespace anchors derived from the
//! Market/generation/family tuple; the embedded RootV2 aggregator remains the
//! sole persisted count and status owner.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedProductArtifactV1,
};
use crate::instructions::product_series::physical_v4::AuthenticatedSeriesPhysicalFounderV4;
use crate::instructions::product_series_current::AuthenticatedRegistryCapabilityV4;
use crate::seeds;
use clutch_product_series::{
    AuthenticatedMarketFamilyAuthorityV1, ContentId, MarketFamilyAggregatorBindingV1,
    MarketFamilyAggregatorV1, MarketFamilyCapabilityPolicyV1,
    MarketFamilyCapabilityPolicyV1Id, MarketFamilyV1, MarketInstanceV2Id,
    RegistryCapabilityProfileV3Id, RegistryProgramReleaseV1Id,
    SeriesLinkObligationConfigurationV2, MARKET_FAMILIES_V1, MARKET_FAMILY_COUNT_V1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

const PRODUCT_MARKET_FAMILY_CAPABILITY_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-market-family-capability-authentication/v1\0";
const PRODUCT_MARKET_FAMILY_CAPABILITY_ARTIFACT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product-market-family-capability-artifact-authentication/v1\0";

struct ExactMarketFamilyInitializationV1 {
    policy_id: ContentId,
    binding_id: ContentId,
}

/// Move-only pre-generation authority over the hostile immutable family-policy
/// artifact and its exact current Registry/physical-founder joins.
///
/// This receipt deliberately contains no generation and derives no family
/// anchors. The persistent ProductReplayAnchor consumes it to derive and own
/// the initial generation before this receipt can be completed into the
/// generation-specific aggregator authority.
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
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn policy_id(&self) -> ContentId {
        self.policy.semantic_id()
    }

    pub(crate) const fn registry_release_id(&self) -> ContentId {
        self.registry_release_id
    }

    pub(crate) const fn capability_profile_id(&self) -> ContentId {
        self.capability_profile_id
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
    obligation_configuration: SeriesLinkObligationConfigurationV2,
    physical_founder_id: ContentId,
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

    pub(crate) const fn aggregator(&self) -> MarketFamilyAggregatorV1 {
        self.aggregator
    }

    pub(crate) const fn obligation_configuration(
        &self,
    ) -> SeriesLinkObligationConfigurationV2 {
        self.obligation_configuration
    }

    pub(crate) const fn physical_founder_id(&self) -> ContentId {
        self.physical_founder_id
    }
}

/// Hostile-authenticate the immutable family-policy artifact before a Market
/// generation exists.
#[inline(never)]
pub(crate) fn authenticate_market_family_capability_policy_artifact_v1(
    program_id: &Pubkey,
    physical: &AuthenticatedSeriesPhysicalFounderV4,
    registry: &AuthenticatedRegistryCapabilityV4,
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

/// Consume the pre-generation artifact authority and derive every canonical
/// family namespace anchor and initial Series-link obligation status.
#[inline(never)]
pub(crate) fn complete_market_family_capability_policy_v1(
    program_id: &Pubkey,
    artifact: AuthenticatedMarketFamilyCapabilityPolicyArtifactV1,
    market_instance_id: MarketInstanceV2Id,
    generation: u64,
) -> Outcome<AuthenticatedMarketFamilyCapabilityPolicyV1> {
    require(generation != 0, ClutchError::MismatchedState)?;
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
        .obligation_configuration(artifact.attachment_plan_id)
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
        obligation_configuration,
        physical_founder_id: artifact.physical_founder_id,
    })
}
