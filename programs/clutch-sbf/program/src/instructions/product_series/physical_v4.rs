//! Fresh physical custody authority for current FundingV4.
//!
//! This is intentionally not an alias for the historical FundingV2 physical
//! slice. Every retained artifact and account version is current: RegistryV3,
//! RegistryCapabilityV4, BundleV6, QuoteV5, AttachmentV5, and FundingV4.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{read_rent, require_system_program, RentParameters};
use crate::instructions::product_source_current::{
    AuthenticatedCompiledProductSeriesBundleV6, AuthenticatedSeriesSourceArtifactsV5,
};
use crate::instructions::product_series_current::{
    authenticate_series_funding_account_v4, authenticate_series_registry_account_v3,
    AuthenticatedRegistryCapabilityV4, AuthenticatedSeriesFundingAccountV4,
};
use crate::source_plane_v3_actions::SourceLifecycleCapitalizationQuoteV1;
use clutch_product_series::{
    AuthenticatedSeriesFundingAuthorityV4, CompiledProductSeriesBundleV6Id,
    ComponentDebitV1, ContentId, SeriesAttachmentPlanV5, SeriesFundingAbortBindingV4,
    SeriesFundingComponentV2, SeriesFundingCompletionBindingV4,
    SeriesFundingQuoteV5, SeriesFundingReservationBindingV4, SeriesFundingStateV4,
    SeriesFundingStateV4Id, SeriesFundingTerminalProjectionV4, SeriesFundingTermsV2Id,
    SeriesPlanV5, SeriesPlanV5Id, FixedCodec, SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::{
    SeriesFundingAccountV4, SERIES_COLLATERAL_VAULT_COUNT_V2,
    SERIES_FUNDING_ACCOUNT_BYTES_V4,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const SERIES_PHYSICAL_CAPITALIZATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-physical-capitalization/v4\0";
const SERIES_PHYSICAL_RETIREMENT_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-physical-retirement/v4\0";
const SERIES_LAMPORT_CAPITALIZATION_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-lamport-capitalization/v4\0";
const SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-collateral-vault-account-poststate/v4\0";
const SERIES_COLLATERAL_VAULT_POSTSTATE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-collateral-vault-poststate/v4\0";
const SERIES_COLLATERAL_TRANSFER_POSTSTATE_DOMAIN_V4: &[u8] =
    b"dragons-clutch/sbf/series-collateral-transfer-poststate/v4\0";

/// Physical-only suffix appended after Product's already-authenticated current
/// Registry/artifact graph. The roles and order are fixed so callers cannot
/// change component ownership by permuting accounts.
pub(super) const SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V4: usize = 26;
pub(super) const IX_PHYSICAL_PAYER_V4: usize = 0;
pub(super) const IX_PHYSICAL_PAYER_TOKEN_ACCOUNT_V4: usize = 1;
pub(super) const IX_PHYSICAL_PAYER_TOKEN_AUTHORITY_V4: usize = 2;
pub(super) const IX_PHYSICAL_COLLATERAL_REFUND_V4: usize = 3;
pub(super) const IX_PHYSICAL_NEUTRAL_COLLATERAL_V4: usize = 4;
pub(super) const IX_PHYSICAL_NEUTRAL_LAMPORT_V4: usize = 5;
pub(super) const IX_PHYSICAL_COLLATERAL_AUTHORITY_V4: usize = 6;
pub(super) const IX_PHYSICAL_REALM_V4: usize = 7;
pub(super) const IX_PHYSICAL_COLLATERAL_PROFILE_V4: usize = 8;
pub(super) const IX_PHYSICAL_COLLATERAL_POLICY_V4: usize = 9;
pub(super) const IX_PHYSICAL_MINT_V4: usize = 10;
pub(super) const IX_PHYSICAL_TOKEN_PROGRAM_V4: usize = 11;
pub(super) const IX_PHYSICAL_TOKEN_PROGRAMDATA_V4: usize = 12;
pub(super) const IX_PHYSICAL_SYSTEM_PROGRAM_V4: usize = 13;
pub(super) const IX_PHYSICAL_RENT_SYSVAR_V4: usize = 14;
pub(super) const IX_PHYSICAL_LAMPORT_VAULTS_V4: usize = 15;
pub(super) const IX_PHYSICAL_COLLATERAL_VAULTS_V4: usize = 21;

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
    vault_poststate_id: ContentId,
    rent_principal_lamports: u64,
    swept_prefund_donation_lamports: u64,
    collateral_principal_atoms: u64,
    collateral_donation_atoms: u64,
    collateral_atoms_after: u64,
    transfer_poststate_id: ContentId,
}

/// Activation-only pure authority over exact current physical poststates.
/// Every later FundingV4 transition is deliberately refused.
#[derive(Debug)]
struct ExactSeriesPhysicalActivationAuthorityV4 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    compiler_bundle_id: CompiledProductSeriesBundleV6Id,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    payer: Pubkey,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    lamport_vaults: [SeriesLamportVaultCapitalizationFactsV4;
        SERIES_FUNDING_COMPONENT_COUNT_V2],
    collateral_vaults: [SeriesCollateralVaultCapitalizationFactsV4;
        SERIES_COLLATERAL_VAULT_COUNT_V2],
}

impl AuthenticatedSeriesFundingAuthorityV4 for ExactSeriesPhysicalActivationAuthorityV4 {
    fn authenticate_activation(
        &self,
        series: &SeriesPlanV5,
        funding_terms_id: SeriesFundingTermsV2Id,
        compiler_bundle_id: CompiledProductSeriesBundleV6Id,
        quote: &SeriesFundingQuoteV5,
        attachment: &SeriesAttachmentPlanV5,
        principal: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
        donations: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    ) -> clutch_product_series::Result<()> {
        if self.id.is_zero()
            || series.id()? != self.series_plan_id
            || funding_terms_id != self.funding_terms_id
            || compiler_bundle_id != self.compiler_bundle_id
            || quote.id()?.content_id() != self.funding_quote_id
            || attachment.id()?.content_id() != self.attachment_plan_id
            || principal != &self.principal
            || donations != &self.donations
            || self.payer == Pubkey::default()
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        let mut total = 0u64;
        let mut index = 0usize;
        while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            let vault = self.lamport_vaults[index];
            if vault.component.index() != index
                || vault.account == Pubkey::default()
                || vault.principal_lamports != principal[index].lamports
                || vault.donation_lamports != donations[index].lamports
                || donations[index].collateral_atoms != 0
                || vault.balance_after
                    != vault.balance_before
                        .checked_add(vault.principal_lamports)
                        .ok_or(clutch_product_series::Error::UnauthenticatedAuthority)?
            {
                return Err(clutch_product_series::Error::UnauthenticatedAuthority);
            }
            total = total
                .checked_add(vault.principal_lamports)
                .ok_or(clutch_product_series::Error::UnauthenticatedAuthority)?;
            index += 1;
        }
        index = 0;
        while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
            let vault = self.collateral_vaults[index];
            let component_index = vault.component.index();
            if vault.component.collateral_vault_index() != Some(index)
                || vault.account == Pubkey::default()
                || vault.account_data_id.is_zero()
                || vault.rent_principal_lamports == 0
                || vault.collateral_principal_atoms
                    != principal[component_index].collateral_atoms
                || vault.collateral_donation_atoms
                    != donations[component_index].collateral_atoms
                || vault.collateral_atoms_after
                    != vault
                        .collateral_principal_atoms
                        .checked_add(vault.collateral_donation_atoms)
                        .ok_or(clutch_product_series::Error::UnauthenticatedAuthority)?
                || vault.transfer_poststate_id.is_zero()
                    != (vault.collateral_atoms_after == 0)
            {
                return Err(clutch_product_series::Error::UnauthenticatedAuthority);
            }
            index += 1;
        }
        if principal[SeriesFundingComponentV2::SeriesAdmission.index()].collateral_atoms != 0
            || donations[SeriesFundingComponentV2::SeriesAdmission.index()].collateral_atoms != 0
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        if self.payer_lamports_after
            != self
                .payer_lamports_before
                .checked_sub(total)
                .ok_or(clutch_product_series::Error::UnauthenticatedAuthority)?
        {
            return Err(clutch_product_series::Error::UnauthenticatedAuthority);
        }
        Ok(())
    }

    fn current_bucket(&self, _series: &SeriesPlanV5) -> clutch_product_series::Result<u64> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_reservation(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &SeriesFundingReservationBindingV4,
        _reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_completion(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &SeriesFundingCompletionBindingV4,
        _completion_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_abort(
        &self,
        _state: &SeriesFundingStateV4,
        _binding: &SeriesFundingAbortBindingV4,
        _abort_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_donation(
        &self,
        _state: &SeriesFundingStateV4,
        _component: SeriesFundingComponentV2,
        _amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_close(
        &self,
        _state: &SeriesFundingStateV4,
        _terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }
}

#[derive(Debug)]
struct PreparedSeriesLamportCapitalizationV4 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    compiler_bundle_id: CompiledProductSeriesBundleV6Id,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    payer: Pubkey,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    lamport_vaults: [SeriesLamportVaultCapitalizationFactsV4;
        SERIES_FUNDING_COMPONENT_COUNT_V2],
}

#[derive(Debug)]
struct PreparedSeriesCollateralCapitalizationV4 {
    id: ContentId,
    collateral_realm_account: Pubkey,
    collateral_realm_id: ContentId,
    collateral_profile_account: Pubkey,
    collateral_profile_id: ContentId,
    collateral_policy_account: Pubkey,
    collateral_policy_id: ContentId,
    collateral_release_id: ContentId,
    collateral_program_deployment_id: ContentId,
    collateral_release_deployment_receipt_id: ContentId,
    collateral_release_deployment_slot: u64,
    token_programdata: Pubkey,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    neutral_sink_lamports_before: u64,
    neutral_sink_lamports_after: u64,
    source_token_prestate_id: ContentId,
    source_token_poststate_id: ContentId,
    collateral_refund_poststate_id: ContentId,
    neutral_collateral_poststate_id: ContentId,
    collateral_vaults: [SeriesCollateralVaultCapitalizationFactsV4;
        SERIES_COLLATERAL_VAULT_COUNT_V2],
}

/// Fresh move-only current physical activation receipt.
///
/// It is returned only after all eleven vaults and FundingV4 are physically
/// committed and hostile-reopened. The current founder must consume it by
/// value; no public constructor, `Clone`, or ID-only downgrade exists.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesPhysicalCapitalizationV4 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: ContentId,
    compiler_bundle_id: ContentId,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    registry_account: Pubkey,
    registry_authentication_id: ContentId,
    registry_data_id: ContentId,
    registry_observed_lamports: u64,
    registry_rent_principal_lamports: u64,
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
    funding_prefund_donation_lamports: u64,
    funding_bump: u8,
    source_capitalization_quote_id: ContentId,
    source_lifecycle_total_per_occurrence_lamports: u64,
    source_failure_terminal_account_bytes: u64,
    source_failure_terminal_rent_principal_lamports: u64,
    collateral_realm_account: Pubkey,
    collateral_realm_id: ContentId,
    collateral_profile_account: Pubkey,
    collateral_profile_id: ContentId,
    collateral_policy_account: Pubkey,
    collateral_policy_id: ContentId,
    collateral_release_id: ContentId,
    collateral_program_deployment_id: ContentId,
    collateral_release_deployment_receipt_id: ContentId,
    collateral_release_deployment_slot: u64,
    payer: Pubkey,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    neutral_lamport_sink_before: u64,
    neutral_lamport_sink_after: u64,
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
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn funding_account(&self) -> Pubkey {
        self.funding_account
    }

    pub(crate) const fn funding_state_id(&self) -> SeriesFundingStateV4Id {
        self.funding_state_id
    }

    pub(crate) const fn funding_authentication_id(&self) -> ContentId {
        self.funding_authentication_id
    }

    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    pub(crate) const fn registry_account(&self) -> Pubkey {
        self.registry_account
    }

    pub(crate) const fn registry_authentication_id(&self) -> ContentId {
        self.registry_authentication_id
    }

    pub(crate) const fn registry_capability_id(&self) -> ContentId {
        self.registry_capability_id
    }

    pub(crate) const fn funding_data_id(&self) -> ContentId {
        self.funding_data_id
    }

    pub(crate) const fn funding_rent_principal_lamports(&self) -> u64 {
        self.funding_rent_principal_lamports
    }

    pub(crate) const fn source_capitalization_quote_id(&self) -> ContentId {
        self.source_capitalization_quote_id
    }

    pub(crate) const fn source_failure_terminal_rent_principal_lamports(&self) -> u64 {
        self.source_failure_terminal_rent_principal_lamports
    }

    pub(crate) fn principal(
        &self,
    ) -> &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2] {
        &self.principal
    }

    pub(crate) fn donations(
        &self,
    ) -> &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2] {
        &self.donations
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

fn multiply_component_debit_v4(
    value: ComponentDebitV1,
    multiplier: u64,
) -> Outcome<ComponentDebitV1> {
    Ok(ComponentDebitV1 {
        lamports: value
            .lamports
            .checked_mul(multiplier)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        collateral_atoms: value
            .collateral_atoms
            .checked_mul(multiplier)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
    })
}

fn derive_series_activation_principal_v4(
    series: &SeriesPlanV5,
    quote: &SeriesFundingQuoteV5,
) -> Outcome<[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2]> {
    series
        .validate_shape()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    quote
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let multiplier = u64::from(series.instance_count);
    let mut principal = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
    let mut index = 0usize;
    while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        principal[index] = multiply_component_debit_v4(quote.components[index], multiplier)?;
        index += 1;
    }
    Ok(principal)
}

fn series_collateral_vault_poststate_id_v4(
    coordinate: super::SeriesCollateralVaultCoordinateV2,
    account_data_id: ContentId,
    rent_principal_lamports: u64,
    swept_prefund_donation_lamports: u64,
    principal_atoms: u64,
    donation_atoms: u64,
    amount_atoms_after: u64,
    transfer_poststate_id: ContentId,
) -> Outcome<ContentId> {
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_COLLATERAL_VAULT_POSTSTATE_DOMAIN_V4,
            &[coordinate.seed],
            &coordinate.compartment.to_le_bytes(),
            &account_data_id.bytes(),
            &rent_principal_lamports.to_le_bytes(),
            &swept_prefund_donation_lamports.to_le_bytes(),
            &principal_atoms.to_le_bytes(),
            &donation_atoms.to_le_bytes(),
            &amount_atoms_after.to_le_bytes(),
            &transfer_poststate_id.bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(id)
}

fn require_source_capitalization_quote_v4(
    source_quote: &SourceLifecycleCapitalizationQuoteV1,
    quote: &SeriesFundingQuoteV5,
    rent: &RentParameters,
) -> Outcome<()> {
    let source_component = quote.components[SeriesFundingComponentV2::SourceWork.index()];
    let terminal_bytes = usize::try_from(source_quote.failure_terminal_account_bytes())
        .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        !source_quote.id().is_zero()
            && source_component.collateral_atoms == 0
            && source_component.lamports == source_quote.total_lamports()
            && source_quote.total_lamports()
                == source_quote
                    .liveness_work_lamports()
                    .checked_add(source_quote.permanent_and_child_rent_lamports())
                    .ok_or(ClutchError::Arithmetic)?
            && source_quote.failure_terminal_rent_principal_lamports()
                == rent.minimum_balance(terminal_bytes)?,
        ClutchError::MismatchedState,
    )
}

#[allow(clippy::too_many_arguments)]
fn fund_series_lamport_capitalization_v4<'a>(
    program_id: &Pubkey,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    bundle: AuthenticatedCompiledProductSeriesBundleV6,
    source_quote: &SourceLifecycleCapitalizationQuoteV1,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    vaults: &[AccountInfo<'a>],
) -> Outcome<PreparedSeriesLamportCapitalizationV4> {
    super::require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    require_system_program(system_program)?;
    require(
        vaults.len() == SERIES_FUNDING_COMPONENT_COUNT_V2,
        ClutchError::AccountCount,
    )?;
    let series = artifacts.series();
    let terms = artifacts.funding_terms();
    let quote = artifacts.quote();
    let attachment = artifacts.attachment();
    let series_plan_id = series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_quote_id = quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let attachment_plan_id = attachment
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require_source_capitalization_quote_v4(source_quote, quote, rent)?;
    require(
        terms.lamport_principal_refund.bytes() == payer.key.to_bytes()
            && bundle.bundle().series_plan_id == series_plan_id
            && bundle.bundle().funding_terms_id == funding_terms_id
            && bundle.bundle().funding_quote_id.content_id() == funding_quote_id
            && bundle.bundle().attachment_plan_id.content_id() == attachment_plan_id,
        ClutchError::MismatchedState,
    )?;
    let principal = derive_series_activation_principal_v4(series, quote)?;
    let mut donations = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
    let empty = SeriesLamportVaultCapitalizationFactsV4 {
        component: SeriesFundingComponentV2::MarketCore,
        account: Pubkey::default(),
        balance_before: 0,
        principal_lamports: 0,
        donation_lamports: 0,
        balance_after: 0,
    };
    let mut facts = [empty; SERIES_FUNDING_COMPONENT_COUNT_V2];

    let payer_lamports_before = payer.lamports();
    let mut total_principal_lamports = 0u64;
    let mut index = 0usize;
    while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        let component = super::series_funding_component_v2(index)?;
        super::require_lamport_vault_metadata_v2(
            program_id,
            series_plan_id,
            component,
            &vaults[index],
        )?;
        require(
            vaults[index].key != payer.key && vaults[index].key != system_program.key,
            ClutchError::AccountAlias,
        )?;
        let mut other = index + 1;
        while other < SERIES_FUNDING_COMPONENT_COUNT_V2 {
            require(vaults[index].key != vaults[other].key, ClutchError::AccountAlias)?;
            other += 1;
        }
        let balance_before = vaults[index].lamports();
        let principal_lamports = principal[index].lamports;
        donations[index] = ComponentDebitV1 {
            lamports: balance_before,
            collateral_atoms: 0,
        };
        let expected_after = balance_before
            .checked_add(principal_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        if principal_lamports != 0 {
            let transfer = Instruction::new_with_bytes(
                super::SYSTEM_PROGRAM_ID,
                &super::transfer_data(principal_lamports),
                vec![
                    AccountMeta::new(*payer.key, true),
                    AccountMeta::new(*vaults[index].key, false),
                ],
            );
            invoke(
                &transfer,
                &[payer.clone(), vaults[index].clone(), system_program.clone()],
            )
            .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
        }
        require(
            vaults[index].lamports() == expected_after,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        facts[index] = SeriesLamportVaultCapitalizationFactsV4 {
            component,
            account: *vaults[index].key,
            balance_before,
            principal_lamports,
            donation_lamports: balance_before,
            balance_after: expected_after,
        };
        total_principal_lamports = total_principal_lamports
            .checked_add(principal_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        index += 1;
    }
    let payer_lamports_after = payer.lamports();
    require(
        payer_lamports_after
            == payer_lamports_before
                .checked_sub(total_principal_lamports)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let funding_commitment = super::series_collateral_funding_commitment_with_domain_v2(
        SERIES_LAMPORT_CAPITALIZATION_DOMAIN_V4,
        &principal,
        &donations,
    )?;
    let mut vault_body = [0u8; SERIES_FUNDING_COMPONENT_COUNT_V2 * 65];
    index = 0;
    while index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        let at = index
            .checked_mul(65)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        vault_body[at] = u8::try_from(facts[index].component.index())
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?;
        vault_body[at + 1..at + 33].copy_from_slice(facts[index].account.as_ref());
        vault_body[at + 33..at + 41].copy_from_slice(&facts[index].balance_before.to_le_bytes());
        vault_body[at + 41..at + 49]
            .copy_from_slice(&facts[index].principal_lamports.to_le_bytes());
        vault_body[at + 49..at + 57]
            .copy_from_slice(&facts[index].donation_lamports.to_le_bytes());
        vault_body[at + 57..at + 65].copy_from_slice(&facts[index].balance_after.to_le_bytes());
        index += 1;
    }
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_LAMPORT_CAPITALIZATION_DOMAIN_V4,
            program_id.as_ref(),
            &series_plan_id.bytes(),
            &funding_terms_id.bytes(),
            &bundle.bundle_id().bytes(),
            &funding_quote_id.bytes(),
            &attachment_plan_id.bytes(),
            &source_quote.id().bytes(),
            payer.key.as_ref(),
            system_program.key.as_ref(),
            &payer_lamports_before.to_le_bytes(),
            &payer_lamports_after.to_le_bytes(),
            &funding_commitment.bytes(),
            &vault_body,
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(PreparedSeriesLamportCapitalizationV4 {
        id,
        series_plan_id,
        funding_terms_id,
        compiler_bundle_id: bundle.bundle_id(),
        funding_quote_id,
        attachment_plan_id,
        payer: *payer.key,
        payer_lamports_before,
        payer_lamports_after,
        principal,
        donations,
        lamport_vaults: facts,
    })
}

#[allow(clippy::too_many_arguments)]
fn deploy_series_collateral_capitalization_v4<'a>(
    program_id: &Pubkey,
    registry: &AuthenticatedRegistryCapabilityV4,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    bundle: AuthenticatedCompiledProductSeriesBundleV6,
    realm_account: &AccountInfo<'a>,
    profile_account: &AccountInfo<'a>,
    policy_account: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    token_programdata: &AccountInfo<'a>,
    payer: &AccountInfo<'a>,
    payer_token_account: &AccountInfo<'a>,
    payer_token_authority: &AccountInfo<'a>,
    collateral_principal_refund: &AccountInfo<'a>,
    neutral_collateral_disposition: &AccountInfo<'a>,
    collateral_authority: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    vaults: &[AccountInfo<'a>],
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
) -> Outcome<PreparedSeriesCollateralCapitalizationV4> {
    require(
        vaults.len() == SERIES_COLLATERAL_VAULT_COUNT_V2,
        ClutchError::AccountCount,
    )?;
    super::require_signer(payer)?;
    require(payer.is_writable, ClutchError::NotWritable)?;
    super::require_signer(payer_token_authority)?;
    require(
        payer_token_account.is_writable
            && !payer_token_account.is_signer
            && !payer_token_account.executable
            && !payer_token_authority.executable,
        ClutchError::MismatchedState,
    )?;
    require_system_program(system_program)?;

    let series = artifacts.series();
    let terms = artifacts.funding_terms();
    let series_plan_id = series
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = terms
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_quote_id = artifacts
        .quote()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_plan_id = artifacts
        .attachment()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        !registry.activation_consumed()
            && registry.series_plan_id() == series_plan_id
            && registry.funding_terms_id() == funding_terms_id
            && registry.compiler_bundle_id() == bundle.bundle_id()
            && bundle.bundle().series_plan_id == series_plan_id
            && bundle.bundle().funding_terms_id == funding_terms_id
            && bundle.bundle().funding_quote_id == funding_quote_id
            && bundle.bundle().attachment_plan_id == attachment_plan_id
            && terms.collateral_mint.bytes() == mint.key.to_bytes()
            && terms.token_program.bytes() == token_program.key.to_bytes()
            && terms.collateral_principal_refund_token_account.bytes()
                == collateral_principal_refund.key.to_bytes()
            && terms.neutral_collateral_disposition_token_account.bytes()
                == neutral_collateral_disposition.key.to_bytes()
            && terms.lamport_principal_refund.bytes() == payer.key.to_bytes()
            && terms.neutral_lamport_sink.bytes() == neutral_lamport_sink.key.to_bytes()
            && principal[SeriesFundingComponentV2::SeriesAdmission.index()].collateral_atoms == 0
            && donations[SeriesFundingComponentV2::SeriesAdmission.index()].collateral_atoms == 0,
        ClutchError::MismatchedState,
    )?;
    super::require_system_lamport_destination(neutral_lamport_sink, terms.neutral_lamport_sink)?;
    require(
        !collateral_principal_refund.is_signer
            && !collateral_principal_refund.is_writable
            && !collateral_principal_refund.executable
            && !neutral_collateral_disposition.is_signer
            && !neutral_collateral_disposition.is_writable
            && !neutral_collateral_disposition.executable,
        ClutchError::MismatchedState,
    )?;

    let bound = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        policy_account,
        token_program,
    )?;
    let realm = bound.realm();
    let policy = bound.policy();
    let deployment = crate::collateral_release::authenticate_collateral_release_deployment_v2(
        bound.release(),
        token_program,
        token_programdata,
    )?;
    let release_id = bound
        .release()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    require(
        realm.realm == super::collateral_content_id(artifacts.genesis().realm_id)
            && realm.profile == super::collateral_content_id(artifacts.genesis().profile_id)
            && policy.mint == super::collateral_id(mint.key)
            && policy.token_program == super::collateral_id(token_program.key)
            && deployment.release() == bound.release()
            && deployment.release_id() == release_id
            && deployment.programdata_account() == super::collateral_id(token_programdata.key),
        ClutchError::MismatchedState,
    )?;
    super::require_collateral_program(token_program, bound)?;
    super::require_series_collateral_authority(
        program_id,
        series_plan_id,
        collateral_authority,
    )?;

    let (funding_account, _) =
        crate::seeds::series_funding_pda(program_id, &series_plan_id.bytes());
    require(
        payer_token_account.key != mint.key
            && payer_token_account.key != token_program.key
            && payer_token_account.key != token_programdata.key
            && payer_token_account.key != collateral_authority.key
            && payer_token_account.key != collateral_principal_refund.key
            && payer_token_account.key != neutral_collateral_disposition.key
            && payer_token_authority.key != payer_token_account.key
            && payer_token_authority.key != mint.key
            && payer_token_authority.key != token_program.key
            && payer_token_authority.key != token_programdata.key
            && payer_token_authority.key != collateral_authority.key
            && collateral_principal_refund.key != neutral_collateral_disposition.key
            && collateral_principal_refund.key != collateral_authority.key
            && collateral_principal_refund.key != mint.key
            && collateral_principal_refund.key != token_program.key
            && collateral_principal_refund.key != token_programdata.key
            && collateral_principal_refund.key != neutral_lamport_sink.key
            && neutral_collateral_disposition.key != collateral_authority.key
            && neutral_collateral_disposition.key != mint.key
            && neutral_collateral_disposition.key != token_program.key
            && neutral_collateral_disposition.key != token_programdata.key
            && neutral_collateral_disposition.key != neutral_lamport_sink.key
            && realm_account.key != profile_account.key
            && realm_account.key != policy_account.key
            && profile_account.key != policy_account.key,
        ClutchError::AccountAlias,
    )?;
    let fixed = [
        payer_token_account.key,
        payer_token_authority.key,
        collateral_principal_refund.key,
        neutral_collateral_disposition.key,
        collateral_authority.key,
        neutral_lamport_sink.key,
        realm_account.key,
        profile_account.key,
        policy_account.key,
        mint.key,
        token_program.key,
        token_programdata.key,
        system_program.key,
    ];
    let mut left = 0usize;
    while left < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        require(
            vaults[left].key != payer.key
                && vaults[left].key != &funding_account
                && vaults[left].key != &registry.series_registry_account(),
            ClutchError::AccountAlias,
        )?;
        for account in fixed {
            require(vaults[left].key != account, ClutchError::AccountAlias)?;
        }
        let mut right = left + 1;
        while right < SERIES_COLLATERAL_VAULT_COUNT_V2 {
            require(vaults[left].key != vaults[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }

    let refund_data = collateral_principal_refund
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    super::admit_realm_collateral_account_v2(
        bound,
        super::runtime_account_view(collateral_principal_refund, &refund_data),
        super::TokenAccountRoleV2::ReceiveOnly {
            account: super::collateral_id(collateral_principal_refund.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let collateral_refund_poststate_id = super::series_collateral_account_state_id_v2(
        SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V4,
        collateral_principal_refund,
        &refund_data,
    )?;
    drop(refund_data);
    let neutral_data = neutral_collateral_disposition
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    super::admit_realm_collateral_account_v2(
        bound,
        super::runtime_account_view(neutral_collateral_disposition, &neutral_data),
        super::TokenAccountRoleV2::ReceiveOnly {
            account: super::collateral_id(neutral_collateral_disposition.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let neutral_collateral_poststate_id = super::series_collateral_account_state_id_v2(
        SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V4,
        neutral_collateral_disposition,
        &neutral_data,
    )?;
    drop(neutral_data);

    let funding_join = super::SeriesCollateralFundingJoinV2 {
        realm: realm.realm,
        profile: realm.profile,
        series_plan: super::CollateralId::from_bytes(series_plan_id.bytes()),
        funding_terms: super::CollateralId::from_bytes(funding_terms_id.bytes()),
        funding_state_account: super::collateral_id(&funding_account),
        quote: super::CollateralId::from_bytes(funding_quote_id.bytes()),
        funding_authority: super::collateral_id(collateral_authority.key),
        collateral_principal_refund_token_account:
            super::collateral_id(collateral_principal_refund.key),
        neutral_collateral_disposition_token_account:
            super::collateral_id(neutral_collateral_disposition.key),
        payer_lamport_refund: super::collateral_id(payer.key),
        neutral_lamport_sink: super::collateral_id(neutral_lamport_sink.key),
    };
    funding_join
        .validate(bound)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let transfer_authority = super::TransferAuthorityV2 {
        address: super::collateral_id(payer_token_authority.key),
        kind: super::TransferAuthorityKindV2::TransactionSigner,
        is_transaction_signer: payer_token_authority.is_signer,
        program_address_authenticated: false,
        is_writable: payer_token_authority.is_writable,
        executable: payer_token_authority.executable,
        data_is_empty: payer_token_authority.data_is_empty(),
    };
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mint_before = super::admit_realm_collateral_mint_v2(
        bound,
        super::runtime_account_view(mint, &mint_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(mint_data);
    let source_data = payer_token_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_before = super::admit_realm_collateral_account_v2(
        bound,
        super::runtime_account_view(payer_token_account, &source_data),
        super::TokenAccountRoleV2::Holder {
            owner: super::collateral_id(payer_token_authority.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let source_token_prestate_id = super::series_collateral_account_state_id_v2(
        SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V4,
        payer_token_account,
        &source_data,
    )?;
    drop(source_data);

    let empty = SeriesCollateralVaultCapitalizationFactsV4 {
        component: SeriesFundingComponentV2::MarketCore,
        account: Pubkey::default(),
        account_data_id: ContentId::ZERO,
        vault_poststate_id: ContentId::ZERO,
        rent_principal_lamports: 0,
        swept_prefund_donation_lamports: 0,
        collateral_principal_atoms: 0,
        collateral_donation_atoms: 0,
        collateral_atoms_after: 0,
        transfer_poststate_id: ContentId::ZERO,
    };
    let mut vault_facts = [empty; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut total_collateral_atoms = 0u64;
    let mut index = 0usize;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        let coordinate = super::series_collateral_vault_coordinate_v2(index)?;
        let component_index = coordinate.component.index();
        total_collateral_atoms = total_collateral_atoms
            .checked_add(
                principal[component_index]
                    .collateral_atoms
                    .checked_add(donations[component_index].collateral_atoms)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
            )
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        index += 1;
    }
    require(
        source_before.amount_atoms >= total_collateral_atoms,
        ClutchError::MismatchedState,
    )?;
    let payer_lamports_before = payer.lamports();
    let neutral_sink_lamports_before = neutral_lamport_sink.lamports();
    index = 0;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        let coordinate = super::series_collateral_vault_coordinate_v2(index)?;
        let component_index = coordinate.component.index();
        let created = super::create_series_collateral_vault_v2(
            program_id,
            bound,
            series_plan_id,
            coordinate,
            payer,
            &vaults[index],
            collateral_authority,
            mint,
            neutral_lamport_sink,
            system_program,
            token_program,
            rent,
        )?;
        let principal_atoms = principal[component_index].collateral_atoms;
        let donation_atoms = donations[component_index].collateral_atoms;
        let amount_atoms = principal_atoms
            .checked_add(donation_atoms)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let mut transfer_poststate_id = ContentId::ZERO;
        if amount_atoms != 0 {
            let request = super::series_segregated_funding_request_v2(
                bound,
                funding_join,
                coordinate.compartment,
                created.binding,
                super::collateral_id(payer_token_authority.key),
                transfer_authority,
                amount_atoms,
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            let mint_data = mint
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let source_data = payer_token_account
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let destination_data = vaults[index]
                .try_borrow_data()
                .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
            let prepared = super::prepare_realm_collateral_transfer_v2(
                bound,
                request,
                super::runtime_account_view(mint, &mint_data),
                super::runtime_account_view(payer_token_account, &source_data),
                super::runtime_account_view(&vaults[index], &destination_data),
            )
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
            drop(mint_data);
            drop(source_data);
            drop(destination_data);
            let accepted = super::invoke_series_collateral_transfer(
                prepared,
                mint,
                payer_token_account,
                &vaults[index],
                payer_token_authority,
                token_program,
                None,
            )?;
            require(
                accepted.kind == super::CustodyTransferKindV2::SegregatedFunding
                    && accepted.amount_atoms == amount_atoms
                    && accepted.destination_semantic_owner
                        == super::CollateralId::from_bytes(series_plan_id.bytes())
                    && accepted.destination_compartment == coordinate.compartment
                    && accepted.destination_atoms_after == amount_atoms
                    && accepted.mint_supply_after == mint_before.supply_atoms,
                ClutchError::SeriesCustodyDeltaMismatch,
            )?;
            transfer_poststate_id = super::series_collateral_transfer_poststate_id_with_kind_v2(
                SERIES_COLLATERAL_TRANSFER_POSTSTATE_DOMAIN_V4,
                accepted,
                super::CustodyTransferKindV2::SegregatedFunding,
            )?;
        }
        let vault_data = vaults[index]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observation = super::admit_realm_collateral_account_v2(
            bound,
            super::runtime_account_view(&vaults[index], &vault_data),
            super::TokenAccountRoleV2::SegregatedVault(created.binding),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        require(
            observation.amount_atoms == amount_atoms
                && vaults[index].lamports() == created.rent_principal_lamports
                && super::checked_data_len_v2(vault_data.len())? == created.data_length,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        let account_data_id = super::series_collateral_account_state_id_v2(
            SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V4,
            &vaults[index],
            &vault_data,
        )?;
        drop(vault_data);
        let vault_poststate_id = series_collateral_vault_poststate_id_v4(
            coordinate,
            account_data_id,
            created.rent_principal_lamports,
            created.swept_prefund_donation_lamports,
            principal_atoms,
            donation_atoms,
            amount_atoms,
            transfer_poststate_id,
        )?;
        vault_facts[index] = SeriesCollateralVaultCapitalizationFactsV4 {
            component: coordinate.component,
            account: *vaults[index].key,
            account_data_id,
            vault_poststate_id,
            rent_principal_lamports: created.rent_principal_lamports,
            swept_prefund_donation_lamports: created.swept_prefund_donation_lamports,
            collateral_principal_atoms: principal_atoms,
            collateral_donation_atoms: donation_atoms,
            collateral_atoms_after: amount_atoms,
            transfer_poststate_id,
        };
        index += 1;
    }
    let source_data = payer_token_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_after = super::admit_realm_collateral_account_v2(
        bound,
        super::runtime_account_view(payer_token_account, &source_data),
        super::TokenAccountRoleV2::Holder {
            owner: super::collateral_id(payer_token_authority.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        source_after.amount_atoms
            == source_before
                .amount_atoms
                .checked_sub(total_collateral_atoms)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let source_token_poststate_id = super::series_collateral_account_state_id_v2(
        SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V4,
        payer_token_account,
        &source_data,
    )?;
    drop(source_data);
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let mint_after = super::admit_realm_collateral_mint_v2(
        bound,
        super::runtime_account_view(mint, &mint_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        mint_after.supply_atoms == mint_before.supply_atoms,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    drop(mint_data);

    let mut total_vault_rent = 0u64;
    let mut total_prefund_donation = 0u64;
    index = 0;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        total_vault_rent = total_vault_rent
            .checked_add(vault_facts[index].rent_principal_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        total_prefund_donation = total_prefund_donation
            .checked_add(vault_facts[index].swept_prefund_donation_lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        index += 1;
    }
    let payer_lamports_after = payer.lamports();
    let neutral_sink_lamports_after = neutral_lamport_sink.lamports();
    require(
        payer_lamports_after
            == payer_lamports_before
                .checked_sub(total_vault_rent)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            && neutral_sink_lamports_after
                == neutral_sink_lamports_before
                    .checked_add(total_prefund_donation)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let mut vault_poststates = [ContentId::ZERO; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut transfer_poststates = [ContentId::ZERO; SERIES_COLLATERAL_VAULT_COUNT_V2];
    index = 0;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        vault_poststates[index] = vault_facts[index].vault_poststate_id;
        transfer_poststates[index] = vault_facts[index].transfer_poststate_id;
        index += 1;
    }
    let vault_body = super::flatten_series_collateral_ids_v2(&vault_poststates)?;
    let transfer_body = super::flatten_series_collateral_ids_v2(&transfer_poststates)?;
    let collateral_program_deployment_id =
        ContentId::from_bytes(policy.token_program_deployment.bytes());
    let collateral_release_deployment_receipt_id =
        ContentId::from_bytes(deployment.receipt_id().bytes());
    let collateral_release_deployment_slot = deployment.deployment_slot();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_PHYSICAL_CAPITALIZATION_DOMAIN_V4,
            program_id.as_ref(),
            &series_plan_id.bytes(),
            &funding_terms_id.bytes(),
            &bundle.bundle_id().bytes(),
            &funding_quote_id.bytes(),
            &attachment_plan_id.bytes(),
            realm_account.key.as_ref(),
            &realm.realm.bytes(),
            profile_account.key.as_ref(),
            &realm.profile.bytes(),
            policy_account.key.as_ref(),
            &bound.policy_id().bytes(),
            &release_id.bytes(),
            &collateral_program_deployment_id.bytes(),
            token_program.key.as_ref(),
            token_programdata.key.as_ref(),
            &collateral_release_deployment_receipt_id.bytes(),
            &collateral_release_deployment_slot.to_le_bytes(),
            payer.key.as_ref(),
            &payer_lamports_before.to_le_bytes(),
            &payer_lamports_after.to_le_bytes(),
            neutral_lamport_sink.key.as_ref(),
            &neutral_sink_lamports_before.to_le_bytes(),
            &neutral_sink_lamports_after.to_le_bytes(),
            &source_token_prestate_id.bytes(),
            &source_token_poststate_id.bytes(),
            &collateral_refund_poststate_id.bytes(),
            &neutral_collateral_poststate_id.bytes(),
            &vault_body,
            &transfer_body,
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(PreparedSeriesCollateralCapitalizationV4 {
        id,
        collateral_realm_account: *realm_account.key,
        collateral_realm_id: ContentId::from_bytes(realm.realm.bytes()),
        collateral_profile_account: *profile_account.key,
        collateral_profile_id: ContentId::from_bytes(realm.profile.bytes()),
        collateral_policy_account: *policy_account.key,
        collateral_policy_id: ContentId::from_bytes(bound.policy_id().bytes()),
        collateral_release_id: ContentId::from_bytes(release_id.bytes()),
        collateral_program_deployment_id,
        collateral_release_deployment_receipt_id,
        collateral_release_deployment_slot,
        token_programdata: *token_programdata.key,
        payer_lamports_before,
        payer_lamports_after,
        neutral_sink_lamports_before,
        neutral_sink_lamports_after,
        source_token_prestate_id,
        source_token_poststate_id,
        collateral_refund_poststate_id,
        neutral_collateral_poststate_id,
        collateral_vaults: vault_facts,
    })
}

/// Create the sole current FundingV4 account and all eleven segregated
/// custody compartments. The returned receipt is move-only and is not a
/// founder authority: Product must consume it while flipping RegistryV3's
/// replay bit and finishing the current founder in the same instruction.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn capitalize_current_series_physical_v4<'a>(
    program_id: &Pubkey,
    registry: &AuthenticatedRegistryCapabilityV4,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    bundle: AuthenticatedCompiledProductSeriesBundleV6,
    artifacts: &AuthenticatedSeriesSourceArtifactsV5,
    source_quote: &SourceLifecycleCapitalizationQuoteV1,
    accounts: &[AccountInfo<'a>],
) -> Outcome<(
    AuthenticatedSeriesFundingAccountV4,
    AuthenticatedSeriesPhysicalCapitalizationV4,
)> {
    require(
        accounts.len() == SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V4,
        ClutchError::AccountCount,
    )?;
    validate_current_physical_authority_v4(registry, bundle, artifacts)?;
    let series_plan_id = artifacts
        .series()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_terms_id = artifacts
        .funding_terms()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_quote_id = artifacts
        .quote()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let attachment_plan_id = artifacts
        .attachment()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let live_registry = authenticate_series_registry_account_v3(
        program_id,
        registry_account,
        series_plan_id,
        true,
    )?;
    let registry_value = live_registry.value();
    require(
        !registry.activation_consumed()
            && !registry_value.activation_consumed
            && live_registry.account() == registry.series_registry_account()
            && live_registry.authentication_id() == registry.series_registry_authentication_id()
            && registry_value.series_plan_id == series_plan_id
            && registry_value.funding_terms_id == funding_terms_id
            && registry_value.compiler_bundle_id == bundle.bundle_id()
            && registry_value.registry_release_id == registry.registry_release_id()
            && registry_value.capability_profile_id == registry.capability_profile_id(),
        ClutchError::MismatchedState,
    )?;
    let (expected_funding, funding_bump) =
        crate::seeds::series_funding_pda(program_id, &series_plan_id.bytes());
    require(*funding_account.key == expected_funding, ClutchError::WrongPda)?;
    super::require_creatable(funding_account)?;

    let payer = &accounts[IX_PHYSICAL_PAYER_V4];
    let payer_token_account = &accounts[IX_PHYSICAL_PAYER_TOKEN_ACCOUNT_V4];
    let payer_token_authority = &accounts[IX_PHYSICAL_PAYER_TOKEN_AUTHORITY_V4];
    let collateral_principal_refund = &accounts[IX_PHYSICAL_COLLATERAL_REFUND_V4];
    let neutral_collateral_disposition = &accounts[IX_PHYSICAL_NEUTRAL_COLLATERAL_V4];
    let neutral_lamport_sink = &accounts[IX_PHYSICAL_NEUTRAL_LAMPORT_V4];
    let collateral_authority = &accounts[IX_PHYSICAL_COLLATERAL_AUTHORITY_V4];
    let realm_account = &accounts[IX_PHYSICAL_REALM_V4];
    let profile_account = &accounts[IX_PHYSICAL_COLLATERAL_PROFILE_V4];
    let policy_account = &accounts[IX_PHYSICAL_COLLATERAL_POLICY_V4];
    let mint = &accounts[IX_PHYSICAL_MINT_V4];
    let token_program = &accounts[IX_PHYSICAL_TOKEN_PROGRAM_V4];
    let token_programdata = &accounts[IX_PHYSICAL_TOKEN_PROGRAMDATA_V4];
    let system_program = &accounts[IX_PHYSICAL_SYSTEM_PROGRAM_V4];
    let rent_sysvar = &accounts[IX_PHYSICAL_RENT_SYSVAR_V4];
    let lamport_vaults = &accounts[IX_PHYSICAL_LAMPORT_VAULTS_V4
        ..IX_PHYSICAL_COLLATERAL_VAULTS_V4];
    let collateral_vaults = &accounts[IX_PHYSICAL_COLLATERAL_VAULTS_V4
        ..SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V4];
    let mut left = 0usize;
    while left < accounts.len() {
        require(
            accounts[left].key != registry_account.key
                && accounts[left].key != funding_account.key,
            ClutchError::AccountAlias,
        )?;
        let mut right = left + 1;
        while right < accounts.len() {
            // One wallet may be both the System payer and Token holder
            // authority. No other role alias is semantically valid.
            if !((left == IX_PHYSICAL_PAYER_V4
                && right == IX_PHYSICAL_PAYER_TOKEN_AUTHORITY_V4)
                || (right == IX_PHYSICAL_PAYER_V4
                    && left == IX_PHYSICAL_PAYER_TOKEN_AUTHORITY_V4))
            {
                require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            }
            right += 1;
        }
        left += 1;
    }

    let rent = read_rent(rent_sysvar)?;
    require_source_capitalization_quote_v4(source_quote, artifacts.quote(), &rent)?;
    let neutral_lamport_sink_before = neutral_lamport_sink.lamports();
    let lamport = fund_series_lamport_capitalization_v4(
        program_id,
        artifacts,
        bundle,
        source_quote,
        payer,
        system_program,
        &rent,
        lamport_vaults,
    )?;
    let collateral = deploy_series_collateral_capitalization_v4(
        program_id,
        registry,
        artifacts,
        bundle,
        realm_account,
        profile_account,
        policy_account,
        mint,
        token_program,
        token_programdata,
        payer,
        payer_token_account,
        payer_token_authority,
        collateral_principal_refund,
        neutral_collateral_disposition,
        collateral_authority,
        neutral_lamport_sink,
        system_program,
        &rent,
        collateral_vaults,
        lamport.principal,
        lamport.donations,
    )?;
    require(
        collateral.payer_lamports_before == lamport.payer_lamports_after
            && collateral.neutral_sink_lamports_before == neutral_lamport_sink_before,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let authority = ExactSeriesPhysicalActivationAuthorityV4 {
        id: ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                SERIES_PHYSICAL_CAPITALIZATION_DOMAIN_V4,
                &lamport.id.bytes(),
                &collateral.id.bytes(),
                &source_quote.id().bytes(),
            ])
            .to_bytes(),
        ),
        series_plan_id: lamport.series_plan_id,
        funding_terms_id: lamport.funding_terms_id,
        compiler_bundle_id: lamport.compiler_bundle_id,
        funding_quote_id: lamport.funding_quote_id,
        attachment_plan_id: lamport.attachment_plan_id,
        payer: lamport.payer,
        payer_lamports_before: lamport.payer_lamports_before,
        payer_lamports_after: lamport.payer_lamports_after,
        principal: lamport.principal,
        donations: lamport.donations,
        lamport_vaults: lamport.lamport_vaults,
        collateral_vaults: collateral.collateral_vaults,
    };
    let state = SeriesFundingStateV4::activate(
        &authority,
        artifacts.series(),
        funding_terms_id,
        bundle.bundle_id(),
        artifacts.quote(),
        artifacts.attachment(),
        lamport.principal,
        lamport.donations,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?;
    state
        .validate_against(
            artifacts.series(),
            artifacts.quote(),
            artifacts.attachment(),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut collateral_rent = [0u64; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut index = 0usize;
    while index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        collateral_rent[index] = collateral.collateral_vaults[index].rent_principal_lamports;
        index += 1;
    }
    let funding_rent_principal_lamports = rent.minimum_balance(SERIES_FUNDING_ACCOUNT_BYTES_V4)?;
    require(funding_rent_principal_lamports != 0, ClutchError::MismatchedState)?;
    let funding_prefund_donation_lamports = funding_account.lamports();
    let funding_value = SeriesFundingAccountV4 {
        state,
        rent_principal_lamports: funding_rent_principal_lamports,
        collateral_vault_rent_principal_lamports: collateral_rent,
        stored_bump: funding_bump,
    };
    let funding_seed = series_plan_id.bytes();
    let bump_seed = [funding_bump];
    super::create_series_program_account(
        program_id,
        payer,
        funding_account,
        neutral_lamport_sink,
        system_program,
        &rent,
        SERIES_FUNDING_ACCOUNT_BYTES_V4,
        funding_rent_principal_lamports,
        &[crate::seeds::SEED_SERIES_FUNDING_V1, &funding_seed, &bump_seed],
    )?;
    {
        let mut data = funding_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        funding_value.encode(&mut data)?;
    }
    let funding = authenticate_series_funding_account_v4(
        program_id,
        funding_account,
        series_plan_id,
        true,
    )?;
    let funding_state_id = funding
        .state()
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let payer_lamports_after = payer.lamports();
    let neutral_lamport_sink_after = neutral_lamport_sink.lamports();
    require(
        funding.value() == &funding_value
            && funding.observed_lamports() == funding_rent_principal_lamports
            && collateral.payer_lamports_after
                .checked_sub(funding_rent_principal_lamports)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
                == payer_lamports_after
            && collateral
                .neutral_sink_lamports_after
                .checked_add(funding_prefund_donation_lamports)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
                == neutral_lamport_sink_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_PHYSICAL_CAPITALIZATION_DOMAIN_V4,
            program_id.as_ref(),
            &registry.id().bytes(),
            registry_account.key.as_ref(),
            &live_registry.authentication_id().bytes(),
            &live_registry.data_id().bytes(),
            &registry_value.rent_principal_lamports.to_le_bytes(),
            &live_registry.observed_lamports().to_le_bytes(),
            &bundle.bundle_id().bytes(),
            &lamport.id.bytes(),
            &collateral.id.bytes(),
            funding_account.key.as_ref(),
            &funding_state_id.bytes(),
            &funding.data_id().bytes(),
            &funding.authentication_id().bytes(),
            &funding_rent_principal_lamports.to_le_bytes(),
            &funding_prefund_donation_lamports.to_le_bytes(),
            &source_quote.id().bytes(),
            &source_quote.total_lamports().to_le_bytes(),
            &source_quote.failure_terminal_account_bytes().to_le_bytes(),
            &source_quote
                .failure_terminal_rent_principal_lamports()
                .to_le_bytes(),
            &payer_lamports_after.to_le_bytes(),
            &neutral_lamport_sink_before.to_le_bytes(),
            &neutral_lamport_sink_after.to_le_bytes(),
            &rent.lamports_per_byte_year.to_le_bytes(),
            &rent.exemption_threshold.to_bits().to_le_bytes(),
            &[rent.burn_percent],
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    let receipt = AuthenticatedSeriesPhysicalCapitalizationV4 {
        id,
        series_plan_id,
        funding_terms_id: funding_terms_id.content_id(),
        compiler_bundle_id: bundle.bundle_id().content_id(),
        funding_quote_id: funding_quote_id.content_id(),
        attachment_plan_id: attachment_plan_id.content_id(),
        registry_account: live_registry.account(),
        registry_authentication_id: live_registry.authentication_id(),
        registry_data_id: live_registry.data_id(),
        registry_observed_lamports: live_registry.observed_lamports(),
        registry_rent_principal_lamports: registry_value.rent_principal_lamports,
        registry_capability_id: registry.id(),
        registry_release_id: registry.registry_release_id(),
        capability_profile_id: registry.capability_profile_id(),
        program_account: registry.program_account(),
        programdata_account: registry.programdata_account(),
        programdata_sha256: registry.programdata_sha256(),
        funding_account: funding.account(),
        funding_state_id,
        funding_data_id: funding.data_id(),
        funding_authentication_id: funding.authentication_id(),
        funding_rent_principal_lamports,
        funding_prefund_donation_lamports,
        funding_bump,
        source_capitalization_quote_id: source_quote.id(),
        source_lifecycle_total_per_occurrence_lamports: source_quote.total_lamports(),
        source_failure_terminal_account_bytes: source_quote.failure_terminal_account_bytes(),
        source_failure_terminal_rent_principal_lamports:
            source_quote.failure_terminal_rent_principal_lamports(),
        collateral_realm_account: collateral.collateral_realm_account,
        collateral_realm_id: collateral.collateral_realm_id,
        collateral_profile_account: collateral.collateral_profile_account,
        collateral_profile_id: collateral.collateral_profile_id,
        collateral_policy_account: collateral.collateral_policy_account,
        collateral_policy_id: collateral.collateral_policy_id,
        collateral_release_id: collateral.collateral_release_id,
        collateral_program_deployment_id: collateral.collateral_program_deployment_id,
        collateral_release_deployment_receipt_id:
            collateral.collateral_release_deployment_receipt_id,
        collateral_release_deployment_slot: collateral.collateral_release_deployment_slot,
        payer: *payer.key,
        payer_lamports_before: lamport.payer_lamports_before,
        payer_lamports_after,
        neutral_lamport_sink_before,
        neutral_lamport_sink_after,
        lamport_principal_refund: *payer.key,
        collateral_principal_refund: *collateral_principal_refund.key,
        neutral_collateral_disposition: *neutral_collateral_disposition.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
        collateral_mint: *mint.key,
        token_program: *token_program.key,
        token_programdata: collateral.token_programdata,
        collateral_authority: *collateral_authority.key,
        payer_token_account: *payer_token_account.key,
        payer_token_authority: *payer_token_authority.key,
        source_token_prestate_id: collateral.source_token_prestate_id,
        source_token_poststate_id: collateral.source_token_poststate_id,
        collateral_refund_poststate_id: collateral.collateral_refund_poststate_id,
        neutral_collateral_poststate_id: collateral.neutral_collateral_poststate_id,
        rent_sysvar: *rent_sysvar.key,
        rent_lamports_per_byte_year: rent.lamports_per_byte_year,
        rent_exemption_threshold_bits: rent.exemption_threshold.to_bits(),
        rent_burn_percent: rent.burn_percent,
        principal: lamport.principal,
        donations: lamport.donations,
        lamport_vaults: lamport.lamport_vaults,
        collateral_vaults: collateral.collateral_vaults,
    };
    Ok((funding, receipt))
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
            .split("pub(crate) struct AuthenticatedSeriesPhysicalCapitalizationV4")
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
            "source_capitalization_quote_id",
            "source_failure_terminal_rent_principal_lamports",
            "collateral_realm_account",
            "collateral_profile_account",
            "collateral_policy_account",
            "collateral_release_id",
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
        assert_eq!(super::IX_PHYSICAL_REALM_V4, 7);
        assert_eq!(super::IX_PHYSICAL_COLLATERAL_PROFILE_V4, 8);
        assert_eq!(super::IX_PHYSICAL_COLLATERAL_POLICY_V4, 9);
        assert_eq!(super::IX_PHYSICAL_LAMPORT_VAULTS_V4, 15);
        assert_eq!(super::IX_PHYSICAL_COLLATERAL_VAULTS_V4, 21);
        assert_eq!(super::SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V4, 26);
    }
}
