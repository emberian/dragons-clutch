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
    read_rent, require_system_program, transfer_data, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_artifact::{
    authenticate_product_artifact_v1, AuthenticatedRegistryCapabilityV3,
};
use crate::instructions::product_series::{
    AuthenticatedCompiledProductSeriesBundleV5, AuthenticatedSeriesFundingAccountV2,
};
use crate::seeds;
use clutch_product_series::{
    ComponentDebitV1, ContentId, MarketFoundationAccountGraphV2Id,
    MarketFoundationScheduleV2Id, MarketInstanceV2Id, SeriesFundingComponentV2,
    SeriesFundingPhaseV2, SeriesFundingQuoteV4, SeriesFundingTermsV2, SeriesMarketDispositionV1,
    SeriesMarketLinkV1Id, SeriesPlanV5Id, SERIES_FUNDING_COMPONENT_COUNT_V2,
};
use clutch_solana_layout::product_series::{
    SeriesFundingAccountV2, SERIES_FUNDING_ACCOUNT_BYTES_V2,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

const FOUNDATION_VAULT_INIT_RECEIPT_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-foundation-vault-init-receipt/v1";
const FOUNDATION_FUNDING_ACCOUNT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-foundation-funding-account-authentication/v1";
const MARKET_CORE_COMPONENT_SEED_V2: u8 = 0;

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
/// intent, or content ID alone must never implement this trait.
pub(crate) trait AuthenticatedFoundationVaultInitAuthorityV1 {
    fn authenticate_foundation_vault_init_v1(
        &self,
        _facts: &FoundationVaultInitAuthorizationFactsV1,
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
    authority.authenticate_foundation_vault_init_v1(&facts)?;

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
            .authenticate_foundation_vault_init_v1(&facts)
            .is_err());
    }
}
