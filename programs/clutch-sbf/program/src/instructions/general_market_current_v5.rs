//! Hostile authentication of the current General V5 market authority.
//!
//! This module is the sole reusable read boundary for current General family
//! consumers.  It joins exact `0x79/v5` and RuntimeV3 bytes to the live Product
//! RootV3, founder LinkV3, RegistryV4/loader capability, FundingV5, the
//! reconstructed BundleV7 artifact graph, and the Realm-owned RevenuePolicyV2.
//! Historical General V4 and Product RootV2/LinkV2 bodies are never projected
//! into this authority.

use std::boxed::Box;

use clutch_batch_policy_identity::revenue_policy_v2::REVENUE_POLICY_V2_BYTES;
use clutch_general_v2_contract::{
    MarketBindingV5, MarketRuntimeV3AccountV1, MARKET_BINDING_ACCOUNT_BYTES_V5,
    MARKET_RUNTIME_ACCOUNT_BYTES,
};
use clutch_product_series::{
    ContentId, MarketFoundationScheduleV4, MarketGenesisProfileV2, MarketInstancePreimageV2,
    MarketLifecyclePhaseV3, SeriesMarketLinkPhaseV3, SeriesFundingPhaseV5, SeriesPlanV5Id,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV3, SeriesMarketLinkAccountV3,
};
use clutch_solana_layout::Hash32;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::seeds;
use crate::source_plane_v3::authenticate_release;

use super::product_artifact::authenticate_product_artifact_v1;
use super::product_market_lifecycle_v3_current::{
    authenticate_market_lifecycle_root_v3, authenticate_series_market_link_v3,
};
use super::product_series_current::{
    authenticate_registry_capability_v5, authenticate_series_funding_account_v5,
    authenticate_series_registry_account_v4,
};
use super::product_source_current::{
    authenticate_compiled_product_series_bundle_v7, authenticate_series_source_artifacts_v6,
};
use super::revenue_policy_v2::{
    authenticate_revenue_policy_record_v2, derive_revenue_market_treasury_v1,
    AuthenticatedRevenuePolicyRecordV2, RevenueMarketTreasuryDerivationV1,
};

const GENERAL_MARKET_BINDING_DATA_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/general-market-binding/data/v5\0";
const GENERAL_MARKET_RUNTIME_DATA_DOMAIN_V3: &[u8] =
    b"dragons-clutch/sbf/general-market-runtime/data/v3\0";
const GENERAL_MARKET_CURRENT_AUTHENTICATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/general-market-current-authentication/v5\0";

/// Nine-account order consumed by `authenticate_series_source_artifacts_v6`:
/// SeriesPlanV5, FundingTermsV2, TemplateV4, NativeBasisV1, RecoveryPolicyV1,
/// PricePolicyV1, GenesisV2, QuoteV6, AttachmentV6.
pub(crate) const GENERAL_MARKET_CURRENT_ARTIFACT_ACCOUNT_COUNT_V5: usize = 9;
/// Fixed accounts outside the nine-account artifact suffix.
pub(crate) const GENERAL_MARKET_CURRENT_FIXED_ACCOUNT_COUNT_V5: usize = 16;
/// Complete hostile read frame.
pub(crate) const GENERAL_MARKET_CURRENT_ACCOUNT_COUNT_V5: usize =
    GENERAL_MARKET_CURRENT_FIXED_ACCOUNT_COUNT_V5
        + GENERAL_MARKET_CURRENT_ARTIFACT_ACCOUNT_COUNT_V5;

/// Named account frame.  It carries physical accounts only; no caller-created
/// semantic ID, generation, ordinal, or graph projection is accepted.
pub(crate) struct GeneralMarketCurrentAccountFrameV5<'frame, 'info> {
    pub(crate) market_binding: &'frame AccountInfo<'info>,
    pub(crate) market_runtime: &'frame AccountInfo<'info>,
    pub(crate) product_root: &'frame AccountInfo<'info>,
    pub(crate) series_link: &'frame AccountInfo<'info>,
    pub(crate) series_funding: &'frame AccountInfo<'info>,
    pub(crate) series_registry: &'frame AccountInfo<'info>,
    pub(crate) registry_program: &'frame AccountInfo<'info>,
    pub(crate) registry_programdata: &'frame AccountInfo<'info>,
    pub(crate) registry_release_artifact: &'frame AccountInfo<'info>,
    pub(crate) capability_profile_artifact: &'frame AccountInfo<'info>,
    pub(crate) source_release: &'frame AccountInfo<'info>,
    pub(crate) compiler_bundle: &'frame AccountInfo<'info>,
    pub(crate) market_instance: &'frame AccountInfo<'info>,
    pub(crate) realm: &'frame AccountInfo<'info>,
    pub(crate) revenue_record: &'frame AccountInfo<'info>,
    pub(crate) revenue_policy_preimage: &'frame AccountInfo<'info>,
    pub(crate) artifacts: &'frame [AccountInfo<'info>],
}

/// Compact move-only authority returned after every physical account and
/// immutable semantic edge has been authenticated.  Large Product account
/// bodies remain in caller-provided buffers and are not copied into this
/// receipt.
#[derive(Debug)]
pub(crate) struct AuthenticatedGeneralMarketCurrentV5 {
    id: ContentId,
    binding_account: Pubkey,
    binding: Box<MarketBindingV5>,
    binding_data_id: ContentId,
    runtime_account: Pubkey,
    runtime: MarketRuntimeV3AccountV1,
    runtime_data_id: ContentId,
    market_instance_account: Pubkey,
    market_instance: MarketInstancePreimageV2,
    market_genesis_account: Pubkey,
    market_genesis: MarketGenesisProfileV2,
    product_root_account: Pubkey,
    product_root_binding_id: ContentId,
    product_root_generation: u64,
    product_root_outcome_count: u8,
    product_root_realm_id: ContentId,
    product_root_collateral_policy_id: ContentId,
    product_root_collateral_release_id: ContentId,
    product_root_registry_release_id: ContentId,
    product_root_data_id: ContentId,
    product_root_semantic_id: ContentId,
    product_root_authentication_id: ContentId,
    product_root_phase: MarketLifecyclePhaseV3,
    product_link_account: Pubkey,
    product_link_binding_id: ContentId,
    product_link_data_id: ContentId,
    product_link_semantic_id: ContentId,
    product_link_authentication_id: ContentId,
    product_link_phase: SeriesMarketLinkPhaseV3,
    funding_account: Pubkey,
    funding_state_id: ContentId,
    funding_data_id: ContentId,
    funding_authentication_id: ContentId,
    funding_phase: SeriesFundingPhaseV5,
    registry_account: Pubkey,
    registry_authentication_id: ContentId,
    registry_capability_id: ContentId,
    source_release_account: Pubkey,
    source_release_manifest_id: ContentId,
    source_release_authentication_id: ContentId,
    compiler_bundle_account: Pubkey,
    compiler_bundle_id: ContentId,
    collateral_profile_id: ContentId,
    foundation_schedule: MarketFoundationScheduleV4,
    realm_account: Pubkey,
    revenue: AuthenticatedRevenuePolicyRecordV2,
    treasury: RevenueMarketTreasuryDerivationV1,
}

impl AuthenticatedGeneralMarketCurrentV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn binding_account(&self) -> Pubkey { self.binding_account }
    pub(crate) fn binding(&self) -> &MarketBindingV5 { self.binding.as_ref() }
    pub(crate) const fn binding_data_id(&self) -> ContentId { self.binding_data_id }
    pub(crate) const fn runtime_account(&self) -> Pubkey { self.runtime_account }
    pub(crate) const fn runtime(&self) -> &MarketRuntimeV3AccountV1 { &self.runtime }
    pub(crate) const fn runtime_data_id(&self) -> ContentId { self.runtime_data_id }
    pub(crate) const fn market_instance(&self) -> &MarketInstancePreimageV2 {
        &self.market_instance
    }
    pub(crate) const fn market_instance_account(&self) -> Pubkey {
        self.market_instance_account
    }
    pub(crate) const fn market_genesis(&self) -> &MarketGenesisProfileV2 {
        &self.market_genesis
    }
    pub(crate) const fn market_genesis_account(&self) -> Pubkey {
        self.market_genesis_account
    }
    pub(crate) const fn product_root_account(&self) -> Pubkey { self.product_root_account }
    pub(crate) const fn product_root_binding_id(&self) -> ContentId {
        self.product_root_binding_id
    }
    pub(crate) const fn product_root_generation(&self) -> u64 {
        self.product_root_generation
    }
    pub(crate) const fn product_root_outcome_count(&self) -> u8 {
        self.product_root_outcome_count
    }
    pub(crate) const fn product_root_realm_id(&self) -> ContentId {
        self.product_root_realm_id
    }
    pub(crate) const fn product_root_collateral_policy_id(&self) -> ContentId {
        self.product_root_collateral_policy_id
    }
    pub(crate) const fn product_root_collateral_release_id(&self) -> ContentId {
        self.product_root_collateral_release_id
    }
    pub(crate) const fn product_root_registry_release_id(&self) -> ContentId {
        self.product_root_registry_release_id
    }
    pub(crate) const fn product_root_data_id(&self) -> ContentId {
        self.product_root_data_id
    }
    pub(crate) const fn product_root_semantic_id(&self) -> ContentId {
        self.product_root_semantic_id
    }
    pub(crate) const fn product_root_authentication_id(&self) -> ContentId {
        self.product_root_authentication_id
    }
    pub(crate) const fn product_root_phase(&self) -> MarketLifecyclePhaseV3 {
        self.product_root_phase
    }
    pub(crate) const fn product_link_account(&self) -> Pubkey { self.product_link_account }
    pub(crate) const fn product_link_binding_id(&self) -> ContentId {
        self.product_link_binding_id
    }
    pub(crate) const fn product_link_data_id(&self) -> ContentId { self.product_link_data_id }
    pub(crate) const fn product_link_semantic_id(&self) -> ContentId {
        self.product_link_semantic_id
    }
    pub(crate) const fn product_link_authentication_id(&self) -> ContentId {
        self.product_link_authentication_id
    }
    pub(crate) const fn product_link_phase(&self) -> SeriesMarketLinkPhaseV3 {
        self.product_link_phase
    }
    pub(crate) const fn funding_account(&self) -> Pubkey { self.funding_account }
    pub(crate) const fn funding_state_id(&self) -> ContentId { self.funding_state_id }
    pub(crate) const fn funding_data_id(&self) -> ContentId { self.funding_data_id }
    pub(crate) const fn funding_authentication_id(&self) -> ContentId {
        self.funding_authentication_id
    }
    pub(crate) const fn funding_phase(&self) -> SeriesFundingPhaseV5 { self.funding_phase }
    pub(crate) const fn registry_account(&self) -> Pubkey { self.registry_account }
    pub(crate) const fn registry_authentication_id(&self) -> ContentId {
        self.registry_authentication_id
    }
    pub(crate) const fn registry_capability_id(&self) -> ContentId {
        self.registry_capability_id
    }
    pub(crate) const fn source_release_account(&self) -> Pubkey {
        self.source_release_account
    }
    pub(crate) const fn source_release_manifest_id(&self) -> ContentId {
        self.source_release_manifest_id
    }
    pub(crate) const fn source_release_authentication_id(&self) -> ContentId {
        self.source_release_authentication_id
    }
    pub(crate) const fn compiler_bundle_account(&self) -> Pubkey {
        self.compiler_bundle_account
    }
    pub(crate) const fn compiler_bundle_id(&self) -> ContentId { self.compiler_bundle_id }
    pub(crate) const fn collateral_profile_id(&self) -> ContentId {
        self.collateral_profile_id
    }
    pub(crate) const fn foundation_schedule(&self) -> MarketFoundationScheduleV4 {
        self.foundation_schedule
    }
    pub(crate) const fn realm_account(&self) -> Pubkey { self.realm_account }
    pub(crate) const fn revenue(&self) -> AuthenticatedRevenuePolicyRecordV2 { self.revenue }
    pub(crate) const fn treasury(&self) -> RevenueMarketTreasuryDerivationV1 { self.treasury }
}

fn require_live(id: ContentId) -> Outcome<()> {
    require(!id.is_zero(), ClutchError::MismatchedState)
}

fn hash_account_data(domain: &[u8], account: &AccountInfo<'_>, data: &[u8]) -> ContentId {
    ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[domain, account.key.as_ref(), data]).to_bytes(),
    )
}

fn require_exact_readonly_account(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    exact_len: usize,
) -> Outcome<()> {
    require(
        account.owner == program_id
            && !account.is_signer
            && !account.is_writable
            && !account.executable
            && account.data_len() == exact_len,
        ClutchError::MismatchedState,
    )
}

fn require_distinct_frame(frame: &GeneralMarketCurrentAccountFrameV5<'_, '_>) -> Outcome<()> {
    require(
        frame.artifacts.len() == GENERAL_MARKET_CURRENT_ARTIFACT_ACCOUNT_COUNT_V5,
        ClutchError::AccountCount,
    )?;
    let fixed = [
        frame.market_binding,
        frame.market_runtime,
        frame.product_root,
        frame.series_link,
        frame.series_funding,
        frame.series_registry,
        frame.registry_program,
        frame.registry_programdata,
        frame.registry_release_artifact,
        frame.capability_profile_artifact,
        frame.source_release,
        frame.compiler_bundle,
        frame.market_instance,
        frame.realm,
        frame.revenue_record,
        frame.revenue_policy_preimage,
    ];
    let mut left = 0usize;
    while left < fixed.len() {
        let mut right = left + 1;
        while right < fixed.len() {
            require(fixed[left].key != fixed[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        for artifact in frame.artifacts {
            require(fixed[left].key != artifact.key, ClutchError::AccountAlias)?;
        }
        left += 1;
    }
    left = 0;
    while left < frame.artifacts.len() {
        let mut right = left + 1;
        while right < frame.artifacts.len() {
            require(
                frame.artifacts[left].key != frame.artifacts[right].key,
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    Ok(())
}

/// Authenticate the complete current General/Product/Revenue read graph.
///
/// RootV3 and LinkV3 decode into caller-provided storage, keeping their large
/// fixed bodies off this callee's stack.  The returned receipt retains only
/// the exact General bodies and compact authentication identities.
///
/// The physical-founder and Product preauthorization identities are immutable
/// lineage facts whose sole persisted owner is the hostile-decoded BindingV5.
/// They are committed into the returned authentication ID but are never
/// reconstructed from a caller-supplied facts object.
#[allow(clippy::too_many_lines)]
#[inline(never)]
pub(crate) fn authenticate_general_market_current_v5(
    program_id: &Pubkey,
    frame: &GeneralMarketCurrentAccountFrameV5<'_, '_>,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedGeneralMarketCurrentV5> {
    authenticate_general_market_current_v5_with_product_access(
        program_id,
        frame,
        root_output,
        link_output,
        false,
        false,
    )
}

/// Same hostile current-market join for a terminal outer which will mutate
/// Product RootV3/LinkV3 later in the same instruction. No mutation occurs in
/// this authentication boundary; the exact writable privilege is only
/// admitted so the returned receipt can precede Product terminalization.
pub(crate) fn authenticate_general_market_current_v5_for_terminal(
    program_id: &Pubkey,
    frame: &GeneralMarketCurrentAccountFrameV5<'_, '_>,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedGeneralMarketCurrentV5> {
    authenticate_general_market_current_v5_with_product_access(
        program_id,
        frame,
        root_output,
        link_output,
        true,
        true,
    )
}

/// Same exact writable Product-state join for Direct's atomic family and
/// founder-Link activation. It remains separately named so the action-1
/// account contract cannot be confused with terminal mutation authority.
pub(crate) fn authenticate_general_market_current_for_product_activation_v5(
    program_id: &Pubkey,
    frame: &GeneralMarketCurrentAccountFrameV5<'_, '_>,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedGeneralMarketCurrentV5> {
    authenticate_general_market_current_v5_with_product_access(
        program_id,
        frame,
        root_output,
        link_output,
        true,
        true,
    )
}

/// Authenticate the complete current graph while admitting only the Product
/// Root write used by Position retirement. The Series Link remains read-only;
/// its later Product terminal mutation has a separately named authority.
pub(crate) fn authenticate_general_market_current_v5_with_root_access(
    program_id: &Pubkey,
    frame: &GeneralMarketCurrentAccountFrameV5<'_, '_>,
    product_root_writable: bool,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
) -> Outcome<AuthenticatedGeneralMarketCurrentV5> {
    authenticate_general_market_current_v5_with_product_access(
        program_id,
        frame,
        root_output,
        link_output,
        product_root_writable,
        false,
    )
}

#[allow(clippy::too_many_lines)]
#[inline(never)]
fn authenticate_general_market_current_v5_with_product_access(
    program_id: &Pubkey,
    frame: &GeneralMarketCurrentAccountFrameV5<'_, '_>,
    root_output: &mut MarketLifecycleRootAccountV3,
    link_output: &mut SeriesMarketLinkAccountV3,
    product_root_writable: bool,
    series_link_writable: bool,
) -> Outcome<AuthenticatedGeneralMarketCurrentV5> {
    require_distinct_frame(frame)?;
    require_exact_readonly_account(
        program_id,
        frame.market_binding,
        MARKET_BINDING_ACCOUNT_BYTES_V5,
    )?;
    require_exact_readonly_account(
        program_id,
        frame.market_runtime,
        MARKET_RUNTIME_ACCOUNT_BYTES,
    )?;
    let binding_data = frame
        .market_binding
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let binding = Box::new(MarketBindingV5::decode(&binding_data)?);
    let binding_data_id = hash_account_data(
        GENERAL_MARKET_BINDING_DATA_DOMAIN_V5,
        frame.market_binding,
        &binding_data,
    );
    drop(binding_data);
    let relation = binding.base().base();
    expect_pda(
        frame.market_binding.key,
        seeds::general_v2_market_binding_pda(
            program_id,
            &relation.market_instance_v2_id.bytes(),
        ),
        Some(relation.stored_bump),
    )?;

    let runtime_data = frame
        .market_runtime
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let runtime = MarketRuntimeV3AccountV1::decode(&runtime_data)?;
    let runtime_data_id = hash_account_data(
        GENERAL_MARKET_RUNTIME_DATA_DOMAIN_V3,
        frame.market_runtime,
        &runtime_data,
    );
    drop(runtime_data);
    expect_pda(
        frame.market_runtime.key,
        seeds::general_v2_market_runtime_pda(program_id, &frame.market_binding.key.to_bytes()),
        Some(runtime.stored_bump),
    )?;
    let binding_rent = binding.rent();
    let binding_floor = binding_rent
        .refundable_principal
        .checked_add(binding_rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let runtime_floor = runtime
        .rent
        .refundable_principal
        .checked_add(runtime.rent.donation_floor)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        relation.market.bytes() == frame.market_runtime.key.to_bytes()
            && runtime.market_binding.bytes() == frame.market_binding.key.to_bytes()
            && runtime.market_instance_v2_id == relation.market_instance_v2_id
            && frame.market_binding.lamports() >= binding_floor
            && frame.market_runtime.lamports() >= runtime_floor,
        ClutchError::MismatchedState,
    )?;

    let authority = binding.authority();
    let market_instance_id = clutch_product_series::MarketInstanceV2Id::from_bytes(
        relation.market_instance_v2_id.bytes(),
    );
    let series_plan_id = SeriesPlanV5Id::from_bytes(relation.series_plan_v5_id.bytes());
    let root = authenticate_market_lifecycle_root_v3(
        program_id,
        frame.product_root,
        market_instance_id,
        authority.product_generation(),
        product_root_writable,
        root_output,
    )?;
    let link = authenticate_series_market_link_v3(
        program_id,
        frame.series_link,
        series_plan_id,
        authority.series_ordinal(),
        market_instance_id,
        authority.product_generation(),
        *frame.product_root.key,
        series_link_writable,
        link_output,
    )?;
    let root_binding = root.binding();
    let link_binding = link.binding();

    let registry_account = authenticate_series_registry_account_v4(
        program_id,
        frame.series_registry,
        series_plan_id,
        false,
    )?;
    let registry = authenticate_registry_capability_v5(
        program_id,
        registry_account,
        frame.registry_program,
        frame.registry_programdata,
        frame.registry_release_artifact,
        frame.capability_profile_artifact,
    )?;
    let funding = authenticate_series_funding_account_v5(
        program_id,
        frame.series_funding,
        series_plan_id,
        product_state_writable,
    )?;
    let source_release = authenticate_release(program_id, frame.source_release)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source_release_account = Pubkey::new_from_array(source_release.account().bytes());
    let source_release_manifest_id = ContentId::from_bytes(source_release.manifest_id().bytes());
    let source_release_authentication_id = ContentId::from_bytes(source_release.id().bytes());
    let artifacts = authenticate_series_source_artifacts_v6(
        program_id,
        frame.artifacts,
        series_plan_id,
        clutch_product_series::SeriesFundingTermsV2Id::from_bytes(
            relation.series_funding_terms_v2_id.bytes(),
        ),
    )?;
    let bundle = authenticate_compiled_product_series_bundle_v7(
        program_id,
        frame.compiler_bundle,
        &registry,
        source_release,
        &artifacts,
    )?;
    let market_instance = *authenticate_product_artifact_v1::<MarketInstancePreimageV2>(
        program_id,
        frame.market_instance,
        market_instance_id.content_id(),
    )?
    .value();
    market_instance
        .validate_bindings(
            artifacts.template(),
            artifacts.basis(),
            artifacts.price_policy(),
            artifacts.genesis(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    funding
        .state()
        .validate_against(artifacts.series(), artifacts.quote(), artifacts.attachment())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    require(
        !frame.revenue_policy_preimage.is_signer
            && !frame.revenue_policy_preimage.is_writable
            && !frame.revenue_policy_preimage.executable
            && frame.revenue_policy_preimage.data_len() == REVENUE_POLICY_V2_BYTES,
        ClutchError::MismatchedState,
    )?;
    let policy_data = frame
        .revenue_policy_preimage
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let revenue = authenticate_revenue_policy_record_v2(
        program_id,
        frame.realm,
        frame.revenue_record,
        &policy_data,
    )?;
    drop(policy_data);
    let treasury = derive_revenue_market_treasury_v1(
        program_id,
        revenue,
        Hash32::from_bytes(relation.market_instance_v2_id.bytes()),
        *frame.market_runtime.key,
    )?;

    let bundle_value = bundle.bundle();
    let bundle_id = bundle.bundle_id().content_id();
    let series_id = artifacts
        .series()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let template_id = artifacts
        .template()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let basis_id = artifacts
        .basis()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recovery_id = artifacts
        .recovery()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let price_policy_id = artifacts
        .price_policy()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let genesis_id = artifacts
        .genesis()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let quote_id = artifacts
        .quote()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_id = artifacts
        .attachment()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let schedule_id = artifacts
        .quote()
        .foundation
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_binding_id = root.binding_id();
    let funding_state_id = funding
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let registry_projection = registry.projection();
    let registry_collateral = registry.realm_collateral();
    let registry_semantic = registry.semantic_owners();

    require(
        authority.product_market_root_account().bytes() == frame.product_root.key.to_bytes()
            && authority.product_market_binding_v3_id().bytes() == root_binding_id.bytes()
            && authority.product_generation() == root_binding.generation
            && authority.series_market_link_account().bytes() == frame.series_link.key.to_bytes()
            && authority.series_market_link_v3_id().bytes() == link.semantic_id().bytes()
            && authority.series_ordinal() == link_binding.ordinal
            && authority.compiler_bundle_v7_id().bytes() == bundle_id.bytes()
            && authority.funding_quote_v6_id().bytes() == quote_id.bytes()
            && authority.attachment_plan_v6_id().bytes() == attachment_id.bytes()
            && authority.foundation_schedule_v4_id().bytes() == schedule_id.bytes()
            && authority.foundation_account_graph_v4_id().bytes()
                == root_binding.foundation_account_graph_id.bytes()
            && authority.series_funding_v5_account().bytes()
                == frame.series_funding.key.to_bytes()
            && authority.market_liability_founding_id().bytes()
                == root_binding.market_liability_founding_id.bytes()
            && authority.claim_mint_founding_plan_id().bytes()
                == root_binding.claim_mint_founding_plan_id.bytes()
            && authority.claim_issuance_binding_id().bytes()
                == root_binding.claim_issuance_binding_id.bytes()
            && authority.general_founding_capability_id().bytes()
                == root_binding.general_founding_capability_id.bytes()
            && authority.revenue_policy_record_account().bytes()
                == frame.revenue_record.key.to_bytes()
            && authority.revenue_policy_record_v2_id().bytes()
                == revenue.record_semantic_id().bytes()
            && authority.revenue_policy_v2_digest().bytes() == revenue.policy_digest().bytes()
            && authority.treasury_owner().bytes() == revenue.treasury_owner().bytes()
            && authority.treasury_position_derivation_policy_v2_id().bytes()
                == revenue.treasury_position_derivation_policy_id().bytes()
            && authority.treasury_position_account().bytes()
                == treasury.treasury_position_account().to_bytes()
            && authority.treasury_service_ledger_account().bytes()
                == treasury.treasury_service_ledger_account().to_bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        root.account() == *frame.product_root.key
            && root_binding.market_instance_id == market_instance_id
            && root_binding.outcome_count == relation.outcome_count
            && root_binding.product_template_id == template_id.content_id()
            && root_binding.native_claim_basis_id == basis_id.content_id()
            && root_binding.recovery_policy_id == recovery_id.content_id()
            && root_binding.price_measure_policy_id == price_policy_id.content_id()
            && root_binding.market_genesis_profile_id == genesis_id.content_id()
            && root_binding.registry_release_id == registry.registry_release_id()
            && root_binding.capability_profile_id == registry.capability_profile_id()
            && root_binding.realm_id == ContentId::from_bytes(revenue.realm().bytes())
            && root_binding.realm_id == registry_collateral.realm_id
            && root_binding.collateral_profile_id == registry_collateral.profile_id
            && root_binding.source_release_id == source_release_authentication_id
            && root_binding.source_plane_contract_id == bundle_value.source_plane_contract_id
            && root_binding.source_spec_id == bundle_value.source_spec_id
            && root_binding.foundation_schedule_id == schedule_id
            && root_binding.foundation_account_graph_id.bytes()
                == authority.foundation_account_graph_v4_id().bytes()
            && root_binding.failure_liveness_policy_id
                == artifacts.quote().failure_liveness_policy_id
            && root_binding.failure_liveness_quote_schedule_id
                == artifacts.quote().failure_recovery_quote_schedule_id
            && root.state().capital().neutral_lamport_sink.bytes()
                == relation.neutral_sink.bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        link.account() == *frame.series_link.key
            && link_binding.series_plan_id == series_id
            && link_binding.market_instance_id == market_instance_id
            && link_binding.market_root_account_id.bytes() == frame.product_root.key.to_bytes()
            && link_binding.market_binding_id == root_binding_id
            && link_binding.generation == root_binding.generation
            && link_binding.funding_terms_id == funding_terms_id
            && link_binding.funding_quote_id == quote_id
            && link_binding.attachment_plan_id == attachment_id
            && link_binding.capability_profile_id == registry.capability_profile_id()
            && link_binding.compiler_bundle_id.content_id() == bundle_id
            && link_binding.source_release_id == source_release_authentication_id
            && link_binding.source_plane_contract_id == bundle_value.source_plane_contract_id
            && link_binding.source_spec_id == bundle_value.source_spec_id
            && link_binding.funding_state_account_id.bytes()
                == frame.series_funding.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        registry.activation_consumed()
            && registry.series_plan_id() == series_id
            && registry.funding_terms_id() == funding_terms_id
            && registry.compiler_bundle_id().content_id() == bundle_id
            && registry_projection.registry_release_id == bundle_value.registry_release_id
            && registry_projection.capability_profile_id
                == bundle_value.capability_profile_id.content_id()
            && registry_semantic.source_plane_contract_id
                == bundle_value.source_plane_contract_id
            && registry_semantic.source_spec_id == bundle_value.source_spec_id
            && registry_semantic.summary_program_id == bundle_value.summary_program_id
            && registry_semantic.product_compiler_release_id
                == bundle_value.product_compiler_release_id
            && registry_semantic.native_claim_basis_id.content_id()
                == bundle_value.native_claim_basis_id.content_id()
            && registry_semantic.evidence_only_recovery_policy_id.content_id()
                == bundle_value.evidence_only_recovery_policy_id.content_id()
            && registry_semantic.price_measure_policy_id.content_id()
                == bundle_value.price_measure_policy_id.content_id()
            && registry_collateral.neutral_lamport_sink.bytes() == relation.neutral_sink.bytes(),
        ClutchError::MismatchedState,
    )?;
    require(
        market_instance
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == market_instance_id
            && market_instance.product_template_id == template_id
            && market_instance.market_genesis_profile_id == genesis_id
            && relation.market_genesis_profile_v2_id.bytes() == genesis_id.bytes()
            && relation.series_plan_v5_id.bytes() == series_id.bytes()
            && relation.series_funding_terms_v2_id.bytes() == funding_terms_id.bytes()
            && relation.price_measure_policy_v1_id.bytes() == price_policy_id.bytes()
            && relation.native_claim_basis_id.bytes() == basis_id.bytes()
            && relation.relation_policy_id.bytes()
                == artifacts.genesis().relation_policy_id.bytes()
            && artifacts.genesis().realm_id.bytes() == revenue.realm().bytes()
            && artifacts.genesis().profile_id == registry_collateral.profile_id
            && artifacts.genesis().capability_profile_id
                == registry.capability_profile_id()
            && artifacts.genesis().fee_policy_id.bytes() == revenue.policy_digest().bytes()
            && artifacts.quote().foundation.outcome_count == relation.outcome_count,
        ClutchError::MismatchedState,
    )?;
    require(
        funding.account() == *frame.series_funding.key
            && funding.state().series_plan_id == series_id
            && funding.state().funding_terms_id == funding_terms_id
            && funding.state().funding_quote_id == quote_id
            && funding.state().attachment_plan_id == attachment_id
            && funding.state().compiler_bundle_id.content_id() == bundle_id,
        ClutchError::MismatchedState,
    )?;

    let root_data_id = root.data_id();
    let root_semantic_id = root.semantic_id();
    let root_authentication_id = root.authentication_id();
    let root_phase = root.state().phase();
    let link_binding_id = link.binding_id();
    let link_data_id = link.data_id();
    let link_semantic_id = link.semantic_id().content_id();
    let link_authentication_id = link.authentication_id();
    let link_phase = link.state().phase();
    let funding_data_id = funding.data_id();
    let funding_authentication_id = funding.authentication_id();
    let funding_phase = funding.state().phase;
    let registry_account_key = registry.series_registry_account();
    let registry_authentication_id = registry.series_registry_authentication_id();
    let registry_capability_id = registry.id();
    let compiler_bundle_account = bundle.artifact_account();
    let revenue_record_id = ContentId::from_bytes(revenue.record_semantic_id().bytes());
    let revenue_digest = ContentId::from_bytes(revenue.policy_digest().bytes());
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            GENERAL_MARKET_CURRENT_AUTHENTICATION_DOMAIN_V5,
            program_id.as_ref(),
            frame.market_binding.key.as_ref(),
            &binding_data_id.bytes(),
            frame.market_runtime.key.as_ref(),
            &runtime_data_id.bytes(),
            frame.product_root.key.as_ref(),
            &root_binding_id.bytes(),
            &root_data_id.bytes(),
            &root_semantic_id.bytes(),
            &root_authentication_id.bytes(),
            frame.series_link.key.as_ref(),
            &link_binding_id.bytes(),
            &link_data_id.bytes(),
            &link_semantic_id.bytes(),
            &link_authentication_id.bytes(),
            frame.series_funding.key.as_ref(),
            &funding_state_id.bytes(),
            &funding_data_id.bytes(),
            &funding_authentication_id.bytes(),
            registry_account_key.as_ref(),
            &registry_authentication_id.bytes(),
            &registry_capability_id.bytes(),
            source_release_account.as_ref(),
            &source_release_manifest_id.bytes(),
            &source_release_authentication_id.bytes(),
            compiler_bundle_account.as_ref(),
            &bundle_id.bytes(),
            frame.revenue_record.key.as_ref(),
            &revenue_record_id.bytes(),
            &revenue_digest.bytes(),
            &authority.series_physical_founder_v5_id().bytes(),
            &authority.product_preauthorization_id().bytes(),
        ])
        .to_bytes(),
    );
    require_live(id)?;

    Ok(AuthenticatedGeneralMarketCurrentV5 {
        id,
        binding_account: *frame.market_binding.key,
        binding,
        binding_data_id,
        runtime_account: *frame.market_runtime.key,
        runtime,
        runtime_data_id,
        market_instance_account: *frame.market_instance.key,
        market_instance,
        market_genesis_account: *frame.artifacts[6].key,
        market_genesis: *artifacts.genesis(),
        product_root_account: *frame.product_root.key,
        product_root_binding_id: root_binding_id,
        product_root_generation: root_binding.generation,
        product_root_outcome_count: root_binding.outcome_count,
        product_root_realm_id: root_binding.realm_id,
        product_root_collateral_policy_id: root_binding.collateral_policy_id,
        product_root_collateral_release_id: root_binding.collateral_release_id,
        product_root_registry_release_id: root_binding.registry_release_id,
        product_root_data_id: root_data_id,
        product_root_semantic_id: root_semantic_id,
        product_root_authentication_id: root_authentication_id,
        product_root_phase: root_phase,
        product_link_account: *frame.series_link.key,
        product_link_binding_id: link_binding_id,
        product_link_data_id: link_data_id,
        product_link_semantic_id: link_semantic_id,
        product_link_authentication_id: link_authentication_id,
        product_link_phase: link_phase,
        funding_account: *frame.series_funding.key,
        funding_state_id,
        funding_data_id,
        funding_authentication_id,
        funding_phase,
        registry_account: registry_account_key,
        registry_authentication_id,
        registry_capability_id,
        source_release_account,
        source_release_manifest_id,
        source_release_authentication_id,
        compiler_bundle_account,
        compiler_bundle_id: bundle_id,
        collateral_profile_id: registry_collateral.profile_id,
        foundation_schedule: artifacts.quote().foundation,
        realm_account: *frame.realm.key,
        revenue,
        treasury,
    })
}

#[cfg(test)]
mod adversarial_source_tests {
    #[test]
    fn current_auth_is_v5_only_and_owns_no_projection_constructor() {
        let source = include_str!("general_market_current_v5.rs");
        let production = source.split("#[cfg(test)]").next().expect("production");
        for required in [
            "MarketBindingV5::decode",
            "authenticate_market_lifecycle_root_v3",
            "authenticate_series_market_link_v3",
            "authenticate_registry_capability_v5",
            "authenticate_series_funding_account_v5",
            "authenticate_compiled_product_series_bundle_v7",
            "authenticate_revenue_policy_record_v2",
        ] {
            assert!(production.contains(required), "missing {required}");
        }
        for forbidden in [
            "MarketBindingV4::decode",
            "MarketLifecycleRootAccountV2",
            "SeriesMarketLinkAccountV2",
            "caller_projection",
        ] {
            assert!(!production.contains(forbidden), "historical authority {forbidden}");
        }
        assert!(!production.contains("impl Clone for AuthenticatedGeneralMarketCurrentV5"));
        assert!(!production.contains("impl Copy for AuthenticatedGeneralMarketCurrentV5"));
    }

    #[test]
    fn large_product_bodies_use_caller_storage_and_all_accounts_are_disjoint() {
        let source = include_str!("general_market_current_v5.rs");
        let auth = source
            .split("fn authenticate_general_market_current_v5_with_product_access")
            .nth(1)
            .and_then(|body| body.split("#[cfg(test)]").next())
            .expect("bounded authenticator");
        assert!(auth.contains("root_output: &mut MarketLifecycleRootAccountV3"));
        assert!(auth.contains("link_output: &mut SeriesMarketLinkAccountV3"));
        assert!(auth.contains("require_distinct_frame(frame)"));
        assert!(!auth.contains("Box::new(MarketLifecycleRootAccountV3"));
        assert!(!auth.contains("Box::new(SeriesMarketLinkAccountV3"));
    }
}
