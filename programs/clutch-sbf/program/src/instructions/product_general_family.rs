// SPDX-License-Identifier: AGPL-3.0-or-later

//! Narrow Product authority for the current General Market-family owner.
//!
//! This module has no dispatch route. It authenticates the exact Product
//! Market root, founder Series link, current ProfileV4 compiler graph, and
//! MarketInstanceV2 before minting a private preauthorization. General must
//! persist that exact preauthorization in its successor account and return a
//! privately authenticated postwrite. Only then can Product admit the General
//! child and persist the `0xaa` successor.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use clutch_product_series::{
    AuthenticatedMarketFamilyAuthorityV1, CompiledProductSeriesBundleV5, ContentId,
    MarketFamilyAggregatorV1, MarketFamilyV1, MarketInstancePreimageV2, MarketInstanceV2Id,
    MarketLifecyclePhaseV1, RegistryCapabilityProfileV4, SeriesAttachmentPlanV4,
    SeriesMarketLinkPhaseV1, SeriesPlanV5, SeriesPlanV5Id,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesMarketLinkAccountV1,
};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use super::product_artifact::authenticate_product_artifact_v1;
use super::product_market::{
    authenticate_market_lifecycle_root_v1, authenticate_series_market_link_v1,
    write_market_lifecycle_root_v1, AuthenticatedMarketLifecycleRootV1,
    AuthenticatedSeriesMarketLinkV1,
};

const GENERAL_FAMILY_PREAUTHORIZATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product/general-family-preauthorization/v1\0";
const GENERAL_FAMILY_ADMISSION_PROJECTION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/product/general-family-admission-projection/v1\0";

/// Exact Product preauthorization General must persist before Product changes
/// its family counts. Private fields prevent payload coordinates from becoming
/// authority merely because they form a coherent set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedGeneralFamilyPreauthorizationV1 {
    program_id: Pubkey,
    market_lifecycle_root_account: Pubkey,
    market_lifecycle_root_pre_semantic_id: ContentId,
    market_lifecycle_root_authentication_id: ContentId,
    market_instance_v2_id: MarketInstanceV2Id,
    product_market_binding_id: ContentId,
    product_generation: u64,
    series_plan_v5_id: SeriesPlanV5Id,
    series_ordinal: u32,
    series_market_link_account: Pubkey,
    series_market_link_semantic_id: ContentId,
    series_market_link_authentication_id: ContentId,
    compiler_bundle_v5_id: ContentId,
    capability_profile_v4_id: ContentId,
    attachment_plan_v4_id: ContentId,
    general_market_owner_account: Pubkey,
    family_admission_sequence: u32,
    preauthorization_id: ContentId,
}

impl AuthenticatedGeneralFamilyPreauthorizationV1 {
    pub(crate) const fn program_id(self) -> Pubkey { self.program_id }
    pub(crate) const fn market_lifecycle_root_account(self) -> Pubkey {
        self.market_lifecycle_root_account
    }
    pub(crate) const fn market_lifecycle_root_pre_semantic_id(self) -> ContentId {
        self.market_lifecycle_root_pre_semantic_id
    }
    pub(crate) const fn market_lifecycle_root_authentication_id(self) -> ContentId {
        self.market_lifecycle_root_authentication_id
    }
    pub(crate) const fn market_instance_v2_id(self) -> MarketInstanceV2Id {
        self.market_instance_v2_id
    }
    pub(crate) const fn product_market_binding_id(self) -> ContentId {
        self.product_market_binding_id
    }
    pub(crate) const fn product_generation(self) -> u64 { self.product_generation }
    pub(crate) const fn series_plan_v5_id(self) -> SeriesPlanV5Id { self.series_plan_v5_id }
    pub(crate) const fn series_ordinal(self) -> u32 { self.series_ordinal }
    pub(crate) const fn series_market_link_account(self) -> Pubkey {
        self.series_market_link_account
    }
    pub(crate) const fn series_market_link_semantic_id(self) -> ContentId {
        self.series_market_link_semantic_id
    }
    pub(crate) const fn series_market_link_authentication_id(self) -> ContentId {
        self.series_market_link_authentication_id
    }
    pub(crate) const fn compiler_bundle_v5_id(self) -> ContentId {
        self.compiler_bundle_v5_id
    }
    pub(crate) const fn capability_profile_v4_id(self) -> ContentId {
        self.capability_profile_v4_id
    }
    pub(crate) const fn attachment_plan_v4_id(self) -> ContentId {
        self.attachment_plan_v4_id
    }
    pub(crate) const fn general_market_owner_account(self) -> Pubkey {
        self.general_market_owner_account
    }
    pub(crate) const fn family_admission_sequence(self) -> u32 {
        self.family_admission_sequence
    }
    pub(crate) const fn preauthorization_id(self) -> ContentId { self.preauthorization_id }
}

/// Private General-owned postwrite interface. Its implementation must expose
/// facts only after authenticating the exact General PDA, owner, version,
/// complete body, rent, and just-written Product preauthorization.
pub(crate) trait AuthenticatedGeneralMarketPostwriteV1 {
    fn account(&self) -> Pubkey;
    fn owner_program(&self) -> Pubkey;
    fn market_instance_v2_id(&self) -> MarketInstanceV2Id;
    fn product_generation(&self) -> u64;
    fn product_market_root_account(&self) -> Pubkey;
    fn product_market_root_pre_semantic_id(&self) -> ContentId;
    fn product_market_binding_id(&self) -> ContentId;
    fn series_plan_v5_id(&self) -> SeriesPlanV5Id;
    fn series_ordinal(&self) -> u32;
    fn series_market_link_account(&self) -> Pubkey;
    fn compiler_bundle_v5_id(&self) -> ContentId;
    fn attachment_plan_v4_id(&self) -> ContentId;
    fn product_preauthorization_id(&self) -> ContentId;
    fn semantic_id(&self) -> ContentId;
    fn data_id(&self) -> ContentId;
    fn authentication_id(&self) -> ContentId;
}

/// Final Product-owned projection after General's exact postwrite was consumed
/// and the General child count was persisted in `0xaa`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedGeneralFamilyAdmissionProjectionV1 {
    preauthorization: AuthenticatedGeneralFamilyPreauthorizationV1,
    general_postwrite_semantic_id: ContentId,
    general_postwrite_data_id: ContentId,
    general_postwrite_authentication_id: ContentId,
    market_lifecycle_root_post_semantic_id: ContentId,
    market_lifecycle_root_post_authentication_id: ContentId,
    family_admission_transition_id: ContentId,
    projection_id: ContentId,
}

impl AuthenticatedGeneralFamilyAdmissionProjectionV1 {
    pub(crate) const fn preauthorization(self) -> AuthenticatedGeneralFamilyPreauthorizationV1 {
        self.preauthorization
    }
    pub(crate) const fn general_postwrite_semantic_id(self) -> ContentId {
        self.general_postwrite_semantic_id
    }
    pub(crate) const fn general_postwrite_data_id(self) -> ContentId {
        self.general_postwrite_data_id
    }
    pub(crate) const fn general_postwrite_authentication_id(self) -> ContentId {
        self.general_postwrite_authentication_id
    }
    pub(crate) const fn market_lifecycle_root_post_semantic_id(self) -> ContentId {
        self.market_lifecycle_root_post_semantic_id
    }
    pub(crate) const fn market_lifecycle_root_post_authentication_id(self) -> ContentId {
        self.market_lifecycle_root_post_authentication_id
    }
    pub(crate) const fn family_admission_transition_id(self) -> ContentId {
        self.family_admission_transition_id
    }
    pub(crate) const fn projection_id(self) -> ContentId { self.projection_id }
}

fn hash_id(parts: &[&[u8]]) -> ContentId {
    ContentId::from_bytes(solana_sha256_hasher::hashv(parts).to_bytes())
}

fn authenticate_root_from_body<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    writable: bool,
    output: &'a mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedMarketLifecycleRootV1<'a>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    MarketLifecycleRootAccountV1::decode_into(&data, output)?;
    drop(data);
    let binding = output.state.binding();
    authenticate_market_lifecycle_root_v1(
        program_id,
        account,
        binding.market_instance_id,
        binding.generation,
        writable,
        output,
    )
}

fn authenticate_link_from_body<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    expected_root: Pubkey,
    writable: bool,
    output: &'a mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedSeriesMarketLinkV1<'a>> {
    let data = account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    SeriesMarketLinkAccountV1::decode_into(&data, output)?;
    drop(data);
    let binding = output.state.binding();
    authenticate_series_market_link_v1(
        program_id,
        account,
        binding.series_plan_id,
        binding.ordinal,
        binding.market_instance_id,
        binding.generation,
        expected_root,
        writable,
        output,
    )
}

/// Authenticate the exact Product graph and mint the only preauthorization
/// accepted by the current General Market owner.
#[allow(clippy::too_many_arguments)]
pub(crate) fn authenticate_general_family_preauthorization_v1(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    link_account: &AccountInfo<'_>,
    market_instance_account: &AccountInfo<'_>,
    series_plan_account: &AccountInfo<'_>,
    compiler_bundle_account: &AccountInfo<'_>,
    capability_profile_account: &AccountInfo<'_>,
    attachment_plan_account: &AccountInfo<'_>,
    root_output: &mut MarketLifecycleRootAccountV1,
    link_output: &mut SeriesMarketLinkAccountV1,
) -> Outcome<AuthenticatedGeneralFamilyPreauthorizationV1> {
    let root = authenticate_root_from_body(program_id, root_account, true, root_output)?;
    let link = authenticate_link_from_body(
        program_id,
        link_account,
        *root_account.key,
        false,
        link_output,
    )?;
    let root_binding = root.state().binding();
    let link_binding = link.state().binding();
    let link_semantic_id = link
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let market = authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        market_instance_account,
        link_binding.market_instance_id.content_id(),
    )?;
    let series = authenticate_product_artifact_v1::<SeriesPlanV5>(
        program_id,
        series_plan_account,
        link_binding.series_plan_id.content_id(),
    )?;
    let bundle = authenticate_product_artifact_v1::<CompiledProductSeriesBundleV5>(
        program_id,
        compiler_bundle_account,
        link_binding.compiler_output_id,
    )?;
    let profile = authenticate_product_artifact_v1::<RegistryCapabilityProfileV4>(
        program_id,
        capability_profile_account,
        link_binding.capability_profile_id,
    )?;
    let attachment = authenticate_product_artifact_v1::<SeriesAttachmentPlanV4>(
        program_id,
        attachment_plan_account,
        link_binding.attachment_plan_id,
    )?;
    let product_families = root.state().product_families();
    let general_slot = product_families.family(MarketFamilyV1::General);
    let general_account = seeds::general_v2_market_binding_pda(
        program_id,
        &root_binding.market_instance_id.bytes(),
    )
    .0;
    let product_market_binding_id = root_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let bundle_id = bundle
        .value()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let profile_id = profile
        .value()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let attachment_id = attachment
        .value()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require(
        root.state().phase() == MarketLifecyclePhaseV1::Founding
            && link.state().phase() == SeriesMarketLinkPhaseV1::PendingMarket
            && root.state().capital().founder_link_id().content_id() == link_semantic_id
            && root_binding.market_instance_id == link_binding.market_instance_id
            && root_binding.generation == link_binding.generation
            && product_market_binding_id == link_binding.market_binding_id
            && root_binding.capability_profile_id == profile_id
            && root_binding.capability_profile_id == link_binding.capability_profile_id
            && root_binding.registry_release_id
                == profile.value().registry_release_id().content_id()
            && root_binding.registry_release_id == bundle.value().registry_release_id
            && root_binding.product_template_id == bundle.value().product_template_id.content_id()
            && root_binding.native_claim_basis_id
                == bundle.value().native_claim_basis_id.content_id()
            && root_binding.recovery_policy_id
                == bundle.value().evidence_only_recovery_policy_id.content_id()
            && root_binding.price_measure_policy_id
                == bundle.value().price_measure_policy_id.content_id()
            && root_binding.market_genesis_profile_id
                == bundle.value().market_genesis_profile_id.content_id()
            && root_binding.source_release_id == bundle.value().source_release_manifest_id
            && root_binding.source_plane_contract_id == bundle.value().source_plane_contract_id
            && root_binding.source_spec_id == bundle.value().source_spec_id
            && link_binding.series_plan_id == bundle.value().series_plan_id
            && link_binding.funding_quote_id == bundle.value().funding_quote_id
            && link_binding.attachment_plan_id == bundle.value().attachment_plan_id.content_id()
            && link_binding.source_release_id == bundle.value().source_release_manifest_id
            && link_binding.source_plane_contract_id == bundle.value().source_plane_contract_id
            && link_binding.source_spec_id == bundle.value().source_spec_id
            && series.value().product_template_id == bundle.value().product_template_id
            && series.value().market_genesis_profile_id
                == bundle.value().market_genesis_profile_id
            && series.value().attachment_plan_id.bytes()
                == bundle.value().attachment_plan_id.bytes()
            && series
                .value()
                .start_bucket(link_binding.ordinal)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == market.value().start_bucket
            && series.value().market_collateral_cap == market.value().collateral_cap
            && market.value().product_template_id == bundle.value().product_template_id
            && market.value().market_genesis_profile_id
                == bundle.value().market_genesis_profile_id
            && attachment_id == bundle.value().attachment_plan_id.content_id()
            && attachment.value().funding_quote_id == bundle.value().funding_quote_id
            && product_families.admits_new_child(MarketFamilyV1::General)
            && product_families
                .binding()
                .family_root_id(MarketFamilyV1::General)
                .bytes()
                == general_account.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let family_admission_sequence = general_slot.counts().admitted;
    let root_pre_semantic_id = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let preauthorization_id = hash_id(&[
        GENERAL_FAMILY_PREAUTHORIZATION_DOMAIN_V1,
        program_id.as_ref(),
        root_account.key.as_ref(),
        &root_pre_semantic_id.bytes(),
        &root.authentication_id().bytes(),
        &root_binding.market_instance_id.bytes(),
        &product_market_binding_id.bytes(),
        &root_binding.generation.to_le_bytes(),
        &link_binding.series_plan_id.bytes(),
        &link_binding.ordinal.to_le_bytes(),
        link_account.key.as_ref(),
        &link_semantic_id.bytes(),
        &link.authentication_id().bytes(),
        &bundle_id.bytes(),
        &profile_id.bytes(),
        &attachment_id.bytes(),
        general_account.as_ref(),
        &family_admission_sequence.to_le_bytes(),
        &root_binding.general_founding_capability_id.bytes(),
    ]);
    preauthorization_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedGeneralFamilyPreauthorizationV1 {
        program_id: *program_id,
        market_lifecycle_root_account: *root_account.key,
        market_lifecycle_root_pre_semantic_id: root_pre_semantic_id,
        market_lifecycle_root_authentication_id: root.authentication_id(),
        market_instance_v2_id: root_binding.market_instance_id,
        product_market_binding_id,
        product_generation: root_binding.generation,
        series_plan_v5_id: link_binding.series_plan_id,
        series_ordinal: link_binding.ordinal,
        series_market_link_account: *link_account.key,
        series_market_link_semantic_id: link_semantic_id,
        series_market_link_authentication_id: link.authentication_id(),
        compiler_bundle_v5_id: bundle_id,
        capability_profile_v4_id: profile_id,
        attachment_plan_v4_id: attachment_id,
        general_market_owner_account: general_account,
        family_admission_sequence,
        preauthorization_id,
    })
}

fn require_matching_general_postwrite<P: AuthenticatedGeneralMarketPostwriteV1 + ?Sized>(
    preauthorization: AuthenticatedGeneralFamilyPreauthorizationV1,
    postwrite: &P,
) -> Outcome<()> {
    for identity in [postwrite.semantic_id(), postwrite.data_id(), postwrite.authentication_id()] {
        identity
            .validate()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    }
    require(
        postwrite.account() == preauthorization.general_market_owner_account
            && postwrite.owner_program() == preauthorization.program_id
            && postwrite.market_instance_v2_id() == preauthorization.market_instance_v2_id
            && postwrite.product_generation() == preauthorization.product_generation
            && postwrite.product_market_root_account()
                == preauthorization.market_lifecycle_root_account
            && postwrite.product_market_root_pre_semantic_id()
                == preauthorization.market_lifecycle_root_pre_semantic_id
            && postwrite.product_market_binding_id()
                == preauthorization.product_market_binding_id
            && postwrite.series_plan_v5_id() == preauthorization.series_plan_v5_id
            && postwrite.series_ordinal() == preauthorization.series_ordinal
            && postwrite.series_market_link_account()
                == preauthorization.series_market_link_account
            && postwrite.compiler_bundle_v5_id() == preauthorization.compiler_bundle_v5_id
            && postwrite.attachment_plan_v4_id() == preauthorization.attachment_plan_v4_id
            && postwrite.product_preauthorization_id() == preauthorization.preauthorization_id,
        ClutchError::MismatchedState,
    )
}

struct GeneralAdmissionAuthorityV1 {
    market_instance_v2_id: MarketInstanceV2Id,
    product_generation: u64,
    general_root_id: ContentId,
    family_admission_sequence: u32,
    admission_receipt_id: ContentId,
}

impl AuthenticatedMarketFamilyAuthorityV1 for GeneralAdmissionAuthorityV1 {
    fn authenticate_admission(
        &self,
        current: &MarketFamilyAggregatorV1,
        family: MarketFamilyV1,
        family_root_id: ContentId,
        family_admission_sequence: u32,
        admission_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        if family != MarketFamilyV1::General
            || current.binding().market_instance_id != self.market_instance_v2_id
            || current.binding().generation != self.product_generation
            || family_root_id != self.general_root_id
            || family_admission_sequence != self.family_admission_sequence
            || admission_receipt_id != self.admission_receipt_id
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }
}

/// Consume General's exact postwrite, persist Product's General-child
/// admission, and return the final cross-owner projection.
pub(crate) fn admit_authenticated_general_family_postwrite_v1<
    P: AuthenticatedGeneralMarketPostwriteV1 + ?Sized,
>(
    program_id: &Pubkey,
    root_account: &AccountInfo<'_>,
    preauthorization: AuthenticatedGeneralFamilyPreauthorizationV1,
    postwrite: &P,
    root_pre_output: &mut MarketLifecycleRootAccountV1,
    root_post_output: &mut MarketLifecycleRootAccountV1,
) -> Outcome<AuthenticatedGeneralFamilyAdmissionProjectionV1> {
    require_matching_general_postwrite(preauthorization, postwrite)?;
    let root = authenticate_root_from_body(program_id, root_account, true, root_pre_output)?;
    let root_pre_semantic_id = root
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        *program_id == preauthorization.program_id
            && *root_account.key == preauthorization.market_lifecycle_root_account
            && root_pre_semantic_id == preauthorization.market_lifecycle_root_pre_semantic_id
            && root.authentication_id()
                == preauthorization.market_lifecycle_root_authentication_id,
        ClutchError::MismatchedState,
    )?;
    let family_root_id = root
        .state()
        .product_families()
        .binding()
        .family_root_id(MarketFamilyV1::General);
    let authority = GeneralAdmissionAuthorityV1 {
        market_instance_v2_id: preauthorization.market_instance_v2_id,
        product_generation: preauthorization.product_generation,
        general_root_id: family_root_id,
        family_admission_sequence: preauthorization.family_admission_sequence,
        admission_receipt_id: postwrite.semantic_id(),
    };
    let successor = root
        .state()
        .admit_product_family_child(
            &authority,
            MarketFamilyV1::General,
            preauthorization.family_admission_sequence,
            postwrite.semantic_id(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rebound = write_market_lifecycle_root_v1(
        program_id,
        root_account,
        root,
        &successor,
        root_post_output,
    )?;
    let root_post_semantic_id = rebound
        .state()
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let family_admission_transition_id = rebound
        .state()
        .product_families()
        .family(MarketFamilyV1::General)
        .last_admission_transition_id();
    let projection_id = hash_id(&[
        GENERAL_FAMILY_ADMISSION_PROJECTION_DOMAIN_V1,
        &preauthorization.preauthorization_id.bytes(),
        &postwrite.semantic_id().bytes(),
        &postwrite.data_id().bytes(),
        &postwrite.authentication_id().bytes(),
        &root_pre_semantic_id.bytes(),
        &root_post_semantic_id.bytes(),
        &rebound.authentication_id().bytes(),
        &family_admission_transition_id.bytes(),
    ]);
    projection_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    Ok(AuthenticatedGeneralFamilyAdmissionProjectionV1 {
        preauthorization,
        general_postwrite_semantic_id: postwrite.semantic_id(),
        general_postwrite_data_id: postwrite.data_id(),
        general_postwrite_authentication_id: postwrite.authentication_id(),
        market_lifecycle_root_post_semantic_id: root_post_semantic_id,
        market_lifecycle_root_post_authentication_id: rebound.authentication_id(),
        family_admission_transition_id,
        projection_id,
    })
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    fn id(byte: u8) -> ContentId { ContentId::from_bytes([byte; 32]) }

    #[derive(Clone, Copy)]
    struct Postwrite {
        account: Pubkey,
        preauthorization: AuthenticatedGeneralFamilyPreauthorizationV1,
        semantic_id: ContentId,
        data_id: ContentId,
        authentication_id: ContentId,
    }

    impl AuthenticatedGeneralMarketPostwriteV1 for Postwrite {
        fn account(&self) -> Pubkey { self.account }
        fn owner_program(&self) -> Pubkey { self.preauthorization.program_id }
        fn market_instance_v2_id(&self) -> MarketInstanceV2Id {
            self.preauthorization.market_instance_v2_id
        }
        fn product_generation(&self) -> u64 { self.preauthorization.product_generation }
        fn product_market_root_account(&self) -> Pubkey {
            self.preauthorization.market_lifecycle_root_account
        }
        fn product_market_root_pre_semantic_id(&self) -> ContentId {
            self.preauthorization.market_lifecycle_root_pre_semantic_id
        }
        fn product_market_binding_id(&self) -> ContentId {
            self.preauthorization.product_market_binding_id
        }
        fn series_plan_v5_id(&self) -> SeriesPlanV5Id {
            self.preauthorization.series_plan_v5_id
        }
        fn series_ordinal(&self) -> u32 { self.preauthorization.series_ordinal }
        fn series_market_link_account(&self) -> Pubkey {
            self.preauthorization.series_market_link_account
        }
        fn compiler_bundle_v5_id(&self) -> ContentId {
            self.preauthorization.compiler_bundle_v5_id
        }
        fn attachment_plan_v4_id(&self) -> ContentId {
            self.preauthorization.attachment_plan_v4_id
        }
        fn product_preauthorization_id(&self) -> ContentId {
            self.preauthorization.preauthorization_id
        }
        fn semantic_id(&self) -> ContentId { self.semantic_id }
        fn data_id(&self) -> ContentId { self.data_id }
        fn authentication_id(&self) -> ContentId { self.authentication_id }
    }

    fn preauthorization() -> AuthenticatedGeneralFamilyPreauthorizationV1 {
        AuthenticatedGeneralFamilyPreauthorizationV1 {
            program_id: Pubkey::new_from_array([1; 32]),
            market_lifecycle_root_account: Pubkey::new_from_array([2; 32]),
            market_lifecycle_root_pre_semantic_id: id(3),
            market_lifecycle_root_authentication_id: id(4),
            market_instance_v2_id: MarketInstanceV2Id::from_bytes([5; 32]),
            product_market_binding_id: id(6),
            product_generation: 7,
            series_plan_v5_id: SeriesPlanV5Id::from_bytes([8; 32]),
            series_ordinal: 9,
            series_market_link_account: Pubkey::new_from_array([10; 32]),
            series_market_link_semantic_id: id(11),
            series_market_link_authentication_id: id(12),
            compiler_bundle_v5_id: id(13),
            capability_profile_v4_id: id(14),
            attachment_plan_v4_id: id(15),
            general_market_owner_account: Pubkey::new_from_array([16; 32]),
            family_admission_sequence: 0,
            preauthorization_id: id(17),
        }
    }

    #[test]
    fn postwrite_identity_swaps_refuse_before_product_root_mutation() {
        let preauthorization = preauthorization();
        let exact = Postwrite {
            account: preauthorization.general_market_owner_account,
            preauthorization,
            semantic_id: id(18),
            data_id: id(19),
            authentication_id: id(20),
        };
        assert!(require_matching_general_postwrite(preauthorization, &exact).is_ok());
        let mut swapped = exact;
        swapped.account = Pubkey::new_from_array([21; 32]);
        assert!(require_matching_general_postwrite(preauthorization, &swapped).is_err());
        let mut stale = exact;
        stale.preauthorization.preauthorization_id = id(22);
        assert!(require_matching_general_postwrite(preauthorization, &stale).is_err());
        let mut wrong_series = exact;
        wrong_series.preauthorization.series_ordinal = 10;
        assert!(require_matching_general_postwrite(preauthorization, &wrong_series).is_err());
    }
}
