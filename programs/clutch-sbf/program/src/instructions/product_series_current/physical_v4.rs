//! Fresh physical custody authority for current FundingV4.
//!
//! This is intentionally not an alias for the historical FundingV2 physical
//! slice. Every retained artifact and account version is current: RegistryV3,
//! RegistryCapabilityV4, BundleV6, QuoteV5, AttachmentV5, and FundingV4.

use super::{
    AuthenticatedRegistryCapabilityV4, AuthenticatedSeriesFundingAccountV4,
};
use crate::accounts::{require, Outcome};
use crate::error::ClutchError;
use crate::instructions::product_source_current::{
    AuthenticatedCompiledProductSeriesBundleV6, AuthenticatedSeriesSourceArtifactsV5,
};
use clutch_product_series::{
    ComponentDebitV1, ContentId, SeriesFundingComponentV2,
    SeriesFundingStateV4Id, SeriesFundingTerminalProjectionV4,
    SeriesPlanV5Id, SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::SERIES_COLLATERAL_VAULT_COUNT_V2;
use solana_pubkey::Pubkey;

const SERIES_PHYSICAL_CAPITALIZATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-physical-capitalization/v4\0";
const SERIES_PHYSICAL_RETIREMENT_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-physical-retirement/v4\0";

/// Physical-only suffix appended after Product's already-authenticated current
/// Registry/artifact graph. The roles and order are fixed so callers cannot
/// change component ownership by permuting accounts.
pub(super) const SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V4: usize = 23;
pub(super) const IX_PHYSICAL_PAYER_V4: usize = 0;
pub(super) const IX_PHYSICAL_PAYER_TOKEN_ACCOUNT_V4: usize = 1;
pub(super) const IX_PHYSICAL_PAYER_TOKEN_AUTHORITY_V4: usize = 2;
pub(super) const IX_PHYSICAL_COLLATERAL_REFUND_V4: usize = 3;
pub(super) const IX_PHYSICAL_NEUTRAL_COLLATERAL_V4: usize = 4;
pub(super) const IX_PHYSICAL_NEUTRAL_LAMPORT_V4: usize = 5;
pub(super) const IX_PHYSICAL_COLLATERAL_AUTHORITY_V4: usize = 6;
pub(super) const IX_PHYSICAL_MINT_V4: usize = 7;
pub(super) const IX_PHYSICAL_TOKEN_PROGRAM_V4: usize = 8;
pub(super) const IX_PHYSICAL_TOKEN_PROGRAMDATA_V4: usize = 9;
pub(super) const IX_PHYSICAL_SYSTEM_PROGRAM_V4: usize = 10;
pub(super) const IX_PHYSICAL_RENT_SYSVAR_V4: usize = 11;
pub(super) const IX_PHYSICAL_LAMPORT_VAULTS_V4: usize = 12;
pub(super) const IX_PHYSICAL_COLLATERAL_VAULTS_V4: usize = 18;

const _: () = assert!(
    IX_PHYSICAL_LAMPORT_VAULTS_V4 + SERIES_FUNDING_COMPONENT_COUNT_V2
        == IX_PHYSICAL_COLLATERAL_VAULTS_V4
);
const _: () = assert!(
    IX_PHYSICAL_COLLATERAL_VAULTS_V4 + SERIES_COLLATERAL_VAULT_COUNT_V2
        == SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V4
);

/// One exact lamport compartment observation in canonical V2 component order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SeriesLamportVaultCapitalizationFactsV4 {
    component: SeriesFundingComponentV2,
    account: Pubkey,
    balance_before: u64,
    principal_lamports: u64,
    donation_lamports: u64,
    balance_after: u64,
}

/// One exact collateral-capable vault in canonical five-vault order:
/// MarketCore, RecoveryReserve, SourceWork, LiquidityFacility, WrapperSet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SeriesCollateralVaultCapitalizationFactsV4 {
    component: SeriesFundingComponentV2,
    account: Pubkey,
    account_data_id: ContentId,
    rent_principal_lamports: u64,
    swept_prefund_donation_lamports: u64,
    collateral_principal_atoms: u64,
    collateral_donation_atoms: u64,
    collateral_atoms_after: u64,
    transfer_poststate_id: ContentId,
}

/// Fresh move-only current physical activation receipt.
///
/// It is returned only after all eleven vaults and FundingV4 are physically
/// committed and hostile-reopened. The current founder must consume it by
/// value; no public constructor, `Clone`, or ID-only downgrade exists.
#[derive(Debug)]
pub(super) struct AuthenticatedSeriesPhysicalCapitalizationV4 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: ContentId,
    compiler_bundle_id: ContentId,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    registry_account: Pubkey,
    registry_authentication_id: ContentId,
    registry_capability_id: ContentId,
    registry_release_id: ContentId,
    capability_profile_id: ContentId,
    program_account: Pubkey,
    programdata_account: Pubkey,
    programdata_sha256: ContentId,
    funding_account: Pubkey,
    funding_state_id: SeriesFundingStateV4Id,
    funding_data_id: ContentId,
    funding_authentication_id: ContentId,
    funding_rent_principal_lamports: u64,
    funding_bump: u8,
    payer: Pubkey,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    lamport_principal_refund: Pubkey,
    collateral_principal_refund: Pubkey,
    neutral_collateral_disposition: Pubkey,
    neutral_lamport_sink: Pubkey,
    collateral_mint: Pubkey,
    token_program: Pubkey,
    token_programdata: Pubkey,
    collateral_authority: Pubkey,
    payer_token_account: Pubkey,
    payer_token_authority: Pubkey,
    source_token_prestate_id: ContentId,
    source_token_poststate_id: ContentId,
    collateral_refund_poststate_id: ContentId,
    neutral_collateral_poststate_id: ContentId,
    rent_sysvar: Pubkey,
    rent_lamports_per_byte_year: u64,
    rent_exemption_threshold_bits: u64,
    rent_burn_percent: u8,
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    lamport_vaults: [SeriesLamportVaultCapitalizationFactsV4;
        SERIES_FUNDING_COMPONENT_COUNT_V2],
    collateral_vaults: [SeriesCollateralVaultCapitalizationFactsV4;
        SERIES_COLLATERAL_VAULT_COUNT_V2],
}

impl AuthenticatedSeriesPhysicalCapitalizationV4 {
    pub(super) const fn id(&self) -> ContentId {
        self.id
    }

    pub(super) const fn funding_account(&self) -> Pubkey {
        self.funding_account
    }

    pub(super) const fn funding_state_id(&self) -> SeriesFundingStateV4Id {
        self.funding_state_id
    }

    pub(super) const fn funding_authentication_id(&self) -> ContentId {
        self.funding_authentication_id
    }

    pub(super) const fn series_plan_id(&self) -> SeriesPlanV5Id {
        self.series_plan_id
    }
}

/// Complete hostile retirement preflight over the same current physical graph.
/// This remains private until the sole Product retirement outer consumes it.
#[derive(Debug)]
pub(super) struct AuthenticatedSeriesPhysicalRetirementPreflightV4 {
    id: ContentId,
    funding: AuthenticatedSeriesFundingAccountV4,
    registry_capability_id: ContentId,
    compiler_bundle_id: ContentId,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    lamport_vaults: [Pubkey; SERIES_FUNDING_COMPONENT_COUNT_V2],
    collateral_vaults: [Pubkey; SERIES_COLLATERAL_VAULT_COUNT_V2],
    lamport_principal_refund: Pubkey,
    collateral_principal_refund: Pubkey,
    neutral_collateral_disposition: Pubkey,
    neutral_lamport_sink: Pubkey,
    funding_rent_principal_lamports: u64,
    collateral_vault_rent_principal_lamports: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
}

/// Non-Copy proof returned only after every custody is empty/closed and the
/// FundingV4 account has been returned to System with an exact rent split.
#[derive(Debug)]
pub(super) struct AuthenticatedSeriesPhysicalRetirementV4 {
    id: ContentId,
    terminal_projection: SeriesFundingTerminalProjectionV4,
    funding_close_receipt_id: ContentId,
    lamport_retirement_receipt_ids: [ContentId; SERIES_FUNDING_COMPONENT_COUNT_V2],
    collateral_principal_receipt_ids: [ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
    collateral_donation_receipt_ids: [ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
    collateral_close_receipt_ids: [ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
}

impl AuthenticatedSeriesPhysicalRetirementV4 {
    pub(super) const fn id(&self) -> ContentId {
        self.id
    }

    pub(super) const fn terminal_projection(&self) -> SeriesFundingTerminalProjectionV4 {
        self.terminal_projection
    }
}

/// Join already-hostile current semantic owners before any physical movement.
/// The account-level constructor below will refine this into the private
/// capitalization receipt; this function accepts no amount or account list.
pub(super) fn validate_current_physical_authority_v4(
    registry: &AuthenticatedRegistryCapabilityV4,
    bundle: AuthenticatedCompiledProductSeriesBundleV6,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
) -> Outcome<()> {
    artifacts.validate_registry_projection(&registry.projection())?;
    let series_plan_id = artifacts
        .series()
        .id()
        .map_err(|_| ClutchError::MismatchedState)?;
    let funding_terms_id = artifacts
        .funding_terms()
        .id()
        .map_err(|_| ClutchError::MismatchedState)?;
    let quote_id = artifacts
        .quote()
        .id()
        .map_err(|_| ClutchError::MismatchedState)?;
    let attachment_id = artifacts
        .attachment()
        .id()
        .map_err(|_| ClutchError::MismatchedState)?;
    require(
        registry.series_plan_id() == series_plan_id
            && registry.funding_terms_id() == funding_terms_id
            && registry.compiler_bundle_id() == bundle.bundle_id()
            && bundle.bundle().series_plan_id == series_plan_id
            && bundle.bundle().funding_terms_id == funding_terms_id
            && bundle.bundle().funding_quote_id == quote_id
            && bundle.bundle().attachment_plan_id == attachment_id,
        ClutchError::MismatchedState,
    )
}

#[cfg(test)]
mod source_invariants {
    #[test]
    fn v4_physical_authority_is_fresh_and_move_only() {
        let source = include_str!("physical_v4.rs");
        let receipt = source
            .split("pub(super) struct AuthenticatedSeriesPhysicalCapitalizationV4")
            .nth(1)
            .and_then(|value| value.split("impl AuthenticatedSeriesPhysicalCapitalizationV4").next())
            .expect("bounded V4 capitalization receipt");
        assert!(!receipt.contains("Clone"));
        assert!(!receipt.contains("Copy"));
        for current in [
            "registry_capability_id",
            "compiler_bundle_id",
            "funding_quote_id",
            "attachment_plan_id",
            "funding_authentication_id",
            "programdata_sha256",
        ] {
            assert!(receipt.contains(current), "missing {current}");
        }
        assert!(!source.contains("AuthenticatedSeriesFundingAccountV2"));
        assert!(!source.contains("CompiledProductSeriesBundleV5"));
        assert!(!source.contains("SeriesFundingQuoteV4"));
        assert!(source.contains("SERIES_FUNDING_COMPONENT_COUNT_V2"));
        assert!(source.contains("SERIES_COLLATERAL_VAULT_COUNT_V2"));
    }

    #[test]
    fn account_suffix_has_exact_six_plus_five_vault_geometry() {
        assert_eq!(super::IX_PHYSICAL_LAMPORT_VAULTS_V4, 12);
        assert_eq!(super::IX_PHYSICAL_COLLATERAL_VAULTS_V4, 18);
        assert_eq!(super::SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V4, 23);
    }
}
