//! Fresh physical custody authority for current FundingV5.
//!
//! This is intentionally not an alias for the historical FundingV2 physical
//! slice. Every retained artifact and account version is current: RegistryV4,
//! RegistryCapabilityV5, BundleV7, QuoteV6, AttachmentV6, and FundingV5.

use crate::accounts::{require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{read_rent, require_system_program, RentParameters};
use crate::instructions::product_source_current::{
    AuthenticatedCompiledProductSeriesBundleV7, AuthenticatedProductSeriesRegistrationV5,
    AuthenticatedSeriesSourceArtifactsV6,
};
use crate::instructions::product_series_current::{
    authenticate_registry_capability_v5, authenticate_series_funding_account_v5,
    authenticate_series_registry_account_v4,
    AuthenticatedRegistryCapabilityV5, AuthenticatedSeriesFundingAccountV5,
};
use crate::instructions::product_series_current::retirement_v5::
    AuthenticatedProductSeriesLifecycleTerminalV5;
use crate::instructions::product_series::replay_v3::
    AuthenticatedProductSeriesReplayTerminalV5;
use crate::source_plane_v3_actions::SourceLifecycleCapitalizationQuoteV1;
use clutch_product_series::{
    AuthenticatedSeriesFundingAuthorityV5, CompiledProductSeriesBundleV7Id,
    ComponentDebitV1, ContentId, SeriesAttachmentPlanV6, SeriesFundingAbortBindingV5,
    MarketFoundationScheduleV4Id,
    SeriesFundingComponentV2, SeriesFundingCompletionBindingV5, SeriesFundingPhaseV5,
    SeriesFundingQuoteV6, SeriesFundingReservationBindingV5, SeriesFundingStateV5,
    SeriesFundingStateV5Id, SeriesFundingTerminalProjectionV5, SeriesFundingTermsV2Id,
    SeriesPlanV5, SeriesPlanV5Id, FixedCodec, SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::{
    SeriesFundingAccountV5, SeriesRegistryAccountV4, SERIES_COLLATERAL_VAULT_COUNT_V2,
    SERIES_FUNDING_ACCOUNT_BYTES_V5, SERIES_REGISTRY_ACCOUNT_BYTES_V4,
};
use solana_account_info::AccountInfo;
use solana_cpi::{invoke, invoke_signed};
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const SERIES_PHYSICAL_CAPITALIZATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-physical-capitalization/v5\0";
const SERIES_PHYSICAL_RETIREMENT_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-physical-retirement/v5\0";
const SERIES_PHYSICAL_RETIREMENT_PREFLIGHT_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-physical-retirement-preflight/v5\0";
const SERIES_TERMINAL_COLLATERAL_TRANSFER_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-terminal-collateral-transfer/v5\0";
const SERIES_COLLATERAL_RENT_RETIREMENT_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-collateral-rent-retirement/v5\0";
const SERIES_LAMPORT_RETIREMENT_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-lamport-retirement/v5\0";
const SERIES_FUNDING_ACCOUNT_RETIREMENT_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-funding-account-retirement/v5\0";
const SERIES_LAMPORT_CAPITALIZATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-lamport-capitalization/v5\0";
const SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-collateral-vault-account-poststate/v5\0";
const SERIES_COLLATERAL_VAULT_POSTSTATE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-collateral-vault-poststate/v5\0";
const SERIES_COLLATERAL_TRANSFER_POSTSTATE_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-collateral-transfer-poststate/v5\0";
const SERIES_PHYSICAL_FOUNDER_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-physical-founder/v5\0";
const SERIES_PHYSICAL_REGISTRATION_DOMAIN_V5: &[u8] =
    b"dragons-clutch/sbf/series-physical-registration/v5\0";

/// Physical-only suffix appended after Product's already-authenticated current
/// Registry/artifact graph. The roles and order are fixed so callers cannot
/// change component ownership by permuting accounts.
pub(super) const SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V5: usize = 26;
pub(super) const IX_PHYSICAL_PAYER_V5: usize = 0;
pub(super) const IX_PHYSICAL_PAYER_TOKEN_ACCOUNT_V5: usize = 1;
pub(super) const IX_PHYSICAL_PAYER_TOKEN_AUTHORITY_V5: usize = 2;
pub(super) const IX_PHYSICAL_COLLATERAL_REFUND_V5: usize = 3;
pub(super) const IX_PHYSICAL_NEUTRAL_COLLATERAL_V5: usize = 4;
pub(super) const IX_PHYSICAL_NEUTRAL_LAMPORT_V5: usize = 5;
pub(super) const IX_PHYSICAL_COLLATERAL_AUTHORITY_V5: usize = 6;
pub(super) const IX_PHYSICAL_REALM_V5: usize = 7;
pub(super) const IX_PHYSICAL_COLLATERAL_PROFILE_V5: usize = 8;
pub(super) const IX_PHYSICAL_COLLATERAL_POLICY_V5: usize = 9;
pub(super) const IX_PHYSICAL_MINT_V5: usize = 10;
pub(super) const IX_PHYSICAL_TOKEN_PROGRAM_V5: usize = 11;
pub(super) const IX_PHYSICAL_TOKEN_PROGRAMDATA_V5: usize = 12;
pub(super) const IX_PHYSICAL_SYSTEM_PROGRAM_V5: usize = 13;
pub(super) const IX_PHYSICAL_RENT_SYSVAR_V5: usize = 14;
pub(super) const IX_PHYSICAL_LAMPORT_VAULTS_V5: usize = 15;
pub(super) const IX_PHYSICAL_COLLATERAL_VAULTS_V5: usize = 21;

const _: () = assert!(
    IX_PHYSICAL_LAMPORT_VAULTS_V5 + SERIES_FUNDING_COMPONENT_COUNT_V2
        == IX_PHYSICAL_COLLATERAL_VAULTS_V5
);

/// Exact physical-retirement suffix. RegistryV4 and FundingV5 precede this
/// suffix in Product's enclosing terminal instruction and are not duplicated.
pub(crate) const SERIES_PHYSICAL_RETIREMENT_ACCOUNT_COUNT_V5: usize = 24;
pub(crate) const IX_RETIRE_COLLATERAL_REFUND_V5: usize = 0;
pub(crate) const IX_RETIRE_NEUTRAL_COLLATERAL_V5: usize = 1;
pub(crate) const IX_RETIRE_LAMPORT_REFUND_V5: usize = 2;
pub(crate) const IX_RETIRE_NEUTRAL_LAMPORT_V5: usize = 3;
pub(crate) const IX_RETIRE_COLLATERAL_AUTHORITY_V5: usize = 4;
pub(crate) const IX_RETIRE_REALM_V5: usize = 5;
pub(crate) const IX_RETIRE_COLLATERAL_PROFILE_V5: usize = 6;
pub(crate) const IX_RETIRE_COLLATERAL_POLICY_V5: usize = 7;
pub(crate) const IX_RETIRE_MINT_V5: usize = 8;
pub(crate) const IX_RETIRE_TOKEN_PROGRAM_V5: usize = 9;
pub(crate) const IX_RETIRE_TOKEN_PROGRAMDATA_V5: usize = 10;
pub(crate) const IX_RETIRE_SYSTEM_PROGRAM_V5: usize = 11;
pub(crate) const IX_RETIRE_RENT_SYSVAR_V5: usize = 12;
pub(crate) const IX_RETIRE_LAMPORT_VAULTS_V5: usize = 13;
pub(crate) const IX_RETIRE_COLLATERAL_VAULTS_V5: usize = 19;

const _: () = assert!(
    IX_RETIRE_LAMPORT_VAULTS_V5 + SERIES_FUNDING_COMPONENT_COUNT_V2
        == IX_RETIRE_COLLATERAL_VAULTS_V5
);
const _: () = assert!(
    IX_RETIRE_COLLATERAL_VAULTS_V5 + SERIES_COLLATERAL_VAULT_COUNT_V2
        == SERIES_PHYSICAL_RETIREMENT_ACCOUNT_COUNT_V5
);
const _: () = assert!(
    IX_PHYSICAL_COLLATERAL_VAULTS_V5 + SERIES_COLLATERAL_VAULT_COUNT_V2
        == SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V5
);

/// One exact lamport compartment observation in canonical V2 component order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SeriesLamportVaultCapitalizationFactsV5 {
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
struct SeriesCollateralVaultCapitalizationFactsV5 {
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

/// Move-only proof that the sole current RegistryV4 account was created from
/// the pre-Registry BundleV7 authority and hostile-reopened under the exact
/// ReleaseV2/ProfileV4 ProgramData identity.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesPhysicalRegistrationV5 {
    id: ContentId,
    registration: AuthenticatedProductSeriesRegistrationV5,
    registry_account: Pubkey,
    registry_data_id: ContentId,
    registry_authentication_id: ContentId,
    registry_rent_principal_lamports: u64,
    registry_prefund_donation_lamports: u64,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    neutral_sink_lamports_before: u64,
    neutral_sink_lamports_after: u64,
}

impl AuthenticatedSeriesPhysicalRegistrationV5 {
    pub(crate) const fn id(&self) -> ContentId { self.id }
    pub(crate) const fn registration_id(&self) -> ContentId { self.registration.id() }
    pub(crate) const fn registry_account(&self) -> Pubkey { self.registry_account }
    pub(crate) const fn registry_data_id(&self) -> ContentId { self.registry_data_id }
    pub(crate) const fn registry_authentication_id(&self) -> ContentId {
        self.registry_authentication_id
    }
    pub(crate) const fn registry_rent_principal_lamports(&self) -> u64 {
        self.registry_rent_principal_lamports
    }
    pub(crate) const fn compiler_bundle_id(&self) -> CompiledProductSeriesBundleV7Id {
        self.registration.compiler_bundle_id()
    }
    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id {
        self.registration.series_plan_id()
    }
}

/// Activation-only pure authority over exact current physical poststates.
/// Every later FundingV5 transition is deliberately refused.
#[derive(Debug)]
struct ExactSeriesPhysicalActivationAuthorityV5 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    compiler_bundle_id: CompiledProductSeriesBundleV7Id,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    payer: Pubkey,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    lamport_vaults: [SeriesLamportVaultCapitalizationFactsV5;
        SERIES_FUNDING_COMPONENT_COUNT_V2],
    collateral_vaults: [SeriesCollateralVaultCapitalizationFactsV5;
        SERIES_COLLATERAL_VAULT_COUNT_V2],
}

impl AuthenticatedSeriesFundingAuthorityV5 for ExactSeriesPhysicalActivationAuthorityV5 {
    fn authenticate_activation(
        &self,
        series: &SeriesPlanV5,
        funding_terms_id: SeriesFundingTermsV2Id,
        compiler_bundle_id: CompiledProductSeriesBundleV7Id,
        quote: &SeriesFundingQuoteV6,
        attachment: &SeriesAttachmentPlanV6,
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
        _state: &SeriesFundingStateV5,
        _binding: &SeriesFundingReservationBindingV5,
        _reservation_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_completion(
        &self,
        _state: &SeriesFundingStateV5,
        _binding: &SeriesFundingCompletionBindingV5,
        _completion_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_pending_abort(
        &self,
        _state: &SeriesFundingStateV5,
        _binding: &SeriesFundingAbortBindingV5,
        _abort_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_donation(
        &self,
        _state: &SeriesFundingStateV5,
        _component: SeriesFundingComponentV2,
        _amount: ComponentDebitV1,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }

    fn authenticate_close(
        &self,
        _state: &SeriesFundingStateV5,
        _terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Err(clutch_product_series::Error::UnauthenticatedAuthority)
    }
}

#[derive(Debug)]
struct PreparedSeriesLamportCapitalizationV5 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: SeriesFundingTermsV2Id,
    compiler_bundle_id: CompiledProductSeriesBundleV7Id,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    payer: Pubkey,
    payer_lamports_before: u64,
    payer_lamports_after: u64,
    principal: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    donations: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    lamport_vaults: [SeriesLamportVaultCapitalizationFactsV5;
        SERIES_FUNDING_COMPONENT_COUNT_V2],
}

#[derive(Debug)]
struct PreparedSeriesCollateralCapitalizationV5 {
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
    collateral_vaults: [SeriesCollateralVaultCapitalizationFactsV5;
        SERIES_COLLATERAL_VAULT_COUNT_V2],
}

/// Fresh move-only current physical activation receipt.
///
/// It is returned only after all eleven vaults and FundingV5 are physically
/// committed and hostile-reopened. The current founder must consume it by
/// value; no public constructor, `Clone`, or ID-only downgrade exists.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesPhysicalCapitalizationV5 {
    id: ContentId,
    series_plan_id: SeriesPlanV5Id,
    funding_terms_id: ContentId,
    compiler_bundle_id: ContentId,
    funding_quote_id: ContentId,
    foundation_schedule_id: MarketFoundationScheduleV4Id,
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
    funding_state_id: SeriesFundingStateV5Id,
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
    lamport_vaults: [SeriesLamportVaultCapitalizationFactsV5;
        SERIES_FUNDING_COMPONENT_COUNT_V2],
    collateral_vaults: [SeriesCollateralVaultCapitalizationFactsV5;
        SERIES_COLLATERAL_VAULT_COUNT_V2],
}

impl AuthenticatedSeriesPhysicalCapitalizationV5 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn funding_account(&self) -> Pubkey {
        self.funding_account
    }

    pub(crate) const fn funding_state_id(&self) -> SeriesFundingStateV5Id {
        self.funding_state_id
    }

    pub(crate) const fn funding_authentication_id(&self) -> ContentId {
        self.funding_authentication_id
    }

    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id {
        self.series_plan_id
    }

    pub(crate) const fn funding_terms_id(&self) -> ContentId {
        self.funding_terms_id
    }

    pub(crate) const fn compiler_bundle_id(&self) -> ContentId {
        self.compiler_bundle_id
    }

    pub(crate) const fn funding_quote_id(&self) -> ContentId {
        self.funding_quote_id
    }

    pub(crate) const fn foundation_schedule_id(&self) -> MarketFoundationScheduleV4Id {
        self.foundation_schedule_id
    }

    pub(crate) const fn attachment_plan_id(&self) -> ContentId {
        self.attachment_plan_id
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

    pub(crate) const fn collateral_realm_id(&self) -> ContentId {
        self.collateral_realm_id
    }

    pub(crate) const fn collateral_profile_id(&self) -> ContentId {
        self.collateral_profile_id
    }

    pub(crate) const fn lamport_principal_refund(&self) -> Pubkey {
        self.lamport_principal_refund
    }

    pub(crate) const fn payer(&self) -> Pubkey {
        self.payer
    }

    pub(crate) const fn neutral_lamport_sink(&self) -> Pubkey {
        self.neutral_lamport_sink
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

/// Move-only proof that the exact physical capitalization was followed by the
/// sole RegistryV4 replay-bit transition and hostile reauthentication.
///
/// The full physical receipt remains owned here. A current founder must move
/// this value through preauthorization, reservation, Source publication, and
/// FundingV5 completion; an ID-only projection cannot recreate it.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesPhysicalFounderV5 {
    id: ContentId,
    capitalization: AuthenticatedSeriesPhysicalCapitalizationV5,
    registry_data_before_id: ContentId,
    registry_authentication_before_id: ContentId,
    registry_data_after_id: ContentId,
    registry_authentication_after_id: ContentId,
    registry_capability_after_id: ContentId,
}

impl AuthenticatedSeriesPhysicalFounderV5 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn capitalization_id(&self) -> ContentId {
        self.capitalization.id
    }

    pub(crate) const fn capitalization(&self) -> &AuthenticatedSeriesPhysicalCapitalizationV5 {
        &self.capitalization
    }

    pub(crate) const fn registry_data_before_id(&self) -> ContentId {
        self.registry_data_before_id
    }

    pub(crate) const fn registry_authentication_before_id(&self) -> ContentId {
        self.registry_authentication_before_id
    }

    pub(crate) const fn registry_data_after_id(&self) -> ContentId {
        self.registry_data_after_id
    }

    pub(crate) const fn registry_authentication_after_id(&self) -> ContentId {
        self.registry_authentication_after_id
    }

    pub(crate) const fn registry_capability_after_id(&self) -> ContentId {
        self.registry_capability_after_id
    }

    pub(crate) const fn series_plan_id(&self) -> SeriesPlanV5Id {
        self.capitalization.series_plan_id
    }

    pub(crate) const fn attachment_plan_id(&self) -> ContentId {
        self.capitalization.attachment_plan_id
    }

    pub(crate) const fn foundation_schedule_id(&self) -> MarketFoundationScheduleV4Id {
        self.capitalization.foundation_schedule_id
    }

    pub(crate) const fn collateral_realm_id(&self) -> ContentId {
        self.capitalization.collateral_realm_id
    }

    pub(crate) const fn collateral_profile_id(&self) -> ContentId {
        self.capitalization.collateral_profile_id
    }

    pub(crate) const fn capability_profile_id(&self) -> ContentId {
        self.capitalization.capability_profile_id
    }

    pub(crate) const fn registry_release_id(&self) -> ContentId {
        self.capitalization.registry_release_id
    }
}

/// Create and hostile-reopen the sole current RegistryV4 replay anchor.
///
/// The pre-Registry authority is consumed by value. Its FundingTerms payer and
/// neutral sink own all rent and predictable-address prefund disposition; no
/// caller-selected identity or amount enters the persisted RegistryV4 body.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn register_current_series_physical_v5<'a>(
    program_id: &Pubkey,
    registration: AuthenticatedProductSeriesRegistrationV5,
    payer: &AccountInfo<'a>,
    registry_account: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
    program_account: &AccountInfo<'a>,
    programdata_account: &AccountInfo<'a>,
    release_artifact: &AccountInfo<'a>,
    profile_artifact: &AccountInfo<'a>,
) -> Outcome<(
    AuthenticatedRegistryCapabilityV5,
    AuthenticatedSeriesPhysicalRegistrationV5,
)> {
    require_system_program(system_program)?;
    super::require_signer(payer)?;
    require(
        payer.is_writable
            && *payer.key == registration.lamport_principal_refund()
            && *neutral_lamport_sink.key == registration.neutral_lamport_sink()
            && payer.key != neutral_lamport_sink.key
            && payer.key != registry_account.key
            && neutral_lamport_sink.key != registry_account.key
            && registry_account.key != program_account.key
            && registry_account.key != programdata_account.key
            && registry_account.key != release_artifact.key
            && registry_account.key != profile_artifact.key
            && *program_account.key == registration.program_account()
            && *programdata_account.key == registration.programdata_account()
            && *release_artifact.key == registration.release_artifact_account()
            && *profile_artifact.key == registration.profile_artifact_account(),
        ClutchError::MismatchedState,
    )?;
    super::require_system_lamport_destination(
        neutral_lamport_sink,
        ContentId::from_bytes(registration.neutral_lamport_sink().to_bytes()),
    )?;
    let (expected_registry, stored_bump) = crate::seeds::series_registry_pda(
        program_id,
        &registration.series_plan_id().bytes(),
    );
    require(*registry_account.key == expected_registry, ClutchError::WrongPda)?;
    super::require_creatable(registry_account)?;
    let rent = read_rent(rent_sysvar)?;
    let rent_principal_lamports = rent.minimum_balance(SERIES_REGISTRY_ACCOUNT_BYTES_V4)?;
    require(rent_principal_lamports != 0, ClutchError::MismatchedState)?;
    let payer_lamports_before = payer.lamports();
    let registry_prefund_donation_lamports = registry_account.lamports();
    let neutral_sink_lamports_before = neutral_lamport_sink.lamports();
    let series_seed = registration.series_plan_id().bytes();
    let bump_seed = [stored_bump];
    super::create_series_program_account(
        program_id,
        payer,
        registry_account,
        neutral_lamport_sink,
        system_program,
        &rent,
        SERIES_REGISTRY_ACCOUNT_BYTES_V4,
        rent_principal_lamports,
        &[crate::seeds::SEED_SERIES_REGISTRY_V1, &series_seed, &bump_seed],
    )?;
    let value = SeriesRegistryAccountV4 {
        series_plan_id: registration.series_plan_id(),
        funding_terms_id: registration.funding_terms_id(),
        registry_release_id: registration.registry_release_id(),
        capability_profile_id: registration.capability_profile_id(),
        compiler_bundle_id: registration.compiler_bundle_id(),
        rent_principal_lamports,
        stored_bump,
        activation_consumed: false,
    };
    {
        let mut data = registry_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        value.encode(&mut data)?;
    }
    let registry = authenticate_series_registry_account_v4(
        program_id,
        registry_account,
        registration.series_plan_id(),
        true,
    )?;
    require(
        registry.value() == &value
            && registry.observed_lamports() == rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let registry_data_id = registry.data_id();
    let registry_authentication_id = registry.authentication_id();
    let capability = authenticate_registry_capability_v5(
        program_id,
        registry,
        program_account,
        programdata_account,
        release_artifact,
        profile_artifact,
    )?;
    let payer_lamports_after = payer.lamports();
    let neutral_sink_lamports_after = neutral_lamport_sink.lamports();
    require(
        !capability.activation_consumed()
            && capability.series_registry_account() == *registry_account.key
            && capability.series_plan_id() == registration.series_plan_id()
            && capability.funding_terms_id() == registration.funding_terms_id()
            && capability.compiler_bundle_id() == registration.compiler_bundle_id()
            && capability.registry_release_id() == registration.registry_release_id()
            && capability.capability_profile_id() == registration.capability_profile_id()
            && capability.program_account() == registration.program_account()
            && capability.programdata_account() == registration.programdata_account()
            && capability.programdata_sha256() == registration.programdata_sha256()
            && payer_lamports_before
                .checked_sub(rent_principal_lamports)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
                == payer_lamports_after
            && neutral_sink_lamports_before
                .checked_add(registry_prefund_donation_lamports)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
                == neutral_sink_lamports_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_PHYSICAL_REGISTRATION_DOMAIN_V5,
            program_id.as_ref(),
            &registration.id().bytes(),
            registry_account.key.as_ref(),
            &registry_data_id.bytes(),
            &registry_authentication_id.bytes(),
            &capability.id().bytes(),
            &rent_principal_lamports.to_le_bytes(),
            &registry_prefund_donation_lamports.to_le_bytes(),
            payer.key.as_ref(),
            &payer_lamports_before.to_le_bytes(),
            &payer_lamports_after.to_le_bytes(),
            neutral_lamport_sink.key.as_ref(),
            &neutral_sink_lamports_before.to_le_bytes(),
            &neutral_sink_lamports_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok((
        capability,
        AuthenticatedSeriesPhysicalRegistrationV5 {
            id,
            registration,
            registry_account: *registry_account.key,
            registry_data_id,
            registry_authentication_id,
            registry_rent_principal_lamports: rent_principal_lamports,
            registry_prefund_donation_lamports,
            payer_lamports_before,
            payer_lamports_after,
            neutral_sink_lamports_before,
            neutral_sink_lamports_after,
        },
    ))
}

/// Consume the sole current Series activation bit after physical FundingV5
/// capitalization, then hostile-reauthenticate the complete RegistryV4-bound
/// CapabilityV5 loader authority and live FundingV5 account.
///
/// This transition is deliberately inseparable from the move-only physical
/// receipt. It neither accepts a receipt ID nor exposes a generic RegistryV4
/// writer.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn activate_current_series_registry_from_physical_v5<'a>(
    program_id: &Pubkey,
    capability_before: &AuthenticatedRegistryCapabilityV5,
    physical: AuthenticatedSeriesPhysicalCapitalizationV5,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    program_account: &AccountInfo<'a>,
    programdata_account: &AccountInfo<'a>,
    release_artifact: &AccountInfo<'a>,
    profile_artifact: &AccountInfo<'a>,
) -> Outcome<(
    AuthenticatedRegistryCapabilityV5,
    AuthenticatedSeriesFundingAccountV5,
    AuthenticatedSeriesPhysicalFounderV5,
)> {
    let series_plan_id = physical.series_plan_id;
    let registry_before = authenticate_series_registry_account_v4(
        program_id,
        registry_account,
        series_plan_id,
        true,
    )?;
    let funding = authenticate_series_funding_account_v5(
        program_id,
        funding_account,
        series_plan_id,
        true,
    )?;
    let value_before = *registry_before.value();
    require(
        !capability_before.activation_consumed()
            && !value_before.activation_consumed
            && capability_before.id() == physical.registry_capability_id
            && capability_before.series_registry_account() == physical.registry_account
            && capability_before.series_registry_authentication_id()
                == physical.registry_authentication_id
            && registry_before.account() == physical.registry_account
            && registry_before.data_id() == physical.registry_data_id
            && registry_before.authentication_id() == physical.registry_authentication_id
            && registry_before.observed_lamports() == physical.registry_observed_lamports
            && value_before.rent_principal_lamports
                == physical.registry_rent_principal_lamports
            && funding.account() == physical.funding_account
            && funding.state().id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == physical.funding_state_id
            && funding.data_id() == physical.funding_data_id
            && funding.authentication_id() == physical.funding_authentication_id
            && funding.value().rent_principal_lamports
                == physical.funding_rent_principal_lamports
            && capability_before.registry_release_id() == physical.registry_release_id
            && capability_before.capability_profile_id() == physical.capability_profile_id
            && capability_before.program_account() == physical.program_account
            && capability_before.programdata_account() == physical.programdata_account
            && capability_before.programdata_sha256() == physical.programdata_sha256,
        ClutchError::MismatchedState,
    )?;
    let value_after = SeriesRegistryAccountV4 {
        activation_consumed: true,
        ..value_before
    };
    {
        let mut data = registry_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        value_after.encode(&mut data)?;
    }
    let registry_after = authenticate_series_registry_account_v4(
        program_id,
        registry_account,
        series_plan_id,
        true,
    )?;
    require(
        registry_after.value() == &value_after
            && registry_after.observed_lamports() == physical.registry_observed_lamports,
        ClutchError::MismatchedState,
    )?;
    let registry_after_data_id = registry_after.data_id();
    let registry_after_authentication_id = registry_after.authentication_id();
    let capability_after = authenticate_registry_capability_v5(
        program_id,
        registry_after,
        program_account,
        programdata_account,
        release_artifact,
        profile_artifact,
    )?;
    require(
        capability_after.activation_consumed()
            && capability_after.series_registry_account() == physical.registry_account
            && capability_after.series_plan_id() == series_plan_id
            && capability_after.funding_terms_id().content_id() == physical.funding_terms_id
            && capability_after.compiler_bundle_id().content_id() == physical.compiler_bundle_id
            && capability_after.registry_release_id() == physical.registry_release_id
            && capability_after.capability_profile_id() == physical.capability_profile_id
            && capability_after.program_account() == physical.program_account
            && capability_after.programdata_account() == physical.programdata_account
            && capability_after.programdata_sha256() == physical.programdata_sha256,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_PHYSICAL_FOUNDER_DOMAIN_V5,
            program_id.as_ref(),
            &physical.id.bytes(),
            registry_account.key.as_ref(),
            &physical.registry_data_id.bytes(),
            &physical.registry_authentication_id.bytes(),
            &registry_after_data_id.bytes(),
            &registry_after_authentication_id.bytes(),
            &capability_after.id().bytes(),
            funding_account.key.as_ref(),
            &funding.data_id().bytes(),
            &funding.authentication_id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    let capability_after_id = capability_after.id();
    Ok((
        capability_after,
        funding,
        AuthenticatedSeriesPhysicalFounderV5 {
            id,
            registry_data_before_id: physical.registry_data_id,
            registry_authentication_before_id: physical.registry_authentication_id,
            registry_data_after_id,
            registry_authentication_after_id,
            registry_capability_after_id: capability_after_id,
            capitalization: physical,
        },
    ))
}

/// Complete hostile retirement preflight over the same current physical graph.
/// This remains private until the sole Product retirement outer consumes it.
#[derive(Debug)]
pub(super) struct AuthenticatedSeriesPhysicalRetirementPreflightV5 {
    id: ContentId,
    funding: AuthenticatedSeriesFundingAccountV5,
    registry_account: Pubkey,
    registry_data_id: ContentId,
    registry_authentication_id: ContentId,
    registry_observed_lamports: u64,
    registry_capability_id: ContentId,
    compiler_bundle_id: ContentId,
    funding_quote_id: ContentId,
    attachment_plan_id: ContentId,
    bound: super::BoundRealmCollateralV2,
    deployment: crate::collateral_release::AuthenticatedCollateralReleaseDeploymentV2,
    funding_join: super::SeriesCollateralFundingJoinV2,
    lamport_vaults: [Pubkey; SERIES_FUNDING_COMPONENT_COUNT_V2],
    collateral_vaults: [Pubkey; SERIES_COLLATERAL_VAULT_COUNT_V2],
    lamport_principal_refund: Pubkey,
    collateral_principal_refund: Pubkey,
    neutral_collateral_disposition: Pubkey,
    neutral_lamport_sink: Pubkey,
    collateral_principal_refund_prestate_id: ContentId,
    neutral_collateral_disposition_prestate_id: ContentId,
    rent: RentParameters,
    funding_rent_principal_lamports: u64,
    collateral_vault_rent_principal_lamports: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
}

/// Non-Copy proof returned only after every custody is empty/closed and the
/// FundingV5 account has been returned to System with an exact rent split.
#[derive(Debug)]
pub(crate) struct AuthenticatedSeriesPhysicalRetirementV5 {
    id: ContentId,
    lifecycle_terminal_id: ContentId,
    replay_terminal_id: ContentId,
    replay_account: Pubkey,
    replay_authentication_id: ContentId,
    replay_terminal_projection_id: ContentId,
    terminal_projection: SeriesFundingTerminalProjectionV5,
    terminal_projection_id: ContentId,
    registry_account: Pubkey,
    registry_data_id: ContentId,
    registry_authentication_id: ContentId,
    funding_account: Pubkey,
    funding_data_before_id: ContentId,
    funding_authentication_before_id: ContentId,
    funding_close_receipt_id: ContentId,
    lamport_principal_refund: Pubkey,
    collateral_principal_refund: Pubkey,
    neutral_collateral_disposition: Pubkey,
    neutral_lamport_sink: Pubkey,
    collateral_principal_refund_before_id: ContentId,
    collateral_principal_refund_after_id: ContentId,
    neutral_collateral_disposition_before_id: ContentId,
    neutral_collateral_disposition_after_id: ContentId,
    lamport_retirement_receipt_ids: [ContentId; SERIES_FUNDING_COMPONENT_COUNT_V2],
    collateral_principal_receipt_ids: [ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
    collateral_donation_receipt_ids: [ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
    collateral_close_receipt_ids: [ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2],
}

impl AuthenticatedSeriesPhysicalRetirementV5 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn lifecycle_terminal_id(&self) -> ContentId {
        self.lifecycle_terminal_id
    }

    pub(crate) const fn replay_terminal_id(&self) -> ContentId {
        self.replay_terminal_id
    }

    pub(crate) const fn replay_account(&self) -> Pubkey { self.replay_account }

    pub(crate) const fn replay_authentication_id(&self) -> ContentId {
        self.replay_authentication_id
    }

    pub(crate) const fn replay_terminal_projection_id(&self) -> ContentId {
        self.replay_terminal_projection_id
    }

    pub(crate) const fn terminal_projection(&self) -> SeriesFundingTerminalProjectionV5 {
        self.terminal_projection
    }

    pub(crate) const fn terminal_projection_id(&self) -> ContentId {
        self.terminal_projection_id
    }

    pub(crate) const fn registry_account(&self) -> Pubkey { self.registry_account }
    pub(crate) const fn registry_data_id(&self) -> ContentId { self.registry_data_id }
    pub(crate) const fn registry_authentication_id(&self) -> ContentId {
        self.registry_authentication_id
    }
    pub(crate) const fn funding_account(&self) -> Pubkey { self.funding_account }
    pub(crate) const fn funding_data_before_id(&self) -> ContentId {
        self.funding_data_before_id
    }
    pub(crate) const fn funding_authentication_before_id(&self) -> ContentId {
        self.funding_authentication_before_id
    }
    pub(crate) const fn funding_close_receipt_id(&self) -> ContentId {
        self.funding_close_receipt_id
    }
    pub(crate) const fn lamport_principal_refund(&self) -> Pubkey {
        self.lamport_principal_refund
    }
    pub(crate) const fn collateral_principal_refund(&self) -> Pubkey {
        self.collateral_principal_refund
    }
    pub(crate) const fn neutral_collateral_disposition(&self) -> Pubkey {
        self.neutral_collateral_disposition
    }
    pub(crate) const fn neutral_lamport_sink(&self) -> Pubkey {
        self.neutral_lamport_sink
    }

    pub(crate) const fn collateral_principal_refund_before_id(&self) -> ContentId {
        self.collateral_principal_refund_before_id
    }
    pub(crate) const fn collateral_principal_refund_after_id(&self) -> ContentId {
        self.collateral_principal_refund_after_id
    }
    pub(crate) const fn neutral_collateral_disposition_before_id(&self) -> ContentId {
        self.neutral_collateral_disposition_before_id
    }
    pub(crate) const fn neutral_collateral_disposition_after_id(&self) -> ContentId {
        self.neutral_collateral_disposition_after_id
    }
    pub(crate) fn lamport_retirement_receipt_ids(
        &self,
    ) -> &[ContentId; SERIES_FUNDING_COMPONENT_COUNT_V2] {
        &self.lamport_retirement_receipt_ids
    }
    pub(crate) fn collateral_principal_receipt_ids(
        &self,
    ) -> &[ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2] {
        &self.collateral_principal_receipt_ids
    }
    pub(crate) fn collateral_donation_receipt_ids(
        &self,
    ) -> &[ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2] {
        &self.collateral_donation_receipt_ids
    }
    pub(crate) fn collateral_close_receipt_ids(
        &self,
    ) -> &[ContentId; SERIES_COLLATERAL_VAULT_COUNT_V2] {
        &self.collateral_close_receipt_ids
    }
}

fn multiply_component_debit_v5(
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

fn derive_series_activation_principal_v5(
    series: &SeriesPlanV5,
    quote: &SeriesFundingQuoteV6,
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
        principal[index] = multiply_component_debit_v5(quote.components[index], multiplier)?;
        index += 1;
    }
    Ok(principal)
}

fn series_collateral_vault_poststate_id_v5(
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
            SERIES_COLLATERAL_VAULT_POSTSTATE_DOMAIN_V5,
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

fn require_source_capitalization_quote_v5(
    source_quote: &SourceLifecycleCapitalizationQuoteV1,
    quote: &SeriesFundingQuoteV6,
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
fn fund_series_lamport_capitalization_v5<'a>(
    program_id: &Pubkey,
    artifacts: &AuthenticatedSeriesSourceArtifactsV6,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    source_quote: &SourceLifecycleCapitalizationQuoteV1,
    payer: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent: &RentParameters,
    vaults: &[AccountInfo<'a>],
) -> Outcome<PreparedSeriesLamportCapitalizationV5> {
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
    require_source_capitalization_quote_v5(source_quote, quote, rent)?;
    require(
        terms.lamport_principal_refund.bytes() == payer.key.to_bytes()
            && bundle.bundle().series_plan_id == series_plan_id
            && bundle.bundle().funding_terms_id == funding_terms_id
            && bundle.bundle().funding_quote_id.content_id() == funding_quote_id
            && bundle.bundle().attachment_plan_id.content_id() == attachment_plan_id,
        ClutchError::MismatchedState,
    )?;
    let principal = derive_series_activation_principal_v5(series, quote)?;
    let mut donations = [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
    let empty = SeriesLamportVaultCapitalizationFactsV5 {
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
        facts[index] = SeriesLamportVaultCapitalizationFactsV5 {
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
        SERIES_LAMPORT_CAPITALIZATION_DOMAIN_V5,
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
            SERIES_LAMPORT_CAPITALIZATION_DOMAIN_V5,
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
    Ok(PreparedSeriesLamportCapitalizationV5 {
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
fn deploy_series_collateral_capitalization_v5<'a>(
    program_id: &Pubkey,
    registry: &AuthenticatedRegistryCapabilityV5,
    artifacts: &AuthenticatedSeriesSourceArtifactsV6,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
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
) -> Outcome<PreparedSeriesCollateralCapitalizationV5> {
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
        SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V5,
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
        SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V5,
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
        SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V5,
        payer_token_account,
        &source_data,
    )?;
    drop(source_data);

    let empty = SeriesCollateralVaultCapitalizationFactsV5 {
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
                SERIES_COLLATERAL_TRANSFER_POSTSTATE_DOMAIN_V5,
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
            SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V5,
            &vaults[index],
            &vault_data,
        )?;
        drop(vault_data);
        let vault_poststate_id = series_collateral_vault_poststate_id_v5(
            coordinate,
            account_data_id,
            created.rent_principal_lamports,
            created.swept_prefund_donation_lamports,
            principal_atoms,
            donation_atoms,
            amount_atoms,
            transfer_poststate_id,
        )?;
        vault_facts[index] = SeriesCollateralVaultCapitalizationFactsV5 {
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
        SERIES_COLLATERAL_VAULT_ACCOUNT_POSTSTATE_DOMAIN_V5,
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
            SERIES_PHYSICAL_CAPITALIZATION_DOMAIN_V5,
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
    Ok(PreparedSeriesCollateralCapitalizationV5 {
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

/// Create the sole current FundingV5 account and all eleven segregated
/// custody compartments. The returned receipt is move-only and is not a
/// founder authority: Product must consume it while flipping RegistryV4's
/// replay bit and finishing the current founder in the same instruction.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn capitalize_current_series_physical_v5<'a>(
    program_id: &Pubkey,
    registry: &AuthenticatedRegistryCapabilityV5,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    bundle: AuthenticatedCompiledProductSeriesBundleV7,
    artifacts: &AuthenticatedSeriesSourceArtifactsV6,
    source_quote: &SourceLifecycleCapitalizationQuoteV1,
    accounts: &[AccountInfo<'a>],
) -> Outcome<(
    AuthenticatedSeriesFundingAccountV5,
    AuthenticatedSeriesPhysicalCapitalizationV5,
)> {
    require(
        accounts.len() == SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V5,
        ClutchError::AccountCount,
    )?;
    validate_current_physical_authority_v5(registry, &bundle, artifacts)?;
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
    let foundation_schedule_id = artifacts
        .quote()
        .foundation
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let live_registry = authenticate_series_registry_account_v4(
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

    let payer = &accounts[IX_PHYSICAL_PAYER_V5];
    let payer_token_account = &accounts[IX_PHYSICAL_PAYER_TOKEN_ACCOUNT_V5];
    let payer_token_authority = &accounts[IX_PHYSICAL_PAYER_TOKEN_AUTHORITY_V5];
    let collateral_principal_refund = &accounts[IX_PHYSICAL_COLLATERAL_REFUND_V5];
    let neutral_collateral_disposition = &accounts[IX_PHYSICAL_NEUTRAL_COLLATERAL_V5];
    let neutral_lamport_sink = &accounts[IX_PHYSICAL_NEUTRAL_LAMPORT_V5];
    let collateral_authority = &accounts[IX_PHYSICAL_COLLATERAL_AUTHORITY_V5];
    let realm_account = &accounts[IX_PHYSICAL_REALM_V5];
    let profile_account = &accounts[IX_PHYSICAL_COLLATERAL_PROFILE_V5];
    let policy_account = &accounts[IX_PHYSICAL_COLLATERAL_POLICY_V5];
    let mint = &accounts[IX_PHYSICAL_MINT_V5];
    let token_program = &accounts[IX_PHYSICAL_TOKEN_PROGRAM_V5];
    let token_programdata = &accounts[IX_PHYSICAL_TOKEN_PROGRAMDATA_V5];
    let system_program = &accounts[IX_PHYSICAL_SYSTEM_PROGRAM_V5];
    let rent_sysvar = &accounts[IX_PHYSICAL_RENT_SYSVAR_V5];
    let lamport_vaults = &accounts[IX_PHYSICAL_LAMPORT_VAULTS_V5
        ..IX_PHYSICAL_COLLATERAL_VAULTS_V5];
    let collateral_vaults = &accounts[IX_PHYSICAL_COLLATERAL_VAULTS_V5
        ..SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V5];
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
            if !((left == IX_PHYSICAL_PAYER_V5
                && right == IX_PHYSICAL_PAYER_TOKEN_AUTHORITY_V5)
                || (right == IX_PHYSICAL_PAYER_V5
                    && left == IX_PHYSICAL_PAYER_TOKEN_AUTHORITY_V5))
            {
                require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            }
            right += 1;
        }
        left += 1;
    }

    let rent = read_rent(rent_sysvar)?;
    require_source_capitalization_quote_v5(source_quote, artifacts.quote(), &rent)?;
    let neutral_lamport_sink_before = neutral_lamport_sink.lamports();
    let lamport = fund_series_lamport_capitalization_v5(
        program_id,
        artifacts,
        &bundle,
        source_quote,
        payer,
        system_program,
        &rent,
        lamport_vaults,
    )?;
    let collateral = deploy_series_collateral_capitalization_v5(
        program_id,
        registry,
        artifacts,
        &bundle,
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
    let authority = ExactSeriesPhysicalActivationAuthorityV5 {
        id: ContentId::from_bytes(
            solana_sha256_hasher::hashv(&[
                SERIES_PHYSICAL_CAPITALIZATION_DOMAIN_V5,
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
    let state = SeriesFundingStateV5::activate(
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
    let funding_rent_principal_lamports = rent.minimum_balance(SERIES_FUNDING_ACCOUNT_BYTES_V5)?;
    require(funding_rent_principal_lamports != 0, ClutchError::MismatchedState)?;
    let funding_prefund_donation_lamports = funding_account.lamports();
    let funding_value = SeriesFundingAccountV5 {
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
        SERIES_FUNDING_ACCOUNT_BYTES_V5,
        funding_rent_principal_lamports,
        &[crate::seeds::SEED_SERIES_FUNDING_V1, &funding_seed, &bump_seed],
    )?;
    {
        let mut data = funding_account
            .try_borrow_mut_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        funding_value.encode(&mut data)?;
    }
    let funding = authenticate_series_funding_account_v5(
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
            SERIES_PHYSICAL_CAPITALIZATION_DOMAIN_V5,
            program_id.as_ref(),
            &registry.id().bytes(),
            registry_account.key.as_ref(),
            &live_registry.authentication_id().bytes(),
            &live_registry.data_id().bytes(),
            &registry_value.rent_principal_lamports.to_le_bytes(),
            &live_registry.observed_lamports().to_le_bytes(),
            &bundle.bundle_id().bytes(),
            &foundation_schedule_id.bytes(),
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
    let receipt = AuthenticatedSeriesPhysicalCapitalizationV5 {
        id,
        series_plan_id,
        funding_terms_id: funding_terms_id.content_id(),
        compiler_bundle_id: bundle.bundle_id().content_id(),
        funding_quote_id: funding_quote_id.content_id(),
        foundation_schedule_id,
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
pub(super) fn validate_current_physical_authority_v5(
    registry: &AuthenticatedRegistryCapabilityV5,
    bundle: &AuthenticatedCompiledProductSeriesBundleV7,
    artifacts: &AuthenticatedSeriesSourceArtifactsV6,
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

#[allow(clippy::too_many_arguments)]
fn authenticate_series_physical_retirement_preflight_v5<'a>(
    program_id: &Pubkey,
    terminal: &AuthenticatedProductSeriesLifecycleTerminalV5,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    accounts: &[AccountInfo<'a>],
) -> Outcome<AuthenticatedSeriesPhysicalRetirementPreflightV5> {
    require(
        accounts.len() == SERIES_PHYSICAL_RETIREMENT_ACCOUNT_COUNT_V5,
        ClutchError::AccountCount,
    )?;
    let collateral_principal_refund = &accounts[IX_RETIRE_COLLATERAL_REFUND_V5];
    let neutral_collateral_disposition = &accounts[IX_RETIRE_NEUTRAL_COLLATERAL_V5];
    let lamport_principal_refund = &accounts[IX_RETIRE_LAMPORT_REFUND_V5];
    let neutral_lamport_sink = &accounts[IX_RETIRE_NEUTRAL_LAMPORT_V5];
    let collateral_authority = &accounts[IX_RETIRE_COLLATERAL_AUTHORITY_V5];
    let realm_account = &accounts[IX_RETIRE_REALM_V5];
    let profile_account = &accounts[IX_RETIRE_COLLATERAL_PROFILE_V5];
    let policy_account = &accounts[IX_RETIRE_COLLATERAL_POLICY_V5];
    let mint = &accounts[IX_RETIRE_MINT_V5];
    let token_program = &accounts[IX_RETIRE_TOKEN_PROGRAM_V5];
    let token_programdata = &accounts[IX_RETIRE_TOKEN_PROGRAMDATA_V5];
    let system_program = &accounts[IX_RETIRE_SYSTEM_PROGRAM_V5];
    let rent_sysvar = &accounts[IX_RETIRE_RENT_SYSVAR_V5];
    let lamport_vaults =
        &accounts[IX_RETIRE_LAMPORT_VAULTS_V5..IX_RETIRE_COLLATERAL_VAULTS_V5];
    let collateral_vaults =
        &accounts[IX_RETIRE_COLLATERAL_VAULTS_V5..SERIES_PHYSICAL_RETIREMENT_ACCOUNT_COUNT_V5];

    require(
        registry_account.key != funding_account.key
            && !registry_account.is_writable
            && funding_account.is_writable
            && !registry_account.executable
            && !funding_account.executable,
        ClutchError::MismatchedState,
    )?;
    let mut left = 0usize;
    while left < accounts.len() {
        require(
            accounts[left].key != registry_account.key
                && accounts[left].key != funding_account.key,
            ClutchError::AccountAlias,
        )?;
        let mut right = left + 1;
        while right < accounts.len() {
            require(accounts[left].key != accounts[right].key, ClutchError::AccountAlias)?;
            right += 1;
        }
        left += 1;
    }

    require_system_program(system_program)?;
    let rent = read_rent(rent_sysvar)?;
    let registry = terminal.registry();
    let bundle = terminal.bundle();
    let artifacts = terminal.artifacts();
    validate_current_physical_authority_v5(registry, bundle, artifacts)?;
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
    let live_registry = authenticate_series_registry_account_v4(
        program_id,
        registry_account,
        series_plan_id,
        false,
    )?;
    let funding = authenticate_series_funding_account_v5(
        program_id,
        funding_account,
        series_plan_id,
        true,
    )?;
    let projection = terminal.terminal_projection();
    let projection_id = projection
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        registry.activation_consumed()
            && live_registry.account() == registry.series_registry_account()
            && live_registry.authentication_id()
                == registry.series_registry_authentication_id()
            && registry.series_plan_id() == series_plan_id
            && registry.funding_terms_id() == funding_terms_id
            && registry.compiler_bundle_id() == bundle.bundle_id()
            && funding.state().phase == SeriesFundingPhaseV5::Closed
            && funding.state().series_plan_id == series_plan_id
            && funding.state().funding_terms_id == funding_terms_id
            && funding.state().compiler_bundle_id == bundle.bundle_id()
            && funding.state().funding_quote_id == funding_quote_id
            && funding.state().attachment_plan_id == attachment_plan_id
            && projection.series_plan_id == series_plan_id
            && projection.funding_terms_id == funding_terms_id
            && projection.compiler_bundle_id == bundle.bundle_id()
            && projection.transition_sequence == funding.state().transition_sequence
            && projection_id == terminal.terminal_projection_id()
            && funding.observed_lamports() >= funding.value().rent_principal_lamports
            && funding.value().rent_principal_lamports != 0,
        ClutchError::MismatchedState,
    )?;
    terminal.authenticate_physical_preflight_v5(registry, &funding, projection)?;
    funding
        .state()
        .validate_against(artifacts.series(), artifacts.quote(), artifacts.attachment())
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mut component_index = 0usize;
    while component_index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        require(
            projection.refundable_principal[component_index]
                == funding.state().components[component_index].remaining_principal
                && projection.donation_residue[component_index]
                    == funding.state().components[component_index].donations,
            ClutchError::MismatchedState,
        )?;
        component_index += 1;
    }

    let terms = artifacts.funding_terms();
    require(
        terms.collateral_mint.bytes() == mint.key.to_bytes()
            && terms.token_program.bytes() == token_program.key.to_bytes()
            && terms.collateral_principal_refund_token_account.bytes()
                == collateral_principal_refund.key.to_bytes()
            && terms.neutral_collateral_disposition_token_account.bytes()
                == neutral_collateral_disposition.key.to_bytes()
            && terms.lamport_principal_refund.bytes()
                == lamport_principal_refund.key.to_bytes()
            && terms.neutral_lamport_sink.bytes() == neutral_lamport_sink.key.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    super::require_system_lamport_destination(
        lamport_principal_refund,
        terms.lamport_principal_refund,
    )?;
    super::require_system_lamport_destination(neutral_lamport_sink, terms.neutral_lamport_sink)?;

    let bound = crate::collateral_release::authenticate_realm_collateral_v2(
        program_id,
        realm_account,
        profile_account,
        policy_account,
        token_program,
    )?;
    let deployment = crate::collateral_release::authenticate_collateral_release_deployment_v2(
        bound.release(),
        token_program,
        token_programdata,
    )?;
    super::require_collateral_program(token_program, bound)?;
    super::require_series_collateral_authority(program_id, series_plan_id, collateral_authority)?;
    let realm = bound.realm();
    require(
        realm.realm == super::collateral_content_id(artifacts.genesis().realm_id)
            && realm.profile == super::collateral_content_id(artifacts.genesis().profile_id)
            && bound.policy().mint == super::collateral_id(mint.key)
            && bound.policy().token_program == super::collateral_id(token_program.key)
            && deployment.release() == bound.release()
            && deployment.release_id()
                == bound
                    .release()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::AuthorizationUnavailable))?,
        ClutchError::MismatchedState,
    )?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    super::admit_realm_collateral_mint_v2(
        bound,
        super::runtime_account_view(mint, &mint_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(mint_data);
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
    let collateral_principal_refund_prestate_id = super::series_collateral_account_state_id_v2(
        SERIES_TERMINAL_COLLATERAL_TRANSFER_DOMAIN_V5,
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
    let neutral_collateral_disposition_prestate_id = super::series_collateral_account_state_id_v2(
        SERIES_TERMINAL_COLLATERAL_TRANSFER_DOMAIN_V5,
        neutral_collateral_disposition,
        &neutral_data,
    )?;
    drop(neutral_data);

    let funding_join = super::SeriesCollateralFundingJoinV2 {
        realm: realm.realm,
        profile: realm.profile,
        series_plan: super::CollateralId::from_bytes(series_plan_id.bytes()),
        funding_terms: super::CollateralId::from_bytes(funding_terms_id.bytes()),
        funding_state_account: super::collateral_id(funding_account.key),
        quote: super::CollateralId::from_bytes(funding_quote_id.bytes()),
        funding_authority: super::collateral_id(collateral_authority.key),
        collateral_principal_refund_token_account:
            super::collateral_id(collateral_principal_refund.key),
        neutral_collateral_disposition_token_account:
            super::collateral_id(neutral_collateral_disposition.key),
        payer_lamport_refund: super::collateral_id(lamport_principal_refund.key),
        neutral_lamport_sink: super::collateral_id(neutral_lamport_sink.key),
    };
    funding_join
        .validate(bound)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let mut authenticated_collateral_vaults =
        [Pubkey::default(); SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut vault_index = 0usize;
    while vault_index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        let coordinate = super::series_collateral_vault_coordinate_v2(vault_index)?;
        let binding = super::series_collateral_binding_v2(
            program_id,
            bound,
            series_plan_id,
            coordinate,
            &collateral_vaults[vault_index],
            collateral_authority,
        )?;
        let data = collateral_vaults[vault_index]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let observation = super::admit_realm_collateral_account_v2(
            bound,
            super::runtime_account_view(&collateral_vaults[vault_index], &data),
            super::TokenAccountRoleV2::SegregatedVault(binding),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
        let component = coordinate.component.index();
        let expected_atoms = projection.refundable_principal[component]
            .collateral_atoms
            .checked_add(projection.donation_residue[component].collateral_atoms)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let stored_rent = funding.value().collateral_vault_rent_principal_lamports[vault_index];
        require(
            observation.amount_atoms == expected_atoms
                && stored_rent != 0
                && collateral_vaults[vault_index].lamports() >= stored_rent
                && collateral_vaults[vault_index].lamports() >= rent.minimum_balance(data.len())?,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        drop(data);
        authenticated_collateral_vaults[vault_index] = *collateral_vaults[vault_index].key;
        vault_index += 1;
    }
    require(
        projection.refundable_principal[SeriesFundingComponentV2::SeriesAdmission.index()]
            .collateral_atoms
            == 0
            && projection.donation_residue[SeriesFundingComponentV2::SeriesAdmission.index()]
                .collateral_atoms
                == 0,
        ClutchError::MismatchedState,
    )?;

    let mut authenticated_lamport_vaults =
        [Pubkey::default(); SERIES_FUNDING_COMPONENT_COUNT_V2];
    component_index = 0;
    while component_index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        let component = super::series_funding_component_v2(component_index)?;
        super::require_lamport_vault_metadata_v2(
            program_id,
            series_plan_id,
            component,
            &lamport_vaults[component_index],
        )?;
        let expected_lamports = projection.refundable_principal[component_index]
            .lamports
            .checked_add(projection.donation_residue[component_index].lamports)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        require(
            lamport_vaults[component_index].lamports() == expected_lamports,
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        authenticated_lamport_vaults[component_index] = *lamport_vaults[component_index].key;
        component_index += 1;
    }

    let mut vault_accounts = [0u8; 32 * (SERIES_FUNDING_COMPONENT_COUNT_V2
        + SERIES_COLLATERAL_VAULT_COUNT_V2)];
    component_index = 0;
    while component_index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        let offset = component_index
            .checked_mul(32)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        vault_accounts[offset..offset + 32]
            .copy_from_slice(authenticated_lamport_vaults[component_index].as_ref());
        component_index += 1;
    }
    vault_index = 0;
    while vault_index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        let ordinal = SERIES_FUNDING_COMPONENT_COUNT_V2
            .checked_add(vault_index)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        let offset = ordinal
            .checked_mul(32)
            .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
        vault_accounts[offset..offset + 32]
            .copy_from_slice(authenticated_collateral_vaults[vault_index].as_ref());
        vault_index += 1;
    }
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_PHYSICAL_RETIREMENT_PREFLIGHT_DOMAIN_V5,
            program_id.as_ref(),
            &terminal.id().bytes(),
            &projection_id.bytes(),
            registry_account.key.as_ref(),
            &live_registry.data_id().bytes(),
            &live_registry.authentication_id().bytes(),
            funding_account.key.as_ref(),
            &funding.data_id().bytes(),
            &funding.authentication_id().bytes(),
            &deployment.receipt_id().bytes(),
            collateral_principal_refund.key.as_ref(),
            neutral_collateral_disposition.key.as_ref(),
            lamport_principal_refund.key.as_ref(),
            neutral_lamport_sink.key.as_ref(),
            &vault_accounts,
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSeriesPhysicalRetirementPreflightV5 {
        id,
        registry_account: live_registry.account(),
        registry_data_id: live_registry.data_id(),
        registry_authentication_id: live_registry.authentication_id(),
        registry_observed_lamports: live_registry.observed_lamports(),
        registry_capability_id: registry.id(),
        compiler_bundle_id: bundle.bundle_id().content_id(),
        funding_quote_id: funding_quote_id.content_id(),
        attachment_plan_id: attachment_plan_id.content_id(),
        bound,
        deployment,
        funding_join,
        lamport_vaults: authenticated_lamport_vaults,
        collateral_vaults: authenticated_collateral_vaults,
        lamport_principal_refund: *lamport_principal_refund.key,
        collateral_principal_refund: *collateral_principal_refund.key,
        neutral_collateral_disposition: *neutral_collateral_disposition.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
        collateral_principal_refund_prestate_id,
        neutral_collateral_disposition_prestate_id,
        rent,
        funding_rent_principal_lamports: funding.value().rent_principal_lamports,
        collateral_vault_rent_principal_lamports:
            funding.value().collateral_vault_rent_principal_lamports,
        funding,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SeriesTerminalCollateralMovementV5 {
    PrincipalRefund,
    DonationDisposition,
}

#[allow(clippy::too_many_arguments)]
fn transfer_series_terminal_collateral_v5<'a>(
    program_id: &Pubkey,
    preflight: &AuthenticatedSeriesPhysicalRetirementPreflightV5,
    terminal_join: super::SeriesCollateralTerminalJoinV2,
    projection: SeriesFundingTerminalProjectionV5,
    coordinate: super::SeriesCollateralVaultCoordinateV2,
    movement: SeriesTerminalCollateralMovementV5,
    mint: &AccountInfo<'a>,
    vault: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
) -> Outcome<ContentId> {
    let component_index = coordinate.component.index();
    let amount_atoms = match movement {
        SeriesTerminalCollateralMovementV5::PrincipalRefund => {
            projection.refundable_principal[component_index].collateral_atoms
        }
        SeriesTerminalCollateralMovementV5::DonationDisposition => {
            projection.donation_residue[component_index].collateral_atoms
        }
    };
    if amount_atoms == 0 {
        return Ok(ContentId::ZERO);
    }
    let binding = super::series_collateral_binding_v2(
        program_id,
        preflight.bound,
        projection.series_plan_id,
        coordinate,
        vault,
        authority,
    )?;
    let transfer_authority = super::TransferAuthorityV2 {
        address: super::collateral_id(authority.key),
        kind: super::TransferAuthorityKindV2::ProgramDerived,
        is_transaction_signer: authority.is_signer,
        program_address_authenticated: true,
        is_writable: authority.is_writable,
        executable: authority.executable,
        data_is_empty: authority.data_is_empty(),
    };
    let request = match movement {
        SeriesTerminalCollateralMovementV5::PrincipalRefund => {
            super::series_principal_refund_request_v2(
                preflight.bound,
                terminal_join,
                coordinate.compartment,
                binding,
                transfer_authority,
                amount_atoms,
            )
        }
        SeriesTerminalCollateralMovementV5::DonationDisposition => {
            super::series_donation_disposition_request_v2(
                preflight.bound,
                terminal_join,
                coordinate.compartment,
                binding,
                transfer_authority,
                amount_atoms,
            )
        }
    }
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let source_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let destination_data = destination
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let prepared = super::prepare_realm_collateral_transfer_v2(
        preflight.bound,
        request,
        super::runtime_account_view(mint, &mint_data),
        super::runtime_account_view(vault, &source_data),
        super::runtime_account_view(destination, &destination_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(mint_data);
    drop(source_data);
    drop(destination_data);
    let series_seed = projection.series_plan_id.bytes();
    let (_, bump) = crate::seeds::series_collateral_authority_pda(program_id, &series_seed);
    let bump_seed = [bump];
    let signer_seeds: &[&[u8]] = &[
        crate::seeds::SEED_SERIES_COLLATERAL_AUTHORITY_V1,
        &series_seed,
        &bump_seed,
    ];
    let accepted = super::invoke_series_collateral_transfer(
        prepared,
        mint,
        vault,
        destination,
        authority,
        token_program,
        Some(signer_seeds),
    )?;
    let expected_kind = match movement {
        SeriesTerminalCollateralMovementV5::PrincipalRefund => {
            super::CustodyTransferKindV2::PrincipalRefund
        }
        SeriesTerminalCollateralMovementV5::DonationDisposition => {
            super::CustodyTransferKindV2::DonationDisposition
        }
    };
    require(
        accepted.kind == expected_kind
            && accepted.amount_atoms == amount_atoms
            && accepted.source_semantic_owner
                == super::CollateralId::from_bytes(projection.series_plan_id.bytes())
            && accepted.source_compartment == coordinate.compartment
            && accepted.destination_semantic_owner
                == super::CollateralId::from_bytes(projection.funding_terms_id.bytes())
            && accepted.destination_compartment == 0,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let kind = match movement {
        SeriesTerminalCollateralMovementV5::PrincipalRefund => [1u8],
        SeriesTerminalCollateralMovementV5::DonationDisposition => [2u8],
    };
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_TERMINAL_COLLATERAL_TRANSFER_DOMAIN_V5,
            &projection.terminal_receipt_id.bytes(),
            &kind,
            &[coordinate.seed],
            &coordinate.compartment.to_le_bytes(),
            vault.key.as_ref(),
            destination.key.as_ref(),
            &accepted.amount_atoms.to_le_bytes(),
            &accepted.source_atoms_after.to_le_bytes(),
            &accepted.destination_atoms_after.to_le_bytes(),
            &accepted.mint_supply_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
fn close_series_collateral_vault_v5<'a>(
    program_id: &Pubkey,
    preflight: &AuthenticatedSeriesPhysicalRetirementPreflightV5,
    terminal_join: super::SeriesCollateralTerminalJoinV2,
    projection: SeriesFundingTerminalProjectionV5,
    coordinate: super::SeriesCollateralVaultCoordinateV2,
    vault_index: usize,
    vault: &AccountInfo<'a>,
    component_lamport_vault: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    lamport_principal_refund: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> Outcome<ContentId> {
    let binding = super::series_collateral_binding_v2(
        program_id,
        preflight.bound,
        projection.series_plan_id,
        coordinate,
        vault,
        authority,
    )?;
    super::require_lamport_vault_metadata_v2(
        program_id,
        projection.series_plan_id,
        coordinate.component,
        component_lamport_vault,
    )?;
    let request = super::SeriesCollateralVaultCloseRequestV2 {
        terminal: terminal_join,
        component: coordinate.compartment,
        vault: binding,
        component_lamport_vault: super::collateral_id(component_lamport_vault.key),
        stored_vault_rent_principal_lamports:
            preflight.collateral_vault_rent_principal_lamports[vault_index],
        authority: super::TransferAuthorityV2 {
            address: super::collateral_id(authority.key),
            kind: super::TransferAuthorityKindV2::ProgramDerived,
            is_transaction_signer: authority.is_signer,
            program_address_authenticated: true,
            is_writable: authority.is_writable,
            executable: authority.executable,
            data_is_empty: authority.data_is_empty(),
        },
    };
    let vault_before_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let component_before_data = component_lamport_vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let prepared_close = super::prepare_series_collateral_vault_close_v2(
        preflight.bound,
        request,
        super::runtime_lamport_account_view(vault, &vault_before_data),
        super::runtime_lamport_account_view(component_lamport_vault, &component_before_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(vault_before_data);
    drop(component_before_data);
    let cpi = prepared_close.cpi();
    require(
        cpi.token_program == super::collateral_id(token_program.key)
            && cpi.accounts[0].address == super::collateral_id(vault.key)
            && cpi.accounts[1].address == super::collateral_id(component_lamport_vault.key)
            && cpi.accounts[2].address == super::collateral_id(authority.key)
            && cpi.program_signed,
        ClutchError::MismatchedState,
    )?;
    let instruction = Instruction::new_with_bytes(
        *token_program.key,
        &cpi.data,
        cpi.accounts.into_iter().map(super::cpi_account_meta).collect(),
    );
    let series_seed = projection.series_plan_id.bytes();
    let (_, bump) = crate::seeds::series_collateral_authority_pda(program_id, &series_seed);
    let bump_seed = [bump];
    invoke_signed(
        &instruction,
        &[
            vault.clone(),
            component_lamport_vault.clone(),
            authority.clone(),
            token_program.clone(),
        ],
        &[&[
            crate::seeds::SEED_SERIES_COLLATERAL_AUTHORITY_V1,
            &series_seed,
            &bump_seed,
        ]],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    let vault_after_data = vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let component_after_close_data = component_lamport_vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let closed = super::accept_series_collateral_vault_close_v2(
        prepared_close,
        super::runtime_lamport_account_view(vault, &vault_after_data),
        super::runtime_lamport_account_view(component_lamport_vault, &component_after_close_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    drop(vault_after_data);
    drop(component_after_close_data);
    let component_before_split_data = component_lamport_vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let refund_before_data = lamport_principal_refund
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let neutral_before_data = neutral_lamport_sink
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let prepared_disposition = super::prepare_series_vault_rent_disposition_v2(
        closed,
        super::runtime_lamport_account_view(component_lamport_vault, &component_before_split_data),
        super::runtime_lamport_account_view(lamport_principal_refund, &refund_before_data),
        super::runtime_lamport_account_view(neutral_lamport_sink, &neutral_before_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(component_before_split_data);
    drop(refund_before_data);
    drop(neutral_before_data);
    let credits = prepared_disposition.credits();
    super::transfer_from_lamport_custody_v2(
        program_id,
        projection.series_plan_id,
        coordinate.component,
        component_lamport_vault,
        lamport_principal_refund,
        system_program,
        credits[0].lamports,
    )?;
    super::transfer_from_lamport_custody_v2(
        program_id,
        projection.series_plan_id,
        coordinate.component,
        component_lamport_vault,
        neutral_lamport_sink,
        system_program,
        credits[1].lamports,
    )?;
    let component_after_data = component_lamport_vault
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let refund_after_data = lamport_principal_refund
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let neutral_after_data = neutral_lamport_sink
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let accepted = super::accept_series_vault_rent_disposition_v2(
        prepared_disposition,
        super::runtime_lamport_account_view(component_lamport_vault, &component_after_data),
        super::runtime_lamport_account_view(lamport_principal_refund, &refund_after_data),
        super::runtime_lamport_account_view(neutral_lamport_sink, &neutral_after_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    drop(component_after_data);
    drop(refund_after_data);
    drop(neutral_after_data);
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_COLLATERAL_RENT_RETIREMENT_DOMAIN_V5,
            &projection.terminal_receipt_id.bytes(),
            &[coordinate.seed],
            &accepted.component.to_le_bytes(),
            &accepted.closed_vault.bytes(),
            component_lamport_vault.key.as_ref(),
            lamport_principal_refund.key.as_ref(),
            neutral_lamport_sink.key.as_ref(),
            &accepted.component_lamports_after.to_le_bytes(),
            &accepted.refunded_rent_principal_lamports.to_le_bytes(),
            &accepted.neutral_surplus_lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
fn settle_series_lamport_component_v5<'a>(
    program_id: &Pubkey,
    projection: SeriesFundingTerminalProjectionV5,
    component: SeriesFundingComponentV2,
    vault: &AccountInfo<'a>,
    lamport_principal_refund: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
) -> Outcome<ContentId> {
    let index = component.index();
    let principal = projection.refundable_principal[index].lamports;
    let donation = projection.donation_residue[index].lamports;
    let vault_before = vault.lamports();
    let refund_before = lamport_principal_refund.lamports();
    let neutral_before = neutral_lamport_sink.lamports();
    require(
        vault_before
            == principal
                .checked_add(donation)
                .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    super::transfer_from_lamport_custody_v2(
        program_id,
        projection.series_plan_id,
        component,
        vault,
        lamport_principal_refund,
        system_program,
        principal,
    )?;
    super::transfer_from_lamport_custody_v2(
        program_id,
        projection.series_plan_id,
        component,
        vault,
        neutral_lamport_sink,
        system_program,
        donation,
    )?;
    require(
        vault.lamports() == 0
            && *vault.owner == super::SYSTEM_PROGRAM_ID
            && vault.data_is_empty()
            && lamport_principal_refund.lamports()
                == refund_before
                    .checked_add(principal)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            && neutral_lamport_sink.lamports()
                == neutral_before
                    .checked_add(donation)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let component_byte = [super::series_funding_component_seed_v2(component)];
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_LAMPORT_RETIREMENT_DOMAIN_V5,
            &projection.terminal_receipt_id.bytes(),
            &projection.series_plan_id.bytes(),
            &component_byte,
            vault.key.as_ref(),
            lamport_principal_refund.key.as_ref(),
            neutral_lamport_sink.key.as_ref(),
            &vault_before.to_le_bytes(),
            &principal.to_le_bytes(),
            &donation.to_le_bytes(),
            &refund_before.to_le_bytes(),
            &lamport_principal_refund.lamports().to_le_bytes(),
            &neutral_before.to_le_bytes(),
            &neutral_lamport_sink.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(id)
}

fn close_series_funding_program_account_v5(
    program_id: &Pubkey,
    projection: SeriesFundingTerminalProjectionV5,
    funding: &AuthenticatedSeriesFundingAccountV5,
    funding_account: &AccountInfo<'_>,
    lamport_principal_refund: &AccountInfo<'_>,
    neutral_lamport_sink: &AccountInfo<'_>,
) -> Outcome<ContentId> {
    require(
        funding.account() == *funding_account.key
            && funding_account.owner == program_id
            && funding_account.is_writable
            && !funding_account.executable
            && lamport_principal_refund.is_writable
            && neutral_lamport_sink.is_writable
            && funding_account.key != lamport_principal_refund.key
            && funding_account.key != neutral_lamport_sink.key
            && lamport_principal_refund.key != neutral_lamport_sink.key,
        ClutchError::MismatchedState,
    )?;
    let held = funding_account.lamports();
    let principal = funding.value().rent_principal_lamports;
    let donation = held
        .checked_sub(principal)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    let refund_before = lamport_principal_refund.lamports();
    let neutral_before = neutral_lamport_sink.lamports();
    super::credit_lamports(lamport_principal_refund, principal)?;
    super::credit_lamports(neutral_lamport_sink, donation)?;
    super::debit_lamports(funding_account, held)?;
    funding_account
        .resize(0)
        .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    funding_account.assign(&super::SYSTEM_PROGRAM_ID);
    require(
        funding_account.lamports() == 0
            && funding_account.data_len() == 0
            && *funding_account.owner == super::SYSTEM_PROGRAM_ID
            && lamport_principal_refund.lamports()
                == refund_before
                    .checked_add(principal)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?
            && neutral_lamport_sink.lamports()
                == neutral_before
                    .checked_add(donation)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_FUNDING_ACCOUNT_RETIREMENT_DOMAIN_V5,
            &projection.terminal_receipt_id.bytes(),
            funding_account.key.as_ref(),
            &funding.data_id().bytes(),
            &funding.authentication_id().bytes(),
            lamport_principal_refund.key.as_ref(),
            neutral_lamport_sink.key.as_ref(),
            &held.to_le_bytes(),
            &principal.to_le_bytes(),
            &donation.to_le_bytes(),
            &refund_before.to_le_bytes(),
            &lamport_principal_refund.lamports().to_le_bytes(),
            &neutral_before.to_le_bytes(),
            &neutral_lamport_sink.lamports().to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(id)
}

/// Physically retire the exact current FundingV5 graph after Product has
/// sealed the whole Series lifecycle. RegistryV4 is permanent and unchanged;
/// every other writable custody is emptied or closed before this move-only
/// receipt is returned to Product's retirement outer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn retire_current_series_physical_v5<'a>(
    program_id: &Pubkey,
    terminal: AuthenticatedProductSeriesReplayTerminalV5,
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    accounts: &[AccountInfo<'a>],
) -> Outcome<AuthenticatedSeriesPhysicalRetirementV5> {
    let replay_terminal_id = terminal.id();
    let replay_account = terminal.replay().replay().account();
    let replay_authentication_id = terminal.replay().replay().authentication_id();
    let replay_terminal_projection_id = terminal.replay().projection_id();
    let preflight = authenticate_series_physical_retirement_preflight_v5(
        program_id,
        terminal.lifecycle(),
        registry_account,
        funding_account,
        accounts,
    )?;
    let projection = terminal.lifecycle().terminal_projection();
    let projection_id = terminal.lifecycle().terminal_projection_id();
    require(
        preflight.funding.account() == *funding_account.key
            && preflight.registry_account == *registry_account.key
            && preflight.registry_capability_id == terminal.lifecycle().registry().id()
            && preflight.compiler_bundle_id
                == terminal.lifecycle().bundle().bundle_id().content_id()
            && preflight.funding_quote_id
                == terminal.lifecycle()
                    .artifacts()
                    .quote()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .content_id()
            && preflight.attachment_plan_id
                == terminal.lifecycle()
                    .artifacts()
                    .attachment()
                    .id()
                    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                    .content_id()
            && preflight.deployment.release() == preflight.bound.release()
            && !preflight.deployment.receipt_id().is_zero()
            && preflight.funding_rent_principal_lamports
                == preflight.funding.value().rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let collateral_principal_refund = &accounts[IX_RETIRE_COLLATERAL_REFUND_V5];
    let neutral_collateral_disposition = &accounts[IX_RETIRE_NEUTRAL_COLLATERAL_V5];
    let lamport_principal_refund = &accounts[IX_RETIRE_LAMPORT_REFUND_V5];
    let neutral_lamport_sink = &accounts[IX_RETIRE_NEUTRAL_LAMPORT_V5];
    let collateral_authority = &accounts[IX_RETIRE_COLLATERAL_AUTHORITY_V5];
    let mint = &accounts[IX_RETIRE_MINT_V5];
    let token_program = &accounts[IX_RETIRE_TOKEN_PROGRAM_V5];
    let system_program = &accounts[IX_RETIRE_SYSTEM_PROGRAM_V5];
    let rent_sysvar = &accounts[IX_RETIRE_RENT_SYSVAR_V5];
    let lamport_vaults =
        &accounts[IX_RETIRE_LAMPORT_VAULTS_V5..IX_RETIRE_COLLATERAL_VAULTS_V5];
    let collateral_vaults =
        &accounts[IX_RETIRE_COLLATERAL_VAULTS_V5..SERIES_PHYSICAL_RETIREMENT_ACCOUNT_COUNT_V5];
    require(
        preflight.collateral_principal_refund == *collateral_principal_refund.key
            && preflight.neutral_collateral_disposition
                == *neutral_collateral_disposition.key
            && preflight.lamport_principal_refund == *lamport_principal_refund.key
            && preflight.neutral_lamport_sink == *neutral_lamport_sink.key
            && read_rent(rent_sysvar)? == preflight.rent,
        ClutchError::MismatchedState,
    )?;
    require_system_program(system_program)?;
    super::require_collateral_program(token_program, preflight.bound)?;

    let terminal_join = super::SeriesCollateralTerminalJoinV2 {
        funding: preflight.funding_join,
        terminal_receipt: super::CollateralId::from_bytes(projection.terminal_receipt_id.bytes()),
    };
    terminal_join
        .validate(preflight.bound)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let funding_data_before_id = preflight.funding.data_id();
    let funding_authentication_before_id = preflight.funding.authentication_id();
    let lifecycle_terminal_id = terminal.lifecycle().id();
    let mut collateral_principal_receipt_ids =
        [ContentId::ZERO; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut collateral_donation_receipt_ids =
        [ContentId::ZERO; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut collateral_close_receipt_ids =
        [ContentId::ZERO; SERIES_COLLATERAL_VAULT_COUNT_V2];
    let mut vault_index = 0usize;
    while vault_index < SERIES_COLLATERAL_VAULT_COUNT_V2 {
        require(
            preflight.collateral_vaults[vault_index] == *collateral_vaults[vault_index].key,
            ClutchError::MismatchedState,
        )?;
        let coordinate = super::series_collateral_vault_coordinate_v2(vault_index)?;
        collateral_principal_receipt_ids[vault_index] = transfer_series_terminal_collateral_v5(
            program_id,
            &preflight,
            terminal_join,
            projection,
            coordinate,
            SeriesTerminalCollateralMovementV5::PrincipalRefund,
            mint,
            &collateral_vaults[vault_index],
            collateral_principal_refund,
            collateral_authority,
            token_program,
        )?;
        collateral_donation_receipt_ids[vault_index] = transfer_series_terminal_collateral_v5(
            program_id,
            &preflight,
            terminal_join,
            projection,
            coordinate,
            SeriesTerminalCollateralMovementV5::DonationDisposition,
            mint,
            &collateral_vaults[vault_index],
            neutral_collateral_disposition,
            collateral_authority,
            token_program,
        )?;
        let vault_data = collateral_vaults[vault_index]
            .try_borrow_data()
            .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
        let binding = super::series_collateral_binding_v2(
            program_id,
            preflight.bound,
            projection.series_plan_id,
            coordinate,
            &collateral_vaults[vault_index],
            collateral_authority,
        )?;
        let emptied = super::admit_realm_collateral_account_v2(
            preflight.bound,
            super::runtime_account_view(&collateral_vaults[vault_index], &vault_data),
            super::TokenAccountRoleV2::SegregatedVault(binding),
        )
        .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
        require(
            emptied.amount_atoms == 0
                && collateral_principal_receipt_ids[vault_index].is_zero()
                    == (projection.refundable_principal[coordinate.component.index()]
                        .collateral_atoms
                        == 0)
                && collateral_donation_receipt_ids[vault_index].is_zero()
                    == (projection.donation_residue[coordinate.component.index()]
                        .collateral_atoms
                        == 0),
            ClutchError::SeriesCustodyDeltaMismatch,
        )?;
        drop(vault_data);
        collateral_close_receipt_ids[vault_index] = close_series_collateral_vault_v5(
            program_id,
            &preflight,
            terminal_join,
            projection,
            coordinate,
            vault_index,
            &collateral_vaults[vault_index],
            &lamport_vaults[coordinate.component.index()],
            collateral_authority,
            lamport_principal_refund,
            neutral_lamport_sink,
            token_program,
            system_program,
        )?;
        vault_index += 1;
    }

    let mut lamport_retirement_receipt_ids =
        [ContentId::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2];
    let mut component_index = 0usize;
    while component_index < SERIES_FUNDING_COMPONENT_COUNT_V2 {
        require(
            preflight.lamport_vaults[component_index] == *lamport_vaults[component_index].key,
            ClutchError::MismatchedState,
        )?;
        let component = super::series_funding_component_v2(component_index)?;
        lamport_retirement_receipt_ids[component_index] = settle_series_lamport_component_v5(
            program_id,
            projection,
            component,
            &lamport_vaults[component_index],
            lamport_principal_refund,
            neutral_lamport_sink,
            system_program,
        )?;
        component_index += 1;
    }

    let funding_close_receipt_id = close_series_funding_program_account_v5(
        program_id,
        projection,
        &preflight.funding,
        funding_account,
        lamport_principal_refund,
        neutral_lamport_sink,
    )?;
    let registry_after = authenticate_series_registry_account_v4(
        program_id,
        registry_account,
        projection.series_plan_id,
        false,
    )?;
    require(
        registry_after.account() == preflight.registry_account
            && registry_after.data_id() == preflight.registry_data_id
            && registry_after.authentication_id() == preflight.registry_authentication_id
            && registry_after.observed_lamports() == preflight.registry_observed_lamports
            && registry_after.authentication_id()
                == terminal
                    .lifecycle()
                    .registry()
                    .series_registry_authentication_id(),
        ClutchError::Replay,
    )?;
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    super::admit_realm_collateral_mint_v2(
        preflight.bound,
        super::runtime_account_view(mint, &mint_data),
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    drop(mint_data);
    let refund_after_data = collateral_principal_refund
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    super::admit_realm_collateral_account_v2(
        preflight.bound,
        super::runtime_account_view(collateral_principal_refund, &refund_after_data),
        super::TokenAccountRoleV2::ReceiveOnly {
            account: super::collateral_id(collateral_principal_refund.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    let collateral_principal_refund_after_id = super::series_collateral_account_state_id_v2(
        SERIES_TERMINAL_COLLATERAL_TRANSFER_DOMAIN_V5,
        collateral_principal_refund,
        &refund_after_data,
    )?;
    drop(refund_after_data);
    let neutral_after_data = neutral_collateral_disposition
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    super::admit_realm_collateral_account_v2(
        preflight.bound,
        super::runtime_account_view(neutral_collateral_disposition, &neutral_after_data),
        super::TokenAccountRoleV2::ReceiveOnly {
            account: super::collateral_id(neutral_collateral_disposition.key),
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    let neutral_collateral_disposition_after_id = super::series_collateral_account_state_id_v2(
        SERIES_TERMINAL_COLLATERAL_TRANSFER_DOMAIN_V5,
        neutral_collateral_disposition,
        &neutral_after_data,
    )?;
    drop(neutral_after_data);

    let funding_commitment_id = super::series_terminal_funding_commitment_v2(
        &projection.refundable_principal,
        &projection.donation_residue,
    )?;
    let principal_receipts =
        super::flatten_series_collateral_ids_v2(&collateral_principal_receipt_ids)?;
    let donation_receipts =
        super::flatten_series_collateral_ids_v2(&collateral_donation_receipt_ids)?;
    let close_receipts = super::flatten_series_collateral_ids_v2(&collateral_close_receipt_ids)?;
    let lamport_receipts =
        super::flatten_series_component_receipt_ids_v2(&lamport_retirement_receipt_ids)?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SERIES_PHYSICAL_RETIREMENT_DOMAIN_V5,
            program_id.as_ref(),
            &preflight.id.bytes(),
            &lifecycle_terminal_id.bytes(),
            &replay_terminal_id.bytes(),
            replay_account.as_ref(),
            &replay_authentication_id.bytes(),
            &replay_terminal_projection_id.bytes(),
            &projection_id.bytes(),
            &funding_commitment_id.bytes(),
            &preflight.deployment.receipt_id().bytes(),
            registry_account.key.as_ref(),
            &registry_after.data_id().bytes(),
            &registry_after.authentication_id().bytes(),
            funding_account.key.as_ref(),
            &funding_data_before_id.bytes(),
            &funding_authentication_before_id.bytes(),
            &funding_close_receipt_id.bytes(),
            &preflight.collateral_principal_refund_prestate_id.bytes(),
            &collateral_principal_refund_after_id.bytes(),
            &preflight.neutral_collateral_disposition_prestate_id.bytes(),
            &neutral_collateral_disposition_after_id.bytes(),
            &principal_receipts,
            &donation_receipts,
            &close_receipts,
            &lamport_receipts,
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSeriesPhysicalRetirementV5 {
        id,
        lifecycle_terminal_id,
        replay_terminal_id,
        replay_account,
        replay_authentication_id,
        replay_terminal_projection_id,
        terminal_projection: projection,
        terminal_projection_id: projection_id,
        registry_account: registry_after.account(),
        registry_data_id: registry_after.data_id(),
        registry_authentication_id: registry_after.authentication_id(),
        funding_account: preflight.funding.account(),
        funding_data_before_id,
        funding_authentication_before_id,
        funding_close_receipt_id,
        lamport_principal_refund: *lamport_principal_refund.key,
        collateral_principal_refund: *collateral_principal_refund.key,
        neutral_collateral_disposition: *neutral_collateral_disposition.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
        collateral_principal_refund_before_id:
            preflight.collateral_principal_refund_prestate_id,
        collateral_principal_refund_after_id,
        neutral_collateral_disposition_before_id:
            preflight.neutral_collateral_disposition_prestate_id,
        neutral_collateral_disposition_after_id,
        lamport_retirement_receipt_ids,
        collateral_principal_receipt_ids,
        collateral_donation_receipt_ids,
        collateral_close_receipt_ids,
    })
}

#[cfg(test)]
mod source_invariants {
    #[test]
    fn v5_registration_is_acyclic_move_only_and_hostile_reopened() {
        let source = include_str!("physical_v5.rs");
        let receipt = source
            .split_once("pub(crate) struct AuthenticatedSeriesPhysicalRegistrationV5")
            .expect("physical registration receipt")
            .1
            .split_once("impl AuthenticatedSeriesPhysicalRegistrationV5")
            .expect("bounded registration receipt")
            .0;
        assert!(!receipt.contains("Clone"));
        assert!(!receipt.contains("Copy"));
        let writer = source
            .split_once("pub(crate) fn register_current_series_physical_v5")
            .expect("current physical registration writer")
            .1
            .split_once("/// Consume the sole current Series activation bit")
            .expect("bounded registration writer")
            .0;
        let create = writer
            .find("super::create_series_program_account(")
            .expect("RegistryV4 physical create");
        let encode = writer.find("value.encode(").expect("RegistryV4 encode");
        let reopen = writer
            .find("authenticate_series_registry_account_v4(")
            .expect("RegistryV4 hostile reopen");
        let release = writer
            .find("authenticate_registry_capability_v5(")
            .expect("RegistryCapabilityV5 release join");
        assert!(create < encode && encode < reopen && reopen < release);
        assert!(writer.contains("registry_prefund_donation_lamports"));
        assert!(writer.contains("registration.lamport_principal_refund()"));
        assert!(writer.contains("registration.neutral_lamport_sink()"));
        assert!(!writer.contains("SeriesRegistryAccountV3"));
        assert!(!writer.contains("AuthenticatedRegistryCapabilityV4"));
    }

    #[test]
    fn v5_physical_authority_is_fresh_and_move_only() {
        let source = include_str!("physical_v5.rs");
        let production = source
            .split_once("#[cfg(test)]")
            .expect("bounded V5 production source")
            .0;
        let receipt = production
            .split("pub(crate) struct AuthenticatedSeriesPhysicalCapitalizationV5")
            .nth(1)
            .and_then(|value| value.split("impl AuthenticatedSeriesPhysicalCapitalizationV5").next())
            .expect("bounded V5 capitalization receipt");
        assert!(!receipt.contains("Clone"));
        assert!(!receipt.contains("Copy"));
        for current in [
            "registry_capability_id",
            "compiler_bundle_id",
            "funding_quote_id",
            "foundation_schedule_id",
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
        assert!(!production.contains("AuthenticatedSeriesFundingAccountV2"));
        assert!(!production.contains("CompiledProductSeriesBundleV6"));
        assert!(!production.contains("SeriesFundingQuoteV5"));
        assert!(!production.contains("SeriesAttachmentPlanV5"));
        assert!(production.contains("CompiledProductSeriesBundleV7"));
        assert!(production.contains("SeriesFundingQuoteV6"));
        assert!(production.contains("SeriesAttachmentPlanV6"));
        assert!(production.contains("SERIES_FUNDING_COMPONENT_COUNT_V2"));
        assert!(production.contains("SERIES_COLLATERAL_VAULT_COUNT_V2"));
    }

    #[test]
    fn account_suffix_has_exact_six_plus_five_vault_geometry() {
        assert_eq!(super::IX_PHYSICAL_REALM_V5, 7);
        assert_eq!(super::IX_PHYSICAL_COLLATERAL_PROFILE_V5, 8);
        assert_eq!(super::IX_PHYSICAL_COLLATERAL_POLICY_V5, 9);
        assert_eq!(super::IX_PHYSICAL_LAMPORT_VAULTS_V5, 15);
        assert_eq!(super::IX_PHYSICAL_COLLATERAL_VAULTS_V5, 21);
        assert_eq!(super::SERIES_PHYSICAL_CAPITALIZATION_ACCOUNT_COUNT_V5, 26);
    }

    #[test]
    fn retirement_suffix_is_exact_and_move_only() {
        assert_eq!(super::IX_RETIRE_COLLATERAL_REFUND_V5, 0);
        assert_eq!(super::IX_RETIRE_NEUTRAL_COLLATERAL_V5, 1);
        assert_eq!(super::IX_RETIRE_LAMPORT_REFUND_V5, 2);
        assert_eq!(super::IX_RETIRE_NEUTRAL_LAMPORT_V5, 3);
        assert_eq!(super::IX_RETIRE_REALM_V5, 5);
        assert_eq!(super::IX_RETIRE_TOKEN_PROGRAMDATA_V5, 10);
        assert_eq!(super::IX_RETIRE_LAMPORT_VAULTS_V5, 13);
        assert_eq!(super::IX_RETIRE_COLLATERAL_VAULTS_V5, 19);
        assert_eq!(super::SERIES_PHYSICAL_RETIREMENT_ACCOUNT_COUNT_V5, 24);

        let source = include_str!("physical_v5.rs");
        let receipt = source
            .split_once("pub(crate) struct AuthenticatedSeriesPhysicalRetirementV5")
            .expect("physical retirement receipt")
            .1
            .split_once("impl AuthenticatedSeriesPhysicalRetirementV5")
            .expect("bounded physical retirement receipt")
            .0;
        assert!(!receipt.contains("Clone"));
        assert!(!receipt.contains("Copy"));
        for fact in [
            "lifecycle_terminal_id",
            "terminal_projection_id",
            "registry_authentication_id",
            "funding_authentication_before_id",
            "funding_close_receipt_id",
            "collateral_principal_refund_before_id",
            "collateral_principal_refund_after_id",
            "neutral_collateral_disposition_before_id",
            "neutral_collateral_disposition_after_id",
            "lamport_retirement_receipt_ids",
            "collateral_close_receipt_ids",
        ] {
            assert!(receipt.contains(fact), "missing {fact}");
        }
    }

    #[test]
    fn retirement_orders_collateral_then_lamports_then_funding_close() {
        let source = include_str!("physical_v5.rs");
        let writer = source
            .split_once("pub(crate) fn retire_current_series_physical_v5")
            .expect("current physical retirement writer")
            .1
            .split_once("#[cfg(test)]")
            .expect("bounded current physical retirement writer")
            .0;
        let collateral_transfer = writer
            .find("transfer_series_terminal_collateral_v5(")
            .expect("collateral terminal transfer");
        let collateral_close = writer
            .find("close_series_collateral_vault_v5(")
            .expect("collateral vault close");
        let lamport_close = writer
            .find("settle_series_lamport_component_v5(")
            .expect("lamport component close");
        let funding_close = writer
            .find("close_series_funding_program_account_v5(")
            .expect("FundingV5 close");
        let registry_reopen = writer
            .find("authenticate_series_registry_account_v4(")
            .expect("RegistryV4 hostile reopen");
        assert!(collateral_transfer < collateral_close);
        assert!(collateral_close < lamport_close);
        assert!(lamport_close < funding_close);
        assert!(funding_close < registry_reopen);
        assert!(writer.contains("AuthenticatedProductSeriesLifecycleTerminalV5"));
        assert!(!writer.contains("AuthenticatedProductSeriesLifecycleTerminalV4"));
    }
}
