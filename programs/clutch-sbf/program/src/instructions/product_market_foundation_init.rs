//! Staged physical capitalization of one Product FoundationVault.
//!
//! This module owns only the first custody movement for a founder: the exact
//! pending MarketCore principal moves from the payer-funded Series component
//! vault into the canonical, zero-data FoundationVault.  It does not create a
//! MarketLifecycleRoot, create a SeriesMarketLink, reserve an ordinal, or
//! enable an instruction route.  A later atomic founder composer must consume
//! the private receipt below while creating those accounts; otherwise this
//! helper remains unreachable because its authority trait refuses by default.
//!
//! Existing lamports at either PDA are never credited as payer principal.  The
//! Series state owns its component donation balance, and the FoundationVault
//! prebalance becomes a separate donation floor eventually payable only to the
//! immutable FundingTerms neutral sink.  Hoard, collateral, future fees, and
//! shortfall funding have no representation in this contract.

use crate::accounts::{expect_pda, require, require_distinct, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    allocate_data, assign_data, read_rent, require_system_program, transfer_data,
    SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedRegistryCapabilityV3,
};
use crate::instructions::product_market::{
    authenticate_market_lifecycle_root_v1, authenticate_series_market_link_v1,
    require_canonical_market_foundation_core_v2,
};
use crate::instructions::product_series::{
    authenticate_series_artifact_accounts_v4, read_series_funding_account_v2,
    read_series_registry_account_v2,
    AuthenticatedCompiledProductSeriesBundleV5, AuthenticatedSeriesArtifactsV4,
    AuthenticatedSeriesFundingAccountV2, AuthenticatedSeriesRegistryAccountV2,
};
use crate::seeds;
use clutch_liveness::runtime_adapter_v1::{
    decode_runtime_policy_account_v1, RuntimePersistedAccountViewV1,
};
use clutch_liveness::runtime_v1::{
    PresentFundingSourceV1, PresentFundingV1, RuntimeCompartmentAdmissionV1,
    RuntimeCompartmentIdentityV1, RuntimeCompartmentKindV1, RuntimeCompartmentV1,
};
use clutch_liveness::Id as LivenessId;
use clutch_product_series::{
    ComponentDebitV1, ContentId, MarketFoundationAccountGraphV2,
    MarketFoundationAccountGraphV2Id, MarketFoundationCapitalV1, MarketFoundationScheduleV2,
    MarketFoundationScheduleV2Id, MarketFoundationSlotV2, MarketFamilyAggregatorV1,
    MarketInstanceV2Id, MarketLifecycleBindingV1, MarketLifecycleRootV1,
    SeriesFundingComponentV2, SeriesFundingPhaseV2, SeriesFundingQuoteV4,
    SeriesFundingTermsV2, SeriesLinkObligationConfigurationV1,
    SeriesMarketAdmissionProjectionV1, SeriesMarketDispositionV1, SeriesMarketLinkBindingV1,
    SeriesMarketLinkV1, SeriesMarketLinkV1Id, SeriesPlanV5Id,
    SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::failure_recovery::{
    decode_failure_account_body_v1, encode_failure_account_header_v1,
    FAILURE_ACCOUNT_HEADER_BYTES_V1, FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1,
    FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1, FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
    FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
};
use clutch_solana_layout::product_series::{
    MarketLifecycleRootAccountV1, SeriesFundingAccountV2, SeriesMarketLinkAccountV1,
    MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1, SERIES_FUNDING_ACCOUNT_BYTES_V2,
    SERIES_MARKET_LINK_ACCOUNT_BYTES_V1,
};
use clutch_solana_layout::registry;
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const FOUNDATION_VAULT_INIT_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-foundation-vault-init-receipt/v1";
const FOUNDATION_FUNDING_ACCOUNT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-foundation-funding-account-authentication/v1";
const RECOVERY_RESERVE_CAPITALIZATION_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-recovery-reserve-capitalization-receipt/v1";
const PRODUCT_MARKET_FOUNDER_ROOT_POSTSTATE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-market-founder-root-poststate/v1";
const PRODUCT_MARKET_FOUNDER_CREATION_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-market-founder-creation-receipt/v1";
const MARKET_CORE_COMPONENT_SEED_V2: u8 = 0;
const SERIES_ADMISSION_COMPONENT_SEED_V2: u8 = 1;
const RECOVERY_RESERVE_COMPONENT_SEED_V2: u8 = 2;

/// Product-owned coordinates which a future root/link creator must authorize.
///
/// This is a projection, not authority.  The default-refusing trait below is
/// the only way it can authorize a custody movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FoundationVaultInitCoordinatesV1 {
    pub(crate) market_instance_id: MarketInstanceV2Id,
    pub(crate) generation: u64,
    pub(crate) founder_link_id: SeriesMarketLinkV1Id,
    pub(crate) lifecycle_root_account: Pubkey,
    pub(crate) founder_link_account: Pubkey,
    pub(crate) lifecycle_replay_account: Pubkey,
    pub(crate) foundation_account_graph_id: MarketFoundationAccountGraphV2Id,
    /// Exact lifecycle shared by the existing liveness policy and Recovery custody.
    pub(crate) liveness_lifecycle_id: ContentId,
}

/// Exact immutable facts offered to the future atomic founder authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FoundationVaultInitAuthorizationFactsV1 {
    pub(crate) coordinates: FoundationVaultInitCoordinatesV1,
    pub(crate) series_plan_id: SeriesPlanV5Id,
    pub(crate) ordinal: u32,
    pub(crate) funding_state_account: Pubkey,
    pub(crate) funding_state_id: ContentId,
    pub(crate) funding_account_data_id: ContentId,
    pub(crate) funding_account_authentication_id: ContentId,
    pub(crate) funding_transition_sequence: u64,
    pub(crate) funding_reservation_receipt_id: ContentId,
    pub(crate) pending_debits: [ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
    pub(crate) compiler_bundle_id: ContentId,
    pub(crate) registry_release_id: ContentId,
    pub(crate) capability_profile_id: ContentId,
    pub(crate) funding_terms_id: ContentId,
    pub(crate) funding_quote_id: ContentId,
    pub(crate) foundation_schedule_id: MarketFoundationScheduleV2Id,
    pub(crate) market_core_vault: Pubkey,
    pub(crate) foundation_vault: Pubkey,
    pub(crate) principal_refund_owner: Pubkey,
    pub(crate) neutral_lamport_sink: Pubkey,
    pub(crate) principal_lamports: u64,
    pub(crate) series_vault_donation_lamports: u64,
    pub(crate) foundation_vault_donation_lamports: u64,
    pub(crate) series_vault_balance_before: u64,
    pub(crate) series_vault_balance_after: u64,
    pub(crate) foundation_vault_balance_before: u64,
    pub(crate) foundation_vault_balance_after: u64,
}

/// Semantic owner for a proposed initial FoundationVault movement.
///
/// The eventual implementation must be private to the atomic creator of the
/// exact `0xaa/1` root and founder `0xad/1` link.  A decoded binding, caller
/// intent, or content ID alone must never implement this trait.  It must
/// compare every active slot of `account_graph` to the canonical family-owned
/// PDA/body source under `schedule`; equality of the recomputed graph ID alone
/// is deliberately insufficient authority.
pub(crate) trait AuthenticatedFoundationVaultInitAuthorityV1 {
    fn authenticate_foundation_vault_init_v1(
        &self,
        _facts: &FoundationVaultInitAuthorizationFactsV1,
        _schedule: &MarketFoundationScheduleV2,
        _account_graph: &MarketFoundationAccountGraphV2,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private proof of the exact initial payer-principal custody movement.
///
/// Fields are private so neither an instruction payload nor a decoded account
/// can mint this authority.  The future root initializer may consume getters
/// to construct `MarketFoundationCapitalV1` and must do so atomically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedFoundationVaultInitV1 {
    id: ContentId,
    facts: FoundationVaultInitAuthorizationFactsV1,
    series_registry_account: Pubkey,
    executing_program: Pubkey,
    executing_programdata: Pubkey,
    programdata_sha256: ContentId,
    release_artifact_account: Pubkey,
    profile_artifact_account: Pubkey,
    compiler_bundle_artifact_account: Pubkey,
    funding_terms_artifact_account: Pubkey,
    funding_quote_artifact_account: Pubkey,
    rent_sysvar: Pubkey,
    rent_lamports_per_byte_year: u64,
    rent_exemption_threshold_bits: u64,
    funding_account_rent_principal_lamports: u64,
    funding_account_observed_lamports: u64,
}

impl AuthenticatedFoundationVaultInitV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn facts(self) -> FoundationVaultInitAuthorizationFactsV1 {
        self.facts
    }

    pub(crate) const fn principal_lamports(self) -> u64 {
        self.facts.principal_lamports
    }

    pub(crate) const fn foundation_vault(self) -> Pubkey {
        self.facts.foundation_vault
    }

    pub(crate) const fn principal_refund_owner(self) -> Pubkey {
        self.facts.principal_refund_owner
    }

    pub(crate) const fn neutral_lamport_sink(self) -> Pubkey {
        self.facts.neutral_lamport_sink
    }

    pub(crate) const fn foundation_vault_donation_lamports(self) -> u64 {
        self.facts.foundation_vault_donation_lamports
    }

    pub(crate) const fn series_vault_balance_after(self) -> u64 {
        self.facts.series_vault_balance_after
    }

    pub(crate) const fn foundation_vault_balance_after(self) -> u64 {
        self.facts.foundation_vault_balance_after
    }
}

/// Exact present-funding and physical-account facts for the sole Recovery custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryReserveCapitalizationFactsV1 {
    pub(crate) foundation_init_receipt_id: ContentId,
    pub(crate) series_plan_id: SeriesPlanV5Id,
    pub(crate) market_instance_id: MarketInstanceV2Id,
    pub(crate) generation: u64,
    pub(crate) funding_account: Pubkey,
    pub(crate) funding_state_id: ContentId,
    pub(crate) funding_transition_sequence: u64,
    pub(crate) funding_reservation_receipt_id: ContentId,
    pub(crate) recovery_reserve_vault: Pubkey,
    pub(crate) recovery_account: Pubkey,
    pub(crate) liveness_policy_account: Pubkey,
    pub(crate) liveness_policy_id: ContentId,
    pub(crate) liveness_realm_id: ContentId,
    pub(crate) liveness_lifecycle_id: ContentId,
    pub(crate) quote_schedule_id: ContentId,
    pub(crate) payer: Pubkey,
    pub(crate) neutral_lamport_sink: Pubkey,
    pub(crate) work_principal_lamports: u64,
    pub(crate) rent_principal_lamports: u64,
    pub(crate) payer_debit_lamports: u64,
    pub(crate) source_donation_lamports: u64,
    pub(crate) recovery_donation_lamports: u64,
    pub(crate) source_balance_before: u64,
    pub(crate) source_balance_after: u64,
    pub(crate) recovery_balance_before: u64,
    pub(crate) recovery_balance_after: u64,
}

/// Private proof that Product moved the complete pending RecoveryReserve debit.
///
/// The receipt is minted only after the framed liveness body is written and
/// hostile-decoded from the canonical Recovery PDA. Failure consumes this
/// receipt as the sole custody fact; it must not move a second reward reserve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedRecoveryReserveCapitalizationV1 {
    id: ContentId,
    facts: RecoveryReserveCapitalizationFactsV1,
    recovery_state: RuntimeCompartmentV1,
    recovery_data_id: ContentId,
}

impl AuthenticatedRecoveryReserveCapitalizationV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn facts(self) -> RecoveryReserveCapitalizationFactsV1 {
        self.facts
    }

    pub(crate) const fn recovery_state(self) -> RuntimeCompartmentV1 {
        self.recovery_state
    }

    pub(crate) const fn recovery_data_id(self) -> ContentId {
        self.recovery_data_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RecoveryReserveFundingPlanV1 {
    work_principal_lamports: u64,
    rent_principal_lamports: u64,
    payer_debit_lamports: u64,
    source_donation_lamports: u64,
    recovery_donation_lamports: u64,
    source_balance_before: u64,
    source_balance_after: u64,
    recovery_balance_before: u64,
    recovery_balance_after: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingNativeComponentPlanV1 {
    payer_debit_lamports: u64,
    source_donation_lamports: u64,
    destination_donation_lamports: u64,
    source_balance_before: u64,
    source_balance_after: u64,
    destination_balance_before: u64,
    destination_balance_after: u64,
}

/// Product-owned semantic inputs for the first shared Market root and link.
///
/// These values are projections until a private implementation of
/// [`AuthenticatedProductMarketFounderCreationAuthorityV1`] authenticates the
/// exact Source, Failure, General, collateral, Registry, and capability
/// owners from which every field was derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductMarketFounderSemanticV1 {
    pub(crate) market_binding: MarketLifecycleBindingV1,
    pub(crate) founder_link_binding: SeriesMarketLinkBindingV1,
    pub(crate) obligation_configuration: SeriesLinkObligationConfigurationV1,
    pub(crate) product_families: MarketFamilyAggregatorV1,
}

/// Exact physical facts offered to the sole founder-creation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductMarketFounderCreationFactsV1 {
    pub(crate) foundation_init_receipt_id: ContentId,
    pub(crate) recovery_capitalization_receipt_id: ContentId,
    pub(crate) registry_account: Pubkey,
    pub(crate) funding_account: Pubkey,
    pub(crate) root_account: Pubkey,
    pub(crate) founder_link_account: Pubkey,
    pub(crate) foundation_vault: Pubkey,
    pub(crate) series_admission_vault: Pubkey,
    pub(crate) recovery_account: Pubkey,
    pub(crate) active_foundation_accounts: u8,
    pub(crate) root_rent_principal_lamports: u64,
    pub(crate) root_donation_lamports: u64,
    pub(crate) foundation_balance_before: u64,
    pub(crate) foundation_balance_after: u64,
    pub(crate) link_rent_principal_lamports: u64,
    pub(crate) link_donation_lamports: u64,
    pub(crate) series_admission_balance_before: u64,
    pub(crate) series_admission_balance_after: u64,
    pub(crate) root_poststate_receipt_id: ContentId,
}

/// Default-refusing owner for the complete first-root semantic join.
pub(crate) trait AuthenticatedProductMarketFounderCreationAuthorityV1 {
    fn authenticate_product_market_founder_creation_v1(
        &self,
        _facts: &ProductMarketFounderCreationFactsV1,
        _semantic: &ProductMarketFounderSemanticV1,
        _schedule: &MarketFoundationScheduleV2,
        _account_graph: &MarketFoundationAccountGraphV2,
    ) -> Outcome<()> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private postwrite proof for the one atomic `0xaa/1` + founder `0xad/1`
/// creation. Funding remains Pending and the link remains PendingMarket until
/// the phased Foundation schedule is fully accepted and Product activates the
/// root/link pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedProductMarketFounderCreationV1 {
    id: ContentId,
    facts: ProductMarketFounderCreationFactsV1,
    root_semantic_id: ContentId,
    root_data_id: ContentId,
    root_authentication_id: ContentId,
    link_semantic_id: SeriesMarketLinkV1Id,
    link_data_id: ContentId,
    link_authentication_id: ContentId,
    market_admission_receipt_id: ContentId,
}

impl AuthenticatedProductMarketFounderCreationV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn facts(self) -> ProductMarketFounderCreationFactsV1 {
        self.facts
    }

    pub(crate) const fn root_semantic_id(self) -> ContentId {
        self.root_semantic_id
    }

    pub(crate) const fn root_authentication_id(self) -> ContentId {
        self.root_authentication_id
    }

    pub(crate) const fn link_semantic_id(self) -> SeriesMarketLinkV1Id {
        self.link_semantic_id
    }

    pub(crate) const fn link_authentication_id(self) -> ContentId {
        self.link_authentication_id
    }

    pub(crate) const fn market_admission_receipt_id(self) -> ContentId {
        self.market_admission_receipt_id
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_recovery_reserve_funding_v1(
    quoted: ComponentDebitV1,
    pending: ComponentDebitV1,
    remaining: ComponentDebitV1,
    source_donations: ComponentDebitV1,
    work_principal_lamports: u64,
    rent_principal_lamports: u64,
    source_balance_before: u64,
    recovery_balance_before: u64,
) -> Outcome<RecoveryReserveFundingPlanV1> {
    let payer_debit_lamports = work_principal_lamports
        .checked_add(rent_principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        work_principal_lamports != 0
            && rent_principal_lamports != 0
            && quoted.lamports == payer_debit_lamports,
        ClutchError::MismatchedState,
    )?;
    let movement = plan_pending_native_component_v1(
        quoted,
        pending,
        remaining,
        source_donations,
        payer_debit_lamports,
        source_balance_before,
        recovery_balance_before,
    )?;
    Ok(RecoveryReserveFundingPlanV1 {
        work_principal_lamports,
        rent_principal_lamports,
        payer_debit_lamports,
        source_donation_lamports: movement.source_donation_lamports,
        recovery_donation_lamports: movement.destination_donation_lamports,
        source_balance_before: movement.source_balance_before,
        source_balance_after: movement.source_balance_after,
        recovery_balance_before: movement.destination_balance_before,
        recovery_balance_after: movement.destination_balance_after,
    })
}

#[allow(clippy::too_many_arguments)]
fn plan_pending_native_component_v1(
    quoted: ComponentDebitV1,
    pending: ComponentDebitV1,
    remaining: ComponentDebitV1,
    source_donations: ComponentDebitV1,
    payer_debit_lamports: u64,
    source_balance_before: u64,
    destination_balance_before: u64,
) -> Outcome<PendingNativeComponentPlanV1> {
    require(
        payer_debit_lamports != 0
            && quoted.collateral_atoms == 0
            && quoted.lamports == payer_debit_lamports
            && pending == quoted
            && remaining.collateral_atoms == 0
            && source_donations.collateral_atoms == 0,
        ClutchError::MismatchedState,
    )?;
    let accounted_source_before = remaining
        .lamports
        .checked_add(source_donations.lamports)
        .and_then(|value| value.checked_add(pending.lamports))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        source_balance_before == accounted_source_before,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let source_balance_after = source_balance_before
        .checked_sub(payer_debit_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_source_after = remaining
        .lamports
        .checked_add(source_donations.lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        source_balance_after == expected_source_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let destination_balance_after = destination_balance_before
        .checked_add(payer_debit_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(PendingNativeComponentPlanV1 {
        payer_debit_lamports,
        source_donation_lamports: source_donations.lamports,
        destination_donation_lamports: destination_balance_before,
        source_balance_before,
        source_balance_after,
        destination_balance_before,
        destination_balance_after,
    })
}

fn flatten_component_debits_v1(
    debits: &[ComponentDebitV1; SERIES_FUNDING_COMPONENT_COUNT_V2],
) -> [u8; SERIES_FUNDING_COMPONENT_COUNT_V2 * 16] {
    let mut output = [0u8; SERIES_FUNDING_COMPONENT_COUNT_V2 * 16];
    let mut index = 0usize;
    while index < debits.len() {
        let at = index * 16;
        output[at..at + 8].copy_from_slice(&debits[index].lamports.to_le_bytes());
        output[at + 8..at + 16]
            .copy_from_slice(&debits[index].collateral_atoms.to_le_bytes());
        index += 1;
    }
    output
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FoundationVaultFundingPrestateV1 {
    quoted_market_core: ComponentDebitV1,
    pending_market_core: ComponentDebitV1,
    remaining_market_core: ComponentDebitV1,
    series_component_donations: ComponentDebitV1,
    observed_series_vault_lamports: u64,
    observed_foundation_vault_lamports: u64,
    funding_account_rent_principal_lamports: u64,
    funding_account_minimum_balance_lamports: u64,
    funding_account_observed_lamports: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FoundationVaultFundingPlanV1 {
    principal_lamports: u64,
    series_vault_donation_lamports: u64,
    series_vault_balance_before: u64,
    series_vault_balance_after: u64,
    foundation_vault_donation_lamports: u64,
    foundation_vault_balance_before: u64,
    foundation_vault_balance_after: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FoundationCapabilityBundleJoinV1 {
    expected_program: Pubkey,
    executing_program: Pubkey,
    registered_series_plan_id: SeriesPlanV5Id,
    bundle_series_plan_id: SeriesPlanV5Id,
    registered_compiler_bundle_id: ContentId,
    compiler_bundle_id: ContentId,
    capability_registry_release_id: ContentId,
    bundle_registry_release_id: ContentId,
    capability_profile_id: ContentId,
    bundle_capability_profile_id: ContentId,
    programdata_sha256: ContentId,
}

fn authenticate_foundation_capability_bundle_join_v1(
    join: FoundationCapabilityBundleJoinV1,
) -> Outcome<()> {
    require(
        join.executing_program == join.expected_program
            && join.registered_series_plan_id == join.bundle_series_plan_id
            && join.registered_compiler_bundle_id == join.compiler_bundle_id
            && join.capability_registry_release_id == join.bundle_registry_release_id
            && join.capability_profile_id == join.bundle_capability_profile_id
            && !join.programdata_sha256.is_zero(),
        ClutchError::MismatchedState,
    )
}

/// Derive exact pre/post balances without treating either prefund as principal.
fn plan_foundation_vault_funding_v1(
    prestate: FoundationVaultFundingPrestateV1,
) -> Outcome<FoundationVaultFundingPlanV1> {
    require(
        prestate.quoted_market_core.lamports != 0
            && prestate.quoted_market_core.collateral_atoms == 0
            && prestate.pending_market_core == prestate.quoted_market_core
            && prestate.remaining_market_core.collateral_atoms == 0
            && prestate.funding_account_rent_principal_lamports != 0
            && prestate.funding_account_observed_lamports
                >= prestate.funding_account_rent_principal_lamports
            && prestate.funding_account_observed_lamports
                >= prestate.funding_account_minimum_balance_lamports,
        ClutchError::MismatchedState,
    )?;
    let accounted_series_before = prestate
        .remaining_market_core
        .lamports
        .checked_add(prestate.series_component_donations.lamports)
        .and_then(|value| value.checked_add(prestate.pending_market_core.lamports))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        prestate.observed_series_vault_lamports == accounted_series_before,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let series_vault_balance_after = prestate
        .observed_series_vault_lamports
        .checked_sub(prestate.pending_market_core.lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_series_after = prestate
        .remaining_market_core
        .lamports
        .checked_add(prestate.series_component_donations.lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        series_vault_balance_after == expected_series_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let foundation_vault_balance_after = prestate
        .observed_foundation_vault_lamports
        .checked_add(prestate.pending_market_core.lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    Ok(FoundationVaultFundingPlanV1 {
        principal_lamports: prestate.pending_market_core.lamports,
        series_vault_donation_lamports: prestate.series_component_donations.lamports,
        series_vault_balance_before: prestate.observed_series_vault_lamports,
        series_vault_balance_after,
        foundation_vault_donation_lamports: prestate.observed_foundation_vault_lamports,
        foundation_vault_balance_before: prestate.observed_foundation_vault_lamports,
        foundation_vault_balance_after,
    })
}

/// Capitalize the canonical FoundationVault from one exact pending founder debit.
///
/// The capability and bundle arguments are private-field receipts minted only
/// after loader ProgramData/ELF, ReleaseV2, ProfileV4, Source release, and the
/// complete Product graph were hostile-authenticated.  FundingTerms and
/// QuoteV4 are reopened here so payout ownership and the 46-slot itemization
/// cannot be substituted after those receipts were minted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn fund_product_foundation_vault_v1<
    'a,
    A: AuthenticatedFoundationVaultInitAuthorityV1 + ?Sized,
>(
    program_id: &Pubkey,
    authority: &A,
    coordinates: FoundationVaultInitCoordinatesV1,
    capability: AuthenticatedRegistryCapabilityV3,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    funding: AuthenticatedSeriesFundingAccountV2,
    foundation_account_graph: &MarketFoundationAccountGraphV2,
    funding_account: &AccountInfo<'a>,
    funding_terms_account: &AccountInfo<'a>,
    funding_quote_account: &AccountInfo<'a>,
    market_core_vault: &AccountInfo<'a>,
    foundation_vault: &AccountInfo<'a>,
    principal_refund_owner: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> Outcome<AuthenticatedFoundationVaultInitV1> {
    require_system_program(system_program)?;
    let rent = read_rent(rent_sysvar)?;
    require_distinct(&[
        funding_account.clone(),
        funding_terms_account.clone(),
        funding_quote_account.clone(),
        market_core_vault.clone(),
        foundation_vault.clone(),
        principal_refund_owner.clone(),
        neutral_lamport_sink.clone(),
        system_program.clone(),
        rent_sysvar.clone(),
    ])?;

    let bundle = compiler_bundle.bundle();
    let compiler_bundle_id = compiler_bundle.bundle_id().content_id();
    authenticate_foundation_capability_bundle_join_v1(FoundationCapabilityBundleJoinV1 {
        expected_program: *program_id,
        executing_program: capability.program_account(),
        registered_series_plan_id: capability.series_plan_id(),
        bundle_series_plan_id: bundle.series_plan_id,
        registered_compiler_bundle_id: capability.compiler_bundle_id(),
        compiler_bundle_id,
        capability_registry_release_id: capability.registry_release_id(),
        bundle_registry_release_id: bundle.registry_release_id,
        capability_profile_id: capability.capability_profile_id(),
        bundle_capability_profile_id: bundle.capability_profile_id.content_id(),
        programdata_sha256: capability.programdata_sha256(),
    })?;
    let funding_terms = authenticate_product_artifact_v1::<SeriesFundingTermsV2>(
        program_id,
        funding_terms_account,
        bundle.funding_terms_id.content_id(),
    )?;
    let funding_quote = authenticate_product_artifact_v1::<SeriesFundingQuoteV4>(
        program_id,
        funding_quote_account,
        bundle.funding_quote_id.content_id(),
    )?;
    let terms = *funding_terms.value();
    let quote = *funding_quote.value();
    require(
        terms.series_plan_id == bundle.series_plan_id
            && quote
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == bundle.funding_quote_id,
        ClutchError::MismatchedState,
    )?;

    require(
        funding_account.owner == program_id
            && funding_account.is_writable
            && !funding_account.is_signer
            && !funding_account.executable
            && funding_account.data_len() == SERIES_FUNDING_ACCOUNT_BYTES_V2,
        ClutchError::MismatchedState,
    )?;
    let funding_data = funding_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let funding_account_data_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&funding_data[..]]).to_bytes());
    let observed_funding = SeriesFundingAccountV2::decode(&funding_data)?;
    drop(funding_data);
    require(
        funding.account() == *funding_account.key
            && funding.value() == observed_funding
            && observed_funding.state.series_plan_id == bundle.series_plan_id
            && observed_funding.state.funding_terms_id == bundle.funding_terms_id
            && observed_funding.state.funding_quote_id == bundle.funding_quote_id
            && observed_funding.state.attachment_plan_id == bundle.attachment_plan_id
            && observed_funding.state.compiler_bundle_id == compiler_bundle.bundle_id()
            && observed_funding.state.phase == SeriesFundingPhaseV2::Pending
            && observed_funding.state.pending_disposition
                == Some(SeriesMarketDispositionV1::Founder)
            && observed_funding.state.pending_market_instance_id
                == coordinates.market_instance_id.content_id()
            && observed_funding.state.pending_series_market_link_id
                == coordinates.founder_link_id.content_id(),
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        funding_account.key,
        seeds::series_funding_pda(program_id, &bundle.series_plan_id.bytes()),
        Some(observed_funding.stored_bump),
    )?;

    let market = coordinates.market_instance_id.bytes();
    let (expected_root, _) =
        seeds::product_market_lifecycle_root_pda(program_id, &market, coordinates.generation);
    let (expected_foundation_vault, _) =
        seeds::product_market_foundation_vault_pda(program_id, &market, coordinates.generation);
    let (expected_founder_link, _) = seeds::product_series_market_link_pda(
        program_id,
        &bundle.series_plan_id.bytes(),
        observed_funding.state.pending_ordinal,
    );
    let (expected_lifecycle_replay, _) =
        seeds::product_market_lifecycle_replay_pda(program_id, &market, coordinates.generation);
    require(
        coordinates.generation != 0
            && coordinates.lifecycle_root_account == expected_root
            && coordinates.founder_link_account == expected_founder_link
            && coordinates.lifecycle_replay_account == expected_lifecycle_replay
            && *foundation_vault.key == expected_foundation_vault,
        ClutchError::MismatchedState,
    )?;
    let product_accounts = [
        coordinates.lifecycle_root_account,
        coordinates.founder_link_account,
        *foundation_vault.key,
        coordinates.lifecycle_replay_account,
    ];
    let mut left = 0usize;
    while left < product_accounts.len() {
        let mut right = left + 1;
        while right < product_accounts.len() {
            require(
                product_accounts[left] != product_accounts[right],
                ClutchError::AccountAlias,
            )?;
            right += 1;
        }
        left += 1;
    }
    for product_account in product_accounts {
        for other in [
            *funding_account.key,
            *funding_terms_account.key,
            *funding_quote_account.key,
            *market_core_vault.key,
            *principal_refund_owner.key,
            *neutral_lamport_sink.key,
            *system_program.key,
            *rent_sysvar.key,
        ] {
            require(product_account != other, ClutchError::AccountAlias)?;
        }
    }
    coordinates
        .foundation_account_graph_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    coordinates
        .founder_link_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    coordinates
        .liveness_lifecycle_id
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let (expected_market_core_vault, market_core_bump) = seeds::series_lamport_vault_pda(
        program_id,
        &bundle.series_plan_id.bytes(),
        MARKET_CORE_COMPONENT_SEED_V2,
    );
    expect_pda(
        market_core_vault.key,
        (expected_market_core_vault, market_core_bump),
        None,
    )?;
    for vault in [market_core_vault, foundation_vault] {
        require(
            vault.is_writable
                && !vault.is_signer
                && !vault.executable
                && vault.owner.to_bytes() == SYSTEM_PROGRAM_ID
                && vault.data_len() == 0,
            ClutchError::MismatchedState,
        )?;
    }
    require(
        principal_refund_owner.key.to_bytes() == terms.lamport_principal_refund.bytes()
            && !principal_refund_owner.is_writable
            && !principal_refund_owner.is_signer
            && !principal_refund_owner.executable
            && principal_refund_owner.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && principal_refund_owner.data_len() == 0
            && neutral_lamport_sink.key.to_bytes() == terms.neutral_lamport_sink.bytes()
            && neutral_lamport_sink.is_writable
            && !neutral_lamport_sink.is_signer
            && !neutral_lamport_sink.executable
            && neutral_lamport_sink.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && neutral_lamport_sink.data_len() == 0,
        ClutchError::MismatchedState,
    )?;

    let account_keys = [
        capability.series_registry_account(),
        capability.program_account(),
        capability.programdata_account(),
        capability.release_artifact_account(),
        capability.profile_artifact_account(),
        compiler_bundle.artifact_account(),
    ];
    for hidden in account_keys {
        for visible in [
            funding_account.key,
            funding_terms_account.key,
            funding_quote_account.key,
            market_core_vault.key,
            foundation_vault.key,
            principal_refund_owner.key,
            neutral_lamport_sink.key,
            system_program.key,
            rent_sysvar.key,
        ] {
            require(hidden != *visible, ClutchError::AccountAlias)?;
        }
    }

    let market_index = SeriesFundingComponentV2::MarketCore.index();
    let schedule_id = quote
        .foundation
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    foundation_account_graph
        .validate(&quote.foundation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let authenticated_graph_id = foundation_account_graph
        .id(&quote.foundation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        authenticated_graph_id == coordinates.foundation_account_graph_id
            && foundation_account_graph.market_instance_id == coordinates.market_instance_id
            && foundation_account_graph.generation == coordinates.generation
            && foundation_account_graph.foundation_schedule_id == schedule_id
            && foundation_account_graph
                .account(MarketFoundationSlotV2::LifecycleRoot)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == coordinates.lifecycle_root_account.to_bytes()
            && foundation_account_graph
                .account(MarketFoundationSlotV2::ProductReplayAnchor)
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
                == coordinates.lifecycle_replay_account.to_bytes(),
        ClutchError::MismatchedState,
    )?;
    let funding_state_id = observed_funding
        .state
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let funding_account_authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FOUNDATION_FUNDING_ACCOUNT_AUTHENTICATION_DOMAIN_V1,
            funding_account.key.as_ref(),
            program_id.as_ref(),
            &funding_account_data_id.bytes(),
            &funding_state_id.bytes(),
            &[observed_funding.stored_bump],
            &observed_funding.rent_principal_lamports.to_le_bytes(),
            &funding_account.lamports().to_le_bytes(),
            &observed_funding.state.transition_sequence.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(
        !funding_account_data_id.is_zero() && !funding_account_authentication_id.is_zero(),
        ClutchError::MismatchedState,
    )?;
    let minimum_funding_balance = rent.minimum_balance(SERIES_FUNDING_ACCOUNT_BYTES_V2)?;
    let plan = plan_foundation_vault_funding_v1(FoundationVaultFundingPrestateV1 {
        quoted_market_core: quote.components[market_index],
        pending_market_core: observed_funding.state.pending_debits[market_index],
        remaining_market_core: observed_funding.state.components[market_index].remaining_principal,
        series_component_donations: observed_funding.state.components[market_index].donations,
        observed_series_vault_lamports: market_core_vault.lamports(),
        observed_foundation_vault_lamports: foundation_vault.lamports(),
        funding_account_rent_principal_lamports: observed_funding.rent_principal_lamports,
        funding_account_minimum_balance_lamports: minimum_funding_balance,
        funding_account_observed_lamports: funding_account.lamports(),
    })?;
    require(
        plan.principal_lamports == quote.foundation.total_principal_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?,
        ClutchError::MismatchedState,
    )?;

    let facts = FoundationVaultInitAuthorizationFactsV1 {
        coordinates,
        series_plan_id: bundle.series_plan_id,
        ordinal: observed_funding.state.pending_ordinal,
        funding_state_account: *funding_account.key,
        funding_state_id,
        funding_account_data_id,
        funding_account_authentication_id,
        funding_transition_sequence: observed_funding.state.transition_sequence,
        funding_reservation_receipt_id: observed_funding.state.pending_reservation_receipt_id,
        pending_debits: observed_funding.state.pending_debits,
        compiler_bundle_id,
        registry_release_id: capability.registry_release_id(),
        capability_profile_id: capability.capability_profile_id(),
        funding_terms_id: bundle.funding_terms_id.content_id(),
        funding_quote_id: bundle.funding_quote_id.content_id(),
        foundation_schedule_id: schedule_id,
        market_core_vault: *market_core_vault.key,
        foundation_vault: *foundation_vault.key,
        principal_refund_owner: *principal_refund_owner.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
        principal_lamports: plan.principal_lamports,
        series_vault_donation_lamports: plan.series_vault_donation_lamports,
        foundation_vault_donation_lamports: plan.foundation_vault_donation_lamports,
        series_vault_balance_before: plan.series_vault_balance_before,
        series_vault_balance_after: plan.series_vault_balance_after,
        foundation_vault_balance_before: plan.foundation_vault_balance_before,
        foundation_vault_balance_after: plan.foundation_vault_balance_after,
    };
    authority.authenticate_foundation_vault_init_v1(
        &facts,
        &quote.foundation,
        foundation_account_graph,
    )?;

    let series = bundle.series_plan_id.bytes();
    let component = [MARKET_CORE_COMPONENT_SEED_V2];
    let bump = [market_core_bump];
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(plan.principal_lamports),
        vec![
            AccountMeta::new(*market_core_vault.key, true),
            AccountMeta::new(*foundation_vault.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[
            market_core_vault.clone(),
            foundation_vault.clone(),
            system_program.clone(),
        ],
        &[&[
            seeds::SEED_SERIES_LAMPORT_VAULT_V1,
            &series,
            &component,
            &bump,
        ]],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        market_core_vault.lamports() == plan.series_vault_balance_after
            && foundation_vault.lamports() == plan.foundation_vault_balance_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;

    let rent_threshold_bits = rent.exemption_threshold.to_bits();
    let ordinal = facts.ordinal.to_le_bytes();
    let generation = facts.coordinates.generation.to_le_bytes();
    let principal = facts.principal_lamports.to_le_bytes();
    let series_donation = facts.series_vault_donation_lamports.to_le_bytes();
    let foundation_donation = facts.foundation_vault_donation_lamports.to_le_bytes();
    let funding_rent = observed_funding.rent_principal_lamports.to_le_bytes();
    let funding_lamports = funding_account.lamports().to_le_bytes();
    let series_before = plan.series_vault_balance_before.to_le_bytes();
    let series_after = plan.series_vault_balance_after.to_le_bytes();
    let foundation_before = plan.foundation_vault_balance_before.to_le_bytes();
    let foundation_after = plan.foundation_vault_balance_after.to_le_bytes();
    let rent_rate = rent.lamports_per_byte_year.to_le_bytes();
    let rent_threshold = rent_threshold_bits.to_le_bytes();
    let funding_transition_sequence = facts.funding_transition_sequence.to_le_bytes();
    let pending_debits = flatten_component_debits_v1(&facts.pending_debits);
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FOUNDATION_VAULT_INIT_RECEIPT_DOMAIN_V1,
            &facts.series_plan_id.bytes(),
            &ordinal,
            &facts.coordinates.market_instance_id.bytes(),
            &generation,
            &facts.coordinates.founder_link_id.bytes(),
            facts.coordinates.lifecycle_root_account.as_ref(),
            facts.coordinates.founder_link_account.as_ref(),
            facts.coordinates.lifecycle_replay_account.as_ref(),
            &facts.coordinates.foundation_account_graph_id.bytes(),
            &facts.coordinates.liveness_lifecycle_id.bytes(),
            facts.funding_state_account.as_ref(),
            &facts.funding_state_id.bytes(),
            &facts.funding_account_data_id.bytes(),
            &facts.funding_account_authentication_id.bytes(),
            &funding_transition_sequence,
            &facts.funding_reservation_receipt_id.bytes(),
            &pending_debits,
            &facts.compiler_bundle_id.bytes(),
            &facts.registry_release_id.bytes(),
            &facts.capability_profile_id.bytes(),
            &facts.funding_terms_id.bytes(),
            &facts.funding_quote_id.bytes(),
            &facts.foundation_schedule_id.bytes(),
            facts.market_core_vault.as_ref(),
            facts.foundation_vault.as_ref(),
            facts.principal_refund_owner.as_ref(),
            facts.neutral_lamport_sink.as_ref(),
            capability.series_registry_account().as_ref(),
            capability.program_account().as_ref(),
            capability.programdata_account().as_ref(),
            &capability.programdata_sha256().bytes(),
            capability.release_artifact_account().as_ref(),
            capability.profile_artifact_account().as_ref(),
            compiler_bundle.artifact_account().as_ref(),
            funding_terms.account().as_ref(),
            funding_quote.account().as_ref(),
            rent_sysvar.key.as_ref(),
            &rent_rate,
            &rent_threshold,
            &funding_rent,
            &funding_lamports,
            &principal,
            &series_donation,
            &foundation_donation,
            &series_before,
            &series_after,
            &foundation_before,
            &foundation_after,
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedFoundationVaultInitV1 {
        id,
        facts,
        series_registry_account: capability.series_registry_account(),
        executing_program: capability.program_account(),
        executing_programdata: capability.programdata_account(),
        programdata_sha256: capability.programdata_sha256(),
        release_artifact_account: capability.release_artifact_account(),
        profile_artifact_account: capability.profile_artifact_account(),
        compiler_bundle_artifact_account: compiler_bundle.artifact_account(),
        funding_terms_artifact_account: funding_terms.account(),
        funding_quote_artifact_account: funding_quote.account(),
        rent_sysvar: *rent_sysvar.key,
        rent_lamports_per_byte_year: rent.lamports_per_byte_year,
        rent_exemption_threshold_bits: rent_threshold_bits,
        funding_account_rent_principal_lamports: observed_funding.rent_principal_lamports,
        funding_account_observed_lamports: funding_account.lamports(),
    })
}

const fn liveness_id_from_content(id: ContentId) -> LivenessId {
    LivenessId::from_bytes(id.bytes())
}

fn liveness_id_from_pubkey(key: &Pubkey) -> LivenessId {
    LivenessId::from_bytes(key.to_bytes())
}

/// Move the exact founder-only RecoveryReserve debit into its sole custody.
///
/// The Series component vault is the only physical source. Existing lamports
/// at the predictable Recovery PDA are preserved in the runtime body as
/// donations, while the Series payer still supplies the complete work plus
/// rent principal. The policy, FundingV2 state, QuoteV4, rent sysvar, PDA, and
/// postimage are all hostile-reopened in this call.
#[allow(clippy::too_many_arguments)]
pub(crate) fn capitalize_product_recovery_reserve_v1<'a>(
    program_id: &Pubkey,
    foundation_init: AuthenticatedFoundationVaultInitV1,
    funding: AuthenticatedSeriesFundingAccountV2,
    funding_account: &AccountInfo<'a>,
    funding_quote_account: &AccountInfo<'a>,
    liveness_policy_account: &AccountInfo<'a>,
    recovery_reserve_vault: &AccountInfo<'a>,
    recovery_account: &AccountInfo<'a>,
    principal_refund_owner: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> Outcome<AuthenticatedRecoveryReserveCapitalizationV1> {
    require_system_program(system_program)?;
    let rent = read_rent(rent_sysvar)?;
    require_distinct(&[
        funding_account.clone(),
        funding_quote_account.clone(),
        liveness_policy_account.clone(),
        recovery_reserve_vault.clone(),
        recovery_account.clone(),
        principal_refund_owner.clone(),
        neutral_lamport_sink.clone(),
        system_program.clone(),
        rent_sysvar.clone(),
    ])?;

    let init = foundation_init.facts;
    require(
        foundation_init.executing_program == *program_id
            && foundation_init.rent_sysvar == *rent_sysvar.key
            && foundation_init.rent_lamports_per_byte_year == rent.lamports_per_byte_year
            && foundation_init.rent_exemption_threshold_bits == rent.exemption_threshold.to_bits()
            && foundation_init.funding_quote_artifact_account == *funding_quote_account.key
            && init.funding_state_account == *funding_account.key
            && init.principal_refund_owner == *principal_refund_owner.key
            && init.neutral_lamport_sink == *neutral_lamport_sink.key,
        ClutchError::MismatchedState,
    )?;
    require(
        principal_refund_owner.key.to_bytes() == init.principal_refund_owner.to_bytes()
            && !principal_refund_owner.is_signer
            && !principal_refund_owner.executable
            && principal_refund_owner.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && principal_refund_owner.data_len() == 0
            && neutral_lamport_sink.key.to_bytes() == init.neutral_lamport_sink.to_bytes()
            && neutral_lamport_sink.is_writable
            && !neutral_lamport_sink.is_signer
            && !neutral_lamport_sink.executable
            && neutral_lamport_sink.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && neutral_lamport_sink.data_len() == 0,
        ClutchError::MismatchedState,
    )?;

    require(
        funding_account.owner == program_id
            && funding_account.is_writable
            && !funding_account.is_signer
            && !funding_account.executable
            && funding_account.data_len() == SERIES_FUNDING_ACCOUNT_BYTES_V2,
        ClutchError::MismatchedState,
    )?;
    let funding_data = funding_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let funding_data_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&funding_data[..]]).to_bytes());
    let observed_funding = SeriesFundingAccountV2::decode(&funding_data)?;
    drop(funding_data);
    let funding_state_id = observed_funding
        .state
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    let funding_authentication_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            FOUNDATION_FUNDING_ACCOUNT_AUTHENTICATION_DOMAIN_V1,
            funding_account.key.as_ref(),
            program_id.as_ref(),
            &funding_data_id.bytes(),
            &funding_state_id.bytes(),
            &[observed_funding.stored_bump],
            &observed_funding.rent_principal_lamports.to_le_bytes(),
            &funding_account.lamports().to_le_bytes(),
            &observed_funding.state.transition_sequence.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(
        funding.account() == *funding_account.key
            && funding.value() == observed_funding
            && funding_state_id == init.funding_state_id
            && funding_data_id == init.funding_account_data_id
            && funding_authentication_id == init.funding_account_authentication_id
            && funding_account.lamports() == foundation_init.funding_account_observed_lamports
            && observed_funding.state.phase == SeriesFundingPhaseV2::Pending
            && observed_funding.state.series_plan_id == init.series_plan_id
            && observed_funding.state.pending_ordinal == init.ordinal
            && observed_funding.state.pending_debits == init.pending_debits
            && observed_funding.state.pending_reservation_receipt_id
                == init.funding_reservation_receipt_id
            && observed_funding.state.transition_sequence == init.funding_transition_sequence,
        ClutchError::MismatchedState,
    )?;
    expect_pda(
        funding_account.key,
        seeds::series_funding_pda(program_id, &init.series_plan_id.bytes()),
        Some(observed_funding.stored_bump),
    )?;
    require(
        observed_funding.rent_principal_lamports
            == foundation_init.funding_account_rent_principal_lamports
            && observed_funding.rent_principal_lamports
                >= rent.minimum_balance(SERIES_FUNDING_ACCOUNT_BYTES_V2)?,
        ClutchError::MismatchedState,
    )?;

    let quote_artifact = authenticate_product_artifact_v1::<SeriesFundingQuoteV4>(
        program_id,
        funding_quote_account,
        init.funding_quote_id,
    )?;
    let quote = *quote_artifact.value();
    let quote_id = quote
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        quote_id.content_id() == init.funding_quote_id,
        ClutchError::MismatchedState,
    )?;

    require(
        liveness_policy_account.owner == program_id
            && !liveness_policy_account.is_writable
            && !liveness_policy_account.is_signer
            && !liveness_policy_account.executable
            && liveness_policy_account.data_len() == FAILURE_LIVENESS_POLICY_ACCOUNT_BYTES_V1,
        ClutchError::MismatchedState,
    )?;
    let policy_data = liveness_policy_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let policy_frame = decode_failure_account_body_v1(
        &policy_data,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_TAG,
        registry::FAILURE_LIVENESS_POLICY_ACCOUNT_VERSION,
        FAILURE_LIVENESS_POLICY_BODY_BYTES_V1,
    )?;
    let policy = decode_runtime_policy_account_v1(
        liveness_id_from_pubkey(program_id),
        liveness_id_from_pubkey(liveness_policy_account.key),
        RuntimePersistedAccountViewV1 {
            account_id: liveness_id_from_pubkey(liveness_policy_account.key),
            owner_program_id: liveness_id_from_pubkey(liveness_policy_account.owner),
            lamports: liveness_policy_account.lamports(),
            data: policy_frame.body,
            writable: false,
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let policy_bump = policy_frame.stored_bump;
    drop(policy_data);
    expect_pda(
        liveness_policy_account.key,
        seeds::failure_liveness_policy_pda(program_id, &quote.failure_liveness_policy_id.bytes()),
        Some(policy_bump),
    )?;
    let recovery_policy = policy.compartments[RuntimeCompartmentKindV1::Recovery.index()];
    require(
        policy.policy_id == liveness_id_from_content(quote.failure_liveness_policy_id)
            && policy.neutral_sink == liveness_id_from_pubkey(neutral_lamport_sink.key)
            && recovery_policy.kind == RuntimeCompartmentKindV1::Recovery
            && recovery_policy.quote_schedule_id
                == liveness_id_from_content(quote.failure_recovery_quote_schedule_id)
            && recovery_policy.receipt_program_id == liveness_id_from_pubkey(program_id)
            && recovery_policy.account_rent_principal_lamports
                == quote.recovery_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;

    let recovery_index = SeriesFundingComponentV2::RecoveryReserve.index();
    let (expected_reserve, reserve_bump) = seeds::series_lamport_vault_pda(
        program_id,
        &init.series_plan_id.bytes(),
        RECOVERY_RESERVE_COMPONENT_SEED_V2,
    );
    expect_pda(
        recovery_reserve_vault.key,
        (expected_reserve, reserve_bump),
        None,
    )?;
    let lifecycle_id = liveness_id_from_content(init.coordinates.liveness_lifecycle_id);
    let (expected_recovery, recovery_bump) = seeds::failure_external_recovery_pda(
        program_id,
        &lifecycle_id.bytes(),
        init.coordinates.generation,
    );
    expect_pda(
        recovery_account.key,
        (expected_recovery, recovery_bump),
        None,
    )?;
    for account in [recovery_reserve_vault, recovery_account] {
        require(
            account.is_writable
                && !account.is_signer
                && !account.executable
                && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
                && account.data_len() == 0,
            ClutchError::MismatchedState,
        )?;
    }
    require(
        rent.minimum_balance(FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1)?
            == quote.recovery_rent_principal_lamports,
        ClutchError::MismatchedState,
    )?;
    let plan = plan_recovery_reserve_funding_v1(
        quote.components[recovery_index],
        observed_funding.state.pending_debits[recovery_index],
        observed_funding.state.components[recovery_index].remaining_principal,
        observed_funding.state.components[recovery_index].donations,
        recovery_policy.work_capital_lamports,
        recovery_policy.account_rent_principal_lamports,
        recovery_reserve_vault.lamports(),
        recovery_account.lamports(),
    )?;
    let recovery_state = RuntimeCompartmentV1::admit(
        policy,
        RuntimeCompartmentAdmissionV1 {
            kind: RuntimeCompartmentKindV1::Recovery,
            identity: RuntimeCompartmentIdentityV1 {
                policy_id: policy.policy_id,
                lifecycle_id,
                account_id: liveness_id_from_pubkey(recovery_account.key),
                owner: liveness_id_from_pubkey(program_id),
                payer: liveness_id_from_pubkey(principal_refund_owner.key),
                neutral_sink: liveness_id_from_pubkey(neutral_lamport_sink.key),
                generation: init.coordinates.generation,
            },
            funding: PresentFundingV1 {
                payer: liveness_id_from_pubkey(principal_refund_owner.key),
                source: PresentFundingSourceV1::PrecapitalizedLivenessEndowment,
                payer_debit_lamports: plan.payer_debit_lamports,
                account_balance_before: plan.recovery_balance_before,
                account_balance_after: plan.recovery_balance_after,
            },
        },
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        recovery_state
            .expected_account_balance_lamports()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == plan.recovery_balance_after,
        ClutchError::MismatchedState,
    )?;
    let mut recovery_body = [0u8; FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1];
    recovery_state
        .encode(&mut recovery_body)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let series = init.series_plan_id.bytes();
    let reserve_component = [RECOVERY_RESERVE_COMPONENT_SEED_V2];
    let reserve_bump_seed = [reserve_bump];
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(plan.payer_debit_lamports),
        vec![
            AccountMeta::new(*recovery_reserve_vault.key, true),
            AccountMeta::new(*recovery_account.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[
            recovery_reserve_vault.clone(),
            recovery_account.clone(),
            system_program.clone(),
        ],
        &[&[
            seeds::SEED_SERIES_LAMPORT_VAULT_V1,
            &series,
            &reserve_component,
            &reserve_bump_seed,
        ]],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))?;
    require(
        recovery_reserve_vault.lamports() == plan.source_balance_after
            && recovery_account.lamports() == plan.recovery_balance_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;

    let lifecycle = init.coordinates.liveness_lifecycle_id.bytes();
    let generation = init.coordinates.generation.to_le_bytes();
    let recovery_bump_seed = [recovery_bump];
    let recovery_signer = &[
        seeds::SEED_FAILURE_EXTERNAL_RECOVERY,
        &lifecycle,
        &generation,
        &recovery_bump_seed,
    ];
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1),
        vec![AccountMeta::new(*recovery_account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[recovery_account.clone(), system_program.clone()],
        &[recovery_signer],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*recovery_account.key, true)],
    );
    invoke_signed(
        &assign,
        &[recovery_account.clone(), system_program.clone()],
        &[recovery_signer],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    require(
        recovery_account.owner == program_id
            && recovery_account.data_len() == FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1
            && recovery_account.lamports() == plan.recovery_balance_after,
        ClutchError::MismatchedState,
    )?;
    let mut recovery_data = recovery_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        recovery_data.iter().all(|byte| *byte == 0),
        ClutchError::AlreadyInitialized,
    )?;
    encode_failure_account_header_v1(
        &mut recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        recovery_bump,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    recovery_data[FAILURE_ACCOUNT_HEADER_BYTES_V1..].copy_from_slice(&recovery_body);
    let recovery_data_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&recovery_data[..]]).to_bytes());
    let rebound_frame = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    let rebound_stored_bump = rebound_frame.stored_bump;
    let rebound = RuntimeCompartmentV1::decode(rebound_frame.body)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    drop(recovery_data);
    require(
        rebound == recovery_state
            && rebound_stored_bump == recovery_bump
            && recovery_account.lamports() == plan.recovery_balance_after,
        ClutchError::MismatchedState,
    )?;

    let facts = RecoveryReserveCapitalizationFactsV1 {
        foundation_init_receipt_id: foundation_init.id,
        series_plan_id: init.series_plan_id,
        market_instance_id: init.coordinates.market_instance_id,
        generation: init.coordinates.generation,
        funding_account: *funding_account.key,
        funding_state_id,
        funding_transition_sequence: init.funding_transition_sequence,
        funding_reservation_receipt_id: init.funding_reservation_receipt_id,
        recovery_reserve_vault: *recovery_reserve_vault.key,
        recovery_account: *recovery_account.key,
        liveness_policy_account: *liveness_policy_account.key,
        liveness_policy_id: quote.failure_liveness_policy_id,
        liveness_realm_id: ContentId::from_bytes(policy.realm_id.bytes()),
        liveness_lifecycle_id: init.coordinates.liveness_lifecycle_id,
        quote_schedule_id: quote.failure_recovery_quote_schedule_id,
        payer: *principal_refund_owner.key,
        neutral_lamport_sink: *neutral_lamport_sink.key,
        work_principal_lamports: plan.work_principal_lamports,
        rent_principal_lamports: plan.rent_principal_lamports,
        payer_debit_lamports: plan.payer_debit_lamports,
        source_donation_lamports: plan.source_donation_lamports,
        recovery_donation_lamports: plan.recovery_donation_lamports,
        source_balance_before: plan.source_balance_before,
        source_balance_after: plan.source_balance_after,
        recovery_balance_before: plan.recovery_balance_before,
        recovery_balance_after: plan.recovery_balance_after,
    };
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            RECOVERY_RESERVE_CAPITALIZATION_RECEIPT_DOMAIN_V1,
            program_id.as_ref(),
            &foundation_init.id.bytes(),
            &facts.series_plan_id.bytes(),
            &facts.market_instance_id.bytes(),
            &facts.generation.to_le_bytes(),
            facts.funding_account.as_ref(),
            &facts.funding_state_id.bytes(),
            &facts.funding_transition_sequence.to_le_bytes(),
            &facts.funding_reservation_receipt_id.bytes(),
            facts.recovery_reserve_vault.as_ref(),
            facts.recovery_account.as_ref(),
            facts.liveness_policy_account.as_ref(),
            &facts.liveness_policy_id.bytes(),
            &facts.liveness_realm_id.bytes(),
            &facts.liveness_lifecycle_id.bytes(),
            &facts.quote_schedule_id.bytes(),
            facts.payer.as_ref(),
            facts.neutral_lamport_sink.as_ref(),
            &facts.work_principal_lamports.to_le_bytes(),
            &facts.rent_principal_lamports.to_le_bytes(),
            &facts.payer_debit_lamports.to_le_bytes(),
            &facts.source_donation_lamports.to_le_bytes(),
            &facts.recovery_donation_lamports.to_le_bytes(),
            &facts.source_balance_before.to_le_bytes(),
            &facts.source_balance_after.to_le_bytes(),
            &facts.recovery_balance_before.to_le_bytes(),
            &facts.recovery_balance_after.to_le_bytes(),
            &recovery_data_id.bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedRecoveryReserveCapitalizationV1 {
        id,
        facts,
        recovery_state: rebound,
        recovery_data_id,
    })
}

fn invoke_founder_pda_transfer_v1<'a>(
    source: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    lamports: u64,
    source_signer_seeds: &[&[u8]],
) -> Outcome<()> {
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(lamports),
        vec![
            AccountMeta::new(*source.key, true),
            AccountMeta::new(*destination.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[source.clone(), destination.clone(), system_program.clone()],
        &[source_signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::SeriesCustodyDeltaMismatch))
}

fn allocate_assign_founder_pda_v1<'a>(
    program_id: &Pubkey,
    account: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    account_bytes: usize,
    signer_seeds: &[&[u8]],
) -> Outcome<()> {
    let allocate = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &allocate_data(account_bytes),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &allocate,
        &[account.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))?;
    let assign = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &assign_data(program_id),
        vec![AccountMeta::new(*account.key, true)],
    );
    invoke_signed(
        &assign,
        &[account.clone(), system_program.clone()],
        &[signer_seeds],
    )
    .map_err(|_| Refusal::Adapter(ClutchError::AccountCreationFailed))
}

/// Create the sole first shared Market root and its founder Series link.
///
/// This capability-disabled composer consumes the exact prior FoundationVault
/// and Recovery capitalization receipts. It hostile-reopens the current
/// RegistryV2/FundingV2 and nine Product artifacts, compares every active slot
/// of the complete 46-slot graph to the supplied physical account, and refuses
/// any already-owned/preallocated body. Root and link predictable-address
/// prefunds remain distinct donation balances and never discount the full
/// pending MarketCore/SeriesAdmission principal debits.
///
/// The resulting root is `Founding`, the link is `PendingMarket`, and Funding
/// remains `Pending`. The later phased Foundation owner must activate root then
/// link, call Product/Series' private pending-completion writer, and finally
/// record the permanent Series lifecycle replay in that same atomic call.
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_product_market_founder_v1<
    'a,
    A: AuthenticatedProductMarketFounderCreationAuthorityV1 + ?Sized,
>(
    program_id: &Pubkey,
    authority: &A,
    foundation_init: AuthenticatedFoundationVaultInitV1,
    recovery: AuthenticatedRecoveryReserveCapitalizationV1,
    capability: AuthenticatedRegistryCapabilityV3,
    compiler_bundle: AuthenticatedCompiledProductSeriesBundleV5,
    registry: AuthenticatedSeriesRegistryAccountV2,
    funding: AuthenticatedSeriesFundingAccountV2,
    semantic: ProductMarketFounderSemanticV1,
    account_graph: &MarketFoundationAccountGraphV2,
    active_foundation_accounts: &[AccountInfo<'a>],
    series_artifact_accounts: &[AccountInfo<'a>],
    registry_account: &AccountInfo<'a>,
    funding_account: &AccountInfo<'a>,
    funding_quote_account: &AccountInfo<'a>,
    foundation_vault: &AccountInfo<'a>,
    series_admission_vault: &AccountInfo<'a>,
    root_account: &AccountInfo<'a>,
    founder_link_account: &AccountInfo<'a>,
    recovery_account: &AccountInfo<'a>,
    principal_refund_owner: &AccountInfo<'a>,
    neutral_lamport_sink: &AccountInfo<'a>,
    system_program: &AccountInfo<'a>,
    rent_sysvar: &AccountInfo<'a>,
) -> Outcome<AuthenticatedProductMarketFounderCreationV1> {
    require_system_program(system_program)?;
    let rent = read_rent(rent_sysvar)?;
    require_distinct(&[
        registry_account.clone(),
        funding_account.clone(),
        funding_quote_account.clone(),
        foundation_vault.clone(),
        series_admission_vault.clone(),
        root_account.clone(),
        founder_link_account.clone(),
        recovery_account.clone(),
        principal_refund_owner.clone(),
        neutral_lamport_sink.clone(),
        system_program.clone(),
        rent_sysvar.clone(),
    ])?;

    let init = foundation_init.facts;
    let recovery_facts = recovery.facts;
    let bundle = compiler_bundle.bundle();
    require(
        foundation_init.executing_program == *program_id
            && foundation_init.series_registry_account == *registry_account.key
            && foundation_init.compiler_bundle_artifact_account
                == compiler_bundle.artifact_account()
            && foundation_init.funding_quote_artifact_account == *funding_quote_account.key
            && foundation_init.rent_sysvar == *rent_sysvar.key
            && foundation_init.rent_lamports_per_byte_year == rent.lamports_per_byte_year
            && foundation_init.rent_exemption_threshold_bits == rent.exemption_threshold.to_bits()
            && recovery_facts.foundation_init_receipt_id == foundation_init.id
            && recovery_facts.series_plan_id == init.series_plan_id
            && recovery_facts.market_instance_id == init.coordinates.market_instance_id
            && recovery_facts.generation == init.coordinates.generation
            && recovery_facts.funding_account == *funding_account.key
            && recovery_facts.funding_state_id == init.funding_state_id
            && recovery_facts.funding_transition_sequence == init.funding_transition_sequence
            && recovery_facts.funding_reservation_receipt_id
                == init.funding_reservation_receipt_id
            && recovery_facts.recovery_account == *recovery_account.key
            && recovery_facts.payer == *principal_refund_owner.key
            && recovery_facts.neutral_lamport_sink == *neutral_lamport_sink.key
            && recovery_facts.liveness_lifecycle_id
                == init.coordinates.liveness_lifecycle_id
            && recovery.recovery_state.identity.account_id
                == liveness_id_from_pubkey(recovery_account.key),
        ClutchError::MismatchedState,
    )?;
    require(
        capability.program_account() == *program_id
            && capability.series_registry_account() == *registry_account.key
            && capability.series_plan_id() == init.series_plan_id
            && capability.funding_terms_id().content_id() == init.funding_terms_id
            && capability.compiler_bundle_id() == init.compiler_bundle_id
            && capability.registry_release_id() == init.registry_release_id
            && capability.capability_profile_id() == init.capability_profile_id
            && compiler_bundle.bundle_id().content_id() == init.compiler_bundle_id
            && bundle.series_plan_id == init.series_plan_id
            && bundle.funding_terms_id.content_id() == init.funding_terms_id
            && bundle.funding_quote_id.content_id() == init.funding_quote_id
            && bundle.registry_release_id == init.registry_release_id
            && bundle.capability_profile_id.content_id() == init.capability_profile_id,
        ClutchError::MismatchedState,
    )?;

    let artifacts = authenticate_series_artifact_accounts_v4(
        program_id,
        series_artifact_accounts,
        bundle.series_plan_id,
        bundle.funding_terms_id,
    )?;
    let live_registry = read_series_registry_account_v2(
        program_id,
        registry_account,
        init.series_plan_id,
        &rent,
    )?;
    require(live_registry == registry, ClutchError::MismatchedState)?;
    let live_funding = read_series_funding_account_v2(
        program_id,
        funding_account,
        live_registry,
        &artifacts,
        &rent,
    )?;
    require(
        live_funding == funding
            && live_funding.value().state.phase == SeriesFundingPhaseV2::Pending
            && live_funding.value().state.pending_ordinal == init.ordinal
            && live_funding.value().state.pending_debits == init.pending_debits
            && live_funding.value().state.pending_reservation_receipt_id
                == init.funding_reservation_receipt_id
            && live_funding.value().state.transition_sequence == init.funding_transition_sequence,
        ClutchError::MismatchedState,
    )?;
    let quote_artifact = authenticate_product_artifact_v1::<SeriesFundingQuoteV4>(
        program_id,
        funding_quote_account,
        init.funding_quote_id,
    )?;
    let quote = *quote_artifact.value();
    require(
        quote
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .content_id()
            == init.funding_quote_id
            && quote
                .foundation
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == init.foundation_schedule_id,
        ClutchError::MismatchedState,
    )?;

    account_graph
        .validate(&quote.foundation)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        account_graph
            .id(&quote.foundation)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            == init.coordinates.foundation_account_graph_id
            && account_graph.market_instance_id == init.coordinates.market_instance_id
            && account_graph.generation == init.coordinates.generation,
        ClutchError::MismatchedState,
    )?;
    require_canonical_market_foundation_core_v2(program_id, *root_account.key, account_graph)?;
    let mut active_index = 0usize;
    let mut slot_index = 0usize;
    while slot_index < account_graph.account_ids.len() {
        let account_id = account_graph.account_ids[slot_index];
        if !account_id.is_zero() {
            let account = active_foundation_accounts
                .get(active_index)
                .ok_or_else(|| Refusal::Adapter(ClutchError::AccountCount))?;
            require(
                account.key.to_bytes() == account_id.bytes()
                    && !account.is_signer
                    && !account.executable
                    && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
                    && account.data_len() == 0
                    && (slot_index != MarketFoundationSlotV2::LifecycleRoot.index()
                        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                        || (*account.key == *root_account.key && account.is_writable)),
                ClutchError::MismatchedState,
            )?;
            active_index = active_index
                .checked_add(1)
                .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
        }
        slot_index += 1;
    }
    require(
        active_index == active_foundation_accounts.len()
            && active_index <= usize::from(u8::MAX),
        ClutchError::AccountCount,
    )?;

    require(
        semantic.market_binding.market_instance_id == init.coordinates.market_instance_id
            && semantic.market_binding.generation == init.coordinates.generation
            && semantic.market_binding.outcome_count == quote.foundation.outcome_count
            && semantic.market_binding.registry_release_id == init.registry_release_id
            && semantic.market_binding.capability_profile_id == init.capability_profile_id
            && semantic.market_binding.realm_id == recovery_facts.liveness_realm_id
            && semantic.market_binding.product_template_id
                == bundle.product_template_id.content_id()
            && semantic.market_binding.native_claim_basis_id
                == bundle.native_claim_basis_id.content_id()
            && semantic.market_binding.recovery_policy_id
                == bundle.evidence_only_recovery_policy_id.content_id()
            && semantic.market_binding.price_measure_policy_id
                == bundle.price_measure_policy_id.content_id()
            && semantic.market_binding.market_genesis_profile_id
                == bundle.market_genesis_profile_id.content_id()
            && semantic.market_binding.source_release_id == bundle.source_release_manifest_id
            && semantic.market_binding.source_plane_contract_id
                == bundle.source_plane_contract_id
            && semantic.market_binding.source_spec_id == bundle.source_spec_id
            && semantic.market_binding.failure_liveness_policy_id
                == recovery_facts.liveness_policy_id
            && semantic.market_binding.failure_liveness_quote_schedule_id
                == recovery_facts.quote_schedule_id
            && semantic.market_binding.foundation_vault_id.bytes()
                == foundation_vault.key.to_bytes()
            && semantic.market_binding.foundation_account_graph_id
                == init.coordinates.foundation_account_graph_id
            && semantic.market_binding.foundation_schedule_id == init.foundation_schedule_id,
        ClutchError::MismatchedState,
    )?;
    let market_binding_id = semantic
        .market_binding
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_binding = semantic.founder_link_binding;
    require(
        link_binding.series_plan_id == init.series_plan_id
            && link_binding.ordinal == init.ordinal
            && link_binding.market_instance_id == init.coordinates.market_instance_id
            && link_binding.market_root_account_id.bytes() == root_account.key.to_bytes()
            && link_binding.market_binding_id == market_binding_id
            && link_binding.disposition == SeriesMarketDispositionV1::Founder
            && link_binding.funding_terms_id.content_id() == init.funding_terms_id
            && link_binding.funding_quote_id.content_id() == init.funding_quote_id
            && link_binding.attachment_plan_id == bundle.attachment_plan_id.content_id()
            && link_binding.capability_profile_id == init.capability_profile_id
            && link_binding.compiler_output_id == init.compiler_bundle_id
            && link_binding.source_release_id == bundle.source_release_manifest_id
            && link_binding.source_plane_contract_id == bundle.source_plane_contract_id
            && link_binding.source_spec_id == bundle.source_spec_id
            && link_binding.funding_state_account_id.bytes() == funding_account.key.to_bytes()
            && link_binding.funding_debit_receipt_id == init.funding_reservation_receipt_id
            && link_binding.rent_refund_owner.bytes() == principal_refund_owner.key.to_bytes()
            && link_binding.neutral_lamport_sink.bytes() == neutral_lamport_sink.key.to_bytes()
            && link_binding.generation == init.coordinates.generation
            && link_binding.funding_transition_sequence == init.funding_transition_sequence,
        ClutchError::MismatchedState,
    )?;
    semantic
        .product_families
        .validate()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    for account in [foundation_vault, series_admission_vault] {
        require(
            account.is_writable
                && !account.is_signer
                && !account.executable
                && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
                && account.data_len() == 0,
            ClutchError::MismatchedState,
        )?;
    }
    for account in [root_account, founder_link_account] {
        require(
            account.is_writable
                && !account.is_signer
                && !account.executable
                && account.owner.to_bytes() == SYSTEM_PROGRAM_ID
                && account.data_len() == 0,
            ClutchError::AlreadyInitialized,
        )?;
    }
    require(
        *foundation_vault.key == init.foundation_vault
            && foundation_vault.lamports() == init.foundation_vault_balance_after
            && *root_account.key == init.coordinates.lifecycle_root_account
            && *founder_link_account.key == init.coordinates.founder_link_account
            && *principal_refund_owner.key == init.principal_refund_owner
            && *neutral_lamport_sink.key == init.neutral_lamport_sink
            && principal_refund_owner.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && principal_refund_owner.data_len() == 0
            && !principal_refund_owner.is_signer
            && !principal_refund_owner.executable
            && neutral_lamport_sink.owner.to_bytes() == SYSTEM_PROGRAM_ID
            && neutral_lamport_sink.data_len() == 0
            && neutral_lamport_sink.is_writable
            && !neutral_lamport_sink.is_signer
            && !neutral_lamport_sink.executable,
        ClutchError::MismatchedState,
    )?;

    let recovery_data = recovery_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let recovery_data_id =
        ContentId::from_bytes(solana_sha256_hasher::hashv(&[&recovery_data[..]]).to_bytes());
    let recovery_frame = decode_failure_account_body_v1(
        &recovery_data,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_TAG,
        registry::FAILURE_EXTERNAL_RECOVERY_ACCOUNT_VERSION,
        FAILURE_EXTERNAL_RECOVERY_BODY_BYTES_V1,
    )?;
    let recovery_state = RuntimeCompartmentV1::decode(recovery_frame.body)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let recovery_stored_bump = recovery_frame.stored_bump;
    drop(recovery_data);
    expect_pda(
        recovery_account.key,
        seeds::failure_external_recovery_pda(
            program_id,
            &recovery_facts.liveness_lifecycle_id.bytes(),
            recovery_facts.generation,
        ),
        Some(recovery_stored_bump),
    )?;
    require(
        recovery_account.owner == program_id
            && recovery_account.data_len() == FAILURE_EXTERNAL_RECOVERY_ACCOUNT_BYTES_V1
            && recovery_account.lamports() == recovery_facts.recovery_balance_after
            && recovery_data_id == recovery.recovery_data_id
            && recovery_state == recovery.recovery_state,
        ClutchError::MismatchedState,
    )?;

    let root_index = MarketFoundationSlotV2::LifecycleRoot
        .index()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_rent_principal_lamports = quote.foundation.slot_principal_lamports[root_index];
    require(
        root_rent_principal_lamports != 0
            && root_rent_principal_lamports
                == rent.minimum_balance(MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1)?,
        ClutchError::MismatchedState,
    )?;
    let foundation_balance_after = foundation_vault
        .lamports()
        .checked_sub(root_rent_principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let expected_foundation_after = init
        .principal_lamports
        .checked_sub(root_rent_principal_lamports)
        .and_then(|value| value.checked_add(init.foundation_vault_donation_lamports))
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    let root_balance_after = root_account
        .lamports()
        .checked_add(root_rent_principal_lamports)
        .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        foundation_balance_after == expected_foundation_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;

    let admission_index = SeriesFundingComponentV2::SeriesAdmission.index();
    let admission_quote = quote.components[admission_index];
    let pending_admission = live_funding.value().state.pending_debits[admission_index];
    let remaining_admission = live_funding.value().state.components[admission_index]
        .remaining_principal;
    let admission_donations =
        live_funding.value().state.components[admission_index].donations;
    let (expected_admission_vault, admission_vault_bump) = seeds::series_lamport_vault_pda(
        program_id,
        &init.series_plan_id.bytes(),
        SERIES_ADMISSION_COMPONENT_SEED_V2,
    );
    expect_pda(
        series_admission_vault.key,
        (expected_admission_vault, admission_vault_bump),
        None,
    )?;
    let link_plan = plan_pending_native_component_v1(
        admission_quote,
        pending_admission,
        remaining_admission,
        admission_donations,
        admission_quote.lamports,
        series_admission_vault.lamports(),
        founder_link_account.lamports(),
    )?;
    require(
        link_plan.payer_debit_lamports
            == rent.minimum_balance(SERIES_MARKET_LINK_ACCOUNT_BYTES_V1)?,
        ClutchError::MismatchedState,
    )?;

    let root_poststate_receipt_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_MARKET_FOUNDER_ROOT_POSTSTATE_DOMAIN_V1,
            program_id.as_ref(),
            &foundation_init.id.bytes(),
            &recovery.id.bytes(),
            root_account.key.as_ref(),
            founder_link_account.key.as_ref(),
            foundation_vault.key.as_ref(),
            &root_rent_principal_lamports.to_le_bytes(),
            &root_account.lamports().to_le_bytes(),
            &root_balance_after.to_le_bytes(),
            &foundation_vault.lamports().to_le_bytes(),
            &foundation_balance_after.to_le_bytes(),
            &link_plan.payer_debit_lamports.to_le_bytes(),
            &link_plan.destination_donation_lamports.to_le_bytes(),
            &link_plan.destination_balance_after.to_le_bytes(),
            &market_binding_id.bytes(),
            &init.coordinates.foundation_account_graph_id.bytes(),
        ])
        .to_bytes(),
    );
    require(!root_poststate_receipt_id.is_zero(), ClutchError::MismatchedState)?;
    let facts = ProductMarketFounderCreationFactsV1 {
        foundation_init_receipt_id: foundation_init.id,
        recovery_capitalization_receipt_id: recovery.id,
        registry_account: *registry_account.key,
        funding_account: *funding_account.key,
        root_account: *root_account.key,
        founder_link_account: *founder_link_account.key,
        foundation_vault: *foundation_vault.key,
        series_admission_vault: *series_admission_vault.key,
        recovery_account: *recovery_account.key,
        active_foundation_accounts: u8::try_from(active_index)
            .map_err(|_| Refusal::Adapter(ClutchError::Arithmetic))?,
        root_rent_principal_lamports,
        root_donation_lamports: root_account.lamports(),
        foundation_balance_before: foundation_vault.lamports(),
        foundation_balance_after,
        link_rent_principal_lamports: link_plan.payer_debit_lamports,
        link_donation_lamports: link_plan.destination_donation_lamports,
        series_admission_balance_before: link_plan.source_balance_before,
        series_admission_balance_after: link_plan.source_balance_after,
        root_poststate_receipt_id,
    };
    authority.authenticate_product_market_founder_creation_v1(
        &facts,
        &semantic,
        &quote.foundation,
        account_graph,
    )?;

    let link_state = SeriesMarketLinkV1::initialize_pending(
        semantic.founder_link_binding,
        semantic.obligation_configuration,
        link_plan.payer_debit_lamports,
        link_plan.destination_donation_lamports,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let link_semantic_id = link_state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        link_semantic_id == init.coordinates.founder_link_id,
        ClutchError::MismatchedState,
    )?;
    let capital = MarketFoundationCapitalV1 {
        founder_link_id: link_semantic_id,
        market_core_debit_receipt_id: foundation_init.id,
        recovery_debit_receipt_id: recovery.id,
        rent_refund_owner: ContentId::from_bytes(principal_refund_owner.key.to_bytes()),
        neutral_lamport_sink: ContentId::from_bytes(neutral_lamport_sink.key.to_bytes()),
        principal_total_lamports: init.principal_lamports,
        principal_remaining_lamports: init.principal_lamports,
        vault_donation_floor_lamports: init.foundation_vault_donation_lamports,
        vault_current_donation_lamports: init.foundation_vault_donation_lamports,
        recovery_work_principal_lamports: recovery_facts.work_principal_lamports,
        recovery_rent_principal_lamports: recovery_facts.rent_principal_lamports,
    };
    let root_initial = MarketLifecycleRootV1::initialize_founder(
        semantic.market_binding,
        &quote.foundation,
        account_graph,
        capital,
        &semantic.product_families,
        root_poststate_receipt_id,
    )
    .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let market_admission =
        SeriesMarketAdmissionProjectionV1::new(semantic.market_binding, link_state, 1)
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let root_state = root_initial
        .admit_series_link(market_admission)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    require(
        root_state.phase() == clutch_product_series::MarketLifecyclePhaseV1::Founding
            && root_state.admitted_series_links() == 1
            && root_state.live_series_links() == 1
            && root_state.capital().principal_remaining_lamports
                == init
                    .principal_lamports
                    .checked_sub(root_rent_principal_lamports)
                    .ok_or_else(|| Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::MismatchedState,
    )?;
    let root_semantic_id = root_state
        .semantic_id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;

    let market = init.coordinates.market_instance_id.bytes();
    let generation = init.coordinates.generation.to_le_bytes();
    let (expected_foundation_vault, foundation_vault_bump) =
        seeds::product_market_foundation_vault_pda(
            program_id,
            &market,
            init.coordinates.generation,
        );
    expect_pda(
        foundation_vault.key,
        (expected_foundation_vault, foundation_vault_bump),
        None,
    )?;
    let (_, root_bump) = seeds::product_market_lifecycle_root_pda(
        program_id,
        &market,
        init.coordinates.generation,
    );
    let (_, link_bump) = seeds::product_series_market_link_pda(
        program_id,
        &init.series_plan_id.bytes(),
        init.ordinal,
    );
    let mut root_postimage = [0u8; MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1];
    MarketLifecycleRootAccountV1::encode_parts(
        &root_state,
        root_rent_principal_lamports,
        root_bump,
        &mut root_postimage,
    )?;
    let mut link_postimage = [0u8; SERIES_MARKET_LINK_ACCOUNT_BYTES_V1];
    SeriesMarketLinkAccountV1::encode_parts(&link_state, link_bump, &mut link_postimage)?;

    let foundation_vault_bump_seed = [foundation_vault_bump];
    invoke_founder_pda_transfer_v1(
        foundation_vault,
        root_account,
        system_program,
        root_rent_principal_lamports,
        &[
            seeds::SEED_PRODUCT_MARKET_FOUNDATION_VAULT,
            &market,
            &generation,
            &foundation_vault_bump_seed,
        ],
    )?;
    require(
        foundation_vault.lamports() == foundation_balance_after
            && root_account.lamports() == root_balance_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let root_bump_seed = [root_bump];
    allocate_assign_founder_pda_v1(
        program_id,
        root_account,
        system_program,
        MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1,
        &[
            seeds::SEED_PRODUCT_MARKET_LIFECYCLE_ROOT,
            &market,
            &generation,
            &root_bump_seed,
        ],
    )?;
    let mut root_data = root_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        root_data.iter().all(|byte| *byte == 0),
        ClutchError::AlreadyInitialized,
    )?;
    root_data.copy_from_slice(&root_postimage);
    drop(root_data);

    let series = init.series_plan_id.bytes();
    let admission_component = [SERIES_ADMISSION_COMPONENT_SEED_V2];
    let admission_vault_bump_seed = [admission_vault_bump];
    invoke_founder_pda_transfer_v1(
        series_admission_vault,
        founder_link_account,
        system_program,
        link_plan.payer_debit_lamports,
        &[
            seeds::SEED_SERIES_LAMPORT_VAULT_V1,
            &series,
            &admission_component,
            &admission_vault_bump_seed,
        ],
    )?;
    require(
        series_admission_vault.lamports() == link_plan.source_balance_after
            && founder_link_account.lamports() == link_plan.destination_balance_after,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let ordinal = init.ordinal.to_le_bytes();
    let link_bump_seed = [link_bump];
    allocate_assign_founder_pda_v1(
        program_id,
        founder_link_account,
        system_program,
        SERIES_MARKET_LINK_ACCOUNT_BYTES_V1,
        &[
            seeds::SEED_PRODUCT_SERIES_MARKET_LINK,
            &series,
            &ordinal,
            &link_bump_seed,
        ],
    )?;
    let mut link_data = founder_link_account
        .try_borrow_mut_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    require(
        link_data.iter().all(|byte| *byte == 0),
        ClutchError::AlreadyInitialized,
    )?;
    link_data.copy_from_slice(&link_postimage);
    drop(link_data);

    let mut root_output = MarketLifecycleRootAccountV1::decode_buffer();
    let authenticated_root = authenticate_market_lifecycle_root_v1(
        program_id,
        root_account,
        init.coordinates.market_instance_id,
        init.coordinates.generation,
        true,
        &mut root_output,
    )?;
    let mut link_output = SeriesMarketLinkAccountV1::decode_buffer();
    let authenticated_link = authenticate_series_market_link_v1(
        program_id,
        founder_link_account,
        init.series_plan_id,
        init.ordinal,
        init.coordinates.market_instance_id,
        init.coordinates.generation,
        *root_account.key,
        true,
        &mut link_output,
    )?;
    require(
        authenticated_root.state() == &root_state
            && authenticated_root.observed_lamports() == root_balance_after
            && authenticated_link.state() == &link_state
            && authenticated_link.observed_lamports() == link_plan.destination_balance_after,
        ClutchError::MismatchedState,
    )?;
    let root_data_id = authenticated_root.data_id();
    let root_authentication_id = authenticated_root.authentication_id();
    let link_data_id = authenticated_link.data_id();
    let link_authentication_id = authenticated_link.authentication_id();
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRODUCT_MARKET_FOUNDER_CREATION_RECEIPT_DOMAIN_V1,
            program_id.as_ref(),
            &foundation_init.id.bytes(),
            &recovery.id.bytes(),
            registry_account.key.as_ref(),
            funding_account.key.as_ref(),
            root_account.key.as_ref(),
            founder_link_account.key.as_ref(),
            foundation_vault.key.as_ref(),
            series_admission_vault.key.as_ref(),
            recovery_account.key.as_ref(),
            &root_semantic_id.bytes(),
            &root_data_id.bytes(),
            &root_authentication_id.bytes(),
            &link_semantic_id.bytes(),
            &link_data_id.bytes(),
            &link_authentication_id.bytes(),
            &market_admission.id().bytes(),
            &root_poststate_receipt_id.bytes(),
            &[facts.active_foundation_accounts],
            &facts.root_rent_principal_lamports.to_le_bytes(),
            &facts.root_donation_lamports.to_le_bytes(),
            &facts.foundation_balance_before.to_le_bytes(),
            &facts.foundation_balance_after.to_le_bytes(),
            &facts.link_rent_principal_lamports.to_le_bytes(),
            &facts.link_donation_lamports.to_le_bytes(),
            &facts.series_admission_balance_before.to_le_bytes(),
            &facts.series_admission_balance_after.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedProductMarketFounderCreationV1 {
        id,
        facts,
        root_semantic_id,
        root_data_id,
        root_authentication_id,
        link_semantic_id,
        link_data_id,
        link_authentication_id,
        market_admission_receipt_id: market_admission.id(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debit(lamports: u64, collateral_atoms: u64) -> ComponentDebitV1 {
        ComponentDebitV1 {
            lamports,
            collateral_atoms,
        }
    }

    fn valid_prestate() -> FoundationVaultFundingPrestateV1 {
        FoundationVaultFundingPrestateV1 {
            quoted_market_core: debit(1_000, 0),
            pending_market_core: debit(1_000, 0),
            remaining_market_core: debit(2_000, 0),
            series_component_donations: debit(17, 9),
            observed_series_vault_lamports: 3_017,
            observed_foundation_vault_lamports: 23,
            funding_account_rent_principal_lamports: 500,
            funding_account_minimum_balance_lamports: 450,
            funding_account_observed_lamports: 507,
        }
    }

    #[test]
    fn foundation_prefund_is_donation_and_never_discounts_principal() {
        let plan = plan_foundation_vault_funding_v1(valid_prestate()).unwrap();
        assert_eq!(plan.principal_lamports, 1_000);
        assert_eq!(plan.series_vault_balance_after, 2_017);
        assert_eq!(plan.foundation_vault_donation_lamports, 23);
        assert_eq!(plan.foundation_vault_balance_after, 1_023);
    }

    #[test]
    fn source_shortfall_or_unaccounted_surplus_refuses() {
        let mut short = valid_prestate();
        short.observed_series_vault_lamports -= 1;
        assert!(plan_foundation_vault_funding_v1(short).is_err());
        let mut surplus = valid_prestate();
        surplus.observed_series_vault_lamports += 1;
        assert!(plan_foundation_vault_funding_v1(surplus).is_err());
    }

    #[test]
    fn pending_debit_must_equal_the_complete_quote() {
        let mut partial = valid_prestate();
        partial.pending_market_core.lamports -= 1;
        assert!(plan_foundation_vault_funding_v1(partial).is_err());
        let mut zero = valid_prestate();
        zero.quoted_market_core.lamports = 0;
        zero.pending_market_core.lamports = 0;
        assert!(plan_foundation_vault_funding_v1(zero).is_err());
    }

    #[test]
    fn collateral_cannot_capitalize_the_lamport_foundation() {
        let mut collateral = valid_prestate();
        collateral.quoted_market_core.collateral_atoms = 1;
        collateral.pending_market_core.collateral_atoms = 1;
        assert!(plan_foundation_vault_funding_v1(collateral).is_err());
    }

    #[test]
    fn funding_root_must_cover_both_stored_and_current_rent() {
        let mut stored = valid_prestate();
        stored.funding_account_observed_lamports = 499;
        assert!(plan_foundation_vault_funding_v1(stored).is_err());
        let mut current = valid_prestate();
        current.funding_account_minimum_balance_lamports = 508;
        assert!(plan_foundation_vault_funding_v1(current).is_err());
    }

    #[test]
    fn foundation_postbalance_overflow_refuses() {
        let mut overflow = valid_prestate();
        overflow.observed_foundation_vault_lamports = u64::MAX;
        assert!(plan_foundation_vault_funding_v1(overflow).is_err());
    }

    #[test]
    fn recovery_prefund_remains_donation_and_full_principal_moves() {
        let plan = plan_recovery_reserve_funding_v1(
            debit(700, 0),
            debit(700, 0),
            debit(900, 0),
            debit(13, 0),
            500,
            200,
            1_613,
            29,
        )
        .unwrap();
        assert_eq!(plan.payer_debit_lamports, 700);
        assert_eq!(plan.source_balance_after, 913);
        assert_eq!(plan.recovery_donation_lamports, 29);
        assert_eq!(plan.recovery_balance_after, 729);
    }

    #[test]
    fn recovery_refuses_shortfall_collateral_or_rent_reclassification() {
        assert!(plan_recovery_reserve_funding_v1(
            debit(700, 0),
            debit(700, 0),
            debit(900, 0),
            debit(13, 0),
            500,
            200,
            1_612,
            29,
        )
        .is_err());
        assert!(plan_recovery_reserve_funding_v1(
            debit(700, 1),
            debit(700, 1),
            debit(900, 0),
            debit(13, 0),
            500,
            200,
            1_613,
            29,
        )
        .is_err());
        assert!(plan_recovery_reserve_funding_v1(
            debit(700, 0),
            debit(700, 0),
            debit(900, 0),
            debit(13, 0),
            501,
            200,
            1_613,
            29,
        )
        .is_err());
    }

    #[test]
    fn pending_native_component_never_credits_destination_prefund() {
        let plan = plan_pending_native_component_v1(
            debit(400, 0),
            debit(400, 0),
            debit(800, 0),
            debit(7, 0),
            400,
            1_207,
            19,
        )
        .unwrap();
        assert_eq!(plan.payer_debit_lamports, 400);
        assert_eq!(plan.source_balance_after, 807);
        assert_eq!(plan.destination_donation_lamports, 19);
        assert_eq!(plan.destination_balance_after, 419);
    }

    #[test]
    fn pending_native_component_refuses_partial_or_unaccounted_debit() {
        assert!(plan_pending_native_component_v1(
            debit(400, 0),
            debit(399, 0),
            debit(800, 0),
            debit(7, 0),
            400,
            1_206,
            19,
        )
        .is_err());
        assert!(plan_pending_native_component_v1(
            debit(400, 0),
            debit(400, 0),
            debit(800, 0),
            debit(7, 0),
            399,
            1_207,
            19,
        )
        .is_err());
    }

    fn valid_capability_join() -> FoundationCapabilityBundleJoinV1 {
        FoundationCapabilityBundleJoinV1 {
            expected_program: Pubkey::new_from_array([31; 32]),
            executing_program: Pubkey::new_from_array([31; 32]),
            registered_series_plan_id: SeriesPlanV5Id::from_bytes([32; 32]),
            bundle_series_plan_id: SeriesPlanV5Id::from_bytes([32; 32]),
            registered_compiler_bundle_id: ContentId::from_bytes([33; 32]),
            compiler_bundle_id: ContentId::from_bytes([33; 32]),
            capability_registry_release_id: ContentId::from_bytes([34; 32]),
            bundle_registry_release_id: ContentId::from_bytes([34; 32]),
            capability_profile_id: ContentId::from_bytes([35; 32]),
            bundle_capability_profile_id: ContentId::from_bytes([35; 32]),
            programdata_sha256: ContentId::from_bytes([36; 32]),
        }
    }

    #[test]
    fn capability_join_refuses_program_or_elf_substitution() {
        let mut wrong_program = valid_capability_join();
        wrong_program.executing_program = Pubkey::new_from_array([37; 32]);
        assert!(authenticate_foundation_capability_bundle_join_v1(wrong_program).is_err());
        let mut missing_elf = valid_capability_join();
        missing_elf.programdata_sha256 = ContentId::ZERO;
        assert!(authenticate_foundation_capability_bundle_join_v1(missing_elf).is_err());
    }

    #[test]
    fn capability_join_refuses_series_bundle_release_or_profile_substitution() {
        let mut wrong_series = valid_capability_join();
        wrong_series.bundle_series_plan_id = SeriesPlanV5Id::from_bytes([38; 32]);
        assert!(authenticate_foundation_capability_bundle_join_v1(wrong_series).is_err());
        let mut wrong_bundle = valid_capability_join();
        wrong_bundle.compiler_bundle_id = ContentId::from_bytes([39; 32]);
        assert!(authenticate_foundation_capability_bundle_join_v1(wrong_bundle).is_err());
        let mut wrong_release = valid_capability_join();
        wrong_release.bundle_registry_release_id = ContentId::from_bytes([40; 32]);
        assert!(authenticate_foundation_capability_bundle_join_v1(wrong_release).is_err());
        let mut wrong_profile = valid_capability_join();
        wrong_profile.bundle_capability_profile_id = ContentId::from_bytes([41; 32]);
        assert!(authenticate_foundation_capability_bundle_join_v1(wrong_profile).is_err());
    }

    struct NoAuthority;

    impl AuthenticatedFoundationVaultInitAuthorityV1 for NoAuthority {}

    #[test]
    fn authority_defaults_to_refusal() {
        let facts = FoundationVaultInitAuthorizationFactsV1 {
            coordinates: FoundationVaultInitCoordinatesV1 {
                market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
                generation: 1,
                founder_link_id: SeriesMarketLinkV1Id::from_bytes([2; 32]),
                lifecycle_root_account: Pubkey::new_from_array([3; 32]),
                founder_link_account: Pubkey::new_from_array([19; 32]),
                lifecycle_replay_account: Pubkey::new_from_array([20; 32]),
                foundation_account_graph_id: MarketFoundationAccountGraphV2Id::from_bytes([4; 32]),
                liveness_lifecycle_id: ContentId::from_bytes([42; 32]),
            },
            series_plan_id: SeriesPlanV5Id::from_bytes([5; 32]),
            ordinal: 0,
            funding_state_account: Pubkey::new_from_array([6; 32]),
            funding_state_id: ContentId::from_bytes([7; 32]),
            funding_account_data_id: ContentId::from_bytes([21; 32]),
            funding_account_authentication_id: ContentId::from_bytes([22; 32]),
            funding_transition_sequence: 1,
            funding_reservation_receipt_id: ContentId::from_bytes([8; 32]),
            pending_debits: [ComponentDebitV1::ZERO; SERIES_FUNDING_COMPONENT_COUNT_V2],
            compiler_bundle_id: ContentId::from_bytes([9; 32]),
            registry_release_id: ContentId::from_bytes([10; 32]),
            capability_profile_id: ContentId::from_bytes([11; 32]),
            funding_terms_id: ContentId::from_bytes([12; 32]),
            funding_quote_id: ContentId::from_bytes([13; 32]),
            foundation_schedule_id: MarketFoundationScheduleV2Id::from_bytes([14; 32]),
            market_core_vault: Pubkey::new_from_array([15; 32]),
            foundation_vault: Pubkey::new_from_array([16; 32]),
            principal_refund_owner: Pubkey::new_from_array([17; 32]),
            neutral_lamport_sink: Pubkey::new_from_array([18; 32]),
            principal_lamports: 1,
            series_vault_donation_lamports: 0,
            foundation_vault_donation_lamports: 0,
            series_vault_balance_before: 1,
            series_vault_balance_after: 0,
            foundation_vault_balance_before: 0,
            foundation_vault_balance_after: 1,
        };
        assert!(NoAuthority
            .authenticate_foundation_vault_init_v1(
                &facts,
                &MarketFoundationScheduleV2 {
                    outcome_count: 1,
                    slot_principal_lamports: [0; 46],
                    founding_timeout_buckets: 1,
                },
                &MarketFoundationAccountGraphV2 {
                    market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
                    generation: 1,
                    foundation_schedule_id: MarketFoundationScheduleV2Id::from_bytes([14; 32]),
                    account_ids: [ContentId::ZERO; 46],
                },
            )
            .is_err());
    }
}
