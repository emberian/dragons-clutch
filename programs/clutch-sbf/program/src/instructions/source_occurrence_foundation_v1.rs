//! Private Product-to-Source pre-root lifecycle capitalization.
//!
//! This module has no dispatcher entry. The atomic Product founder composer
//! supplies a private preauthorization implementation after it has reserved
//! one exact FundingV2 ordinal. Only then may the pending SourceWork principal
//! move from its canonical Series vault into Source's lifecycle custody and
//! publish the occurrence/request graph consumed by SourceSeries 77/v2.

use crate::accounts::{expect_pda, require, Outcome};
use crate::error::{ClutchError, Refusal};
use crate::instructions::genesis::{
    read_rent, require_system_program, transfer_data, SYSTEM_PROGRAM_ID,
};
use crate::instructions::product_series::{
    publish_source_semantic_inputs_v1, AuthenticatedSeriesFundingAccountV2,
    AuthenticatedSourceSemanticPublicationV1,
};
use crate::seeds;
use crate::source_plane_v3::{authenticate_occurrence, runtime_key};
use crate::source_plane_v3_actions::{
    admit_source_lifecycle_v1, initialize_source_funding_custody_v1,
    persist_source_generation_request_v1, preallocate_statistic_result_lineage_v1,
    publish_source_occurrence_v1, quote_source_lifecycle_capitalization_v1,
    AuthenticatedSourceFundingCustodyV1, AuthenticatedSourceLifecycleAdmissionV1,
    PersistedSourceGenerationRequestV1, PreallocatedStatisticResultLineageV1,
    PublishedSourceOccurrenceV1, PublishedSourceSemanticInputsV1,
    SourceLifecycleCapitalizationQuoteV1,
};
use clutch_liveness::runtime_v1::RuntimeLivenessPolicyV1;
use clutch_product_series::{
    ComponentDebitV1, ContentId, SeriesFundingComponentV2, SeriesFundingPhaseV2,
    SeriesMarketDispositionV1,
};
use clutch_solana_layout::product_series::{
    SeriesFundingAccountV2, SERIES_FUNDING_ACCOUNT_BYTES_V2,
};
use clutch_source_plane_v3::FixedCodec;
use clutch_source_plane_v3_runtime::{
    AuthenticatedSourceRouteV1, OccurrenceDispositionV1, OccurrenceSourceReceiptV1,
    SourceGenerationRequestV1, SourceWorkScheduleBindingV1,
};
use solana_account_info::AccountInfo;
use solana_cpi::invoke_signed;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::vec;

const SOURCE_WORK_COMPONENT_SEED_V2: u8 = 3;
const SOURCE_WORK_CAPITALIZATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-source-work-capitalization/v1";
const PRE_ROOT_SOURCE_OCCURRENCE_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-pre-root-source-occurrence/v1";
const SOURCE_GENERATION_POLICY_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-source-generation-policy/v1";
const SOURCE_FUNDING_ACCOUNT_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/product-source-funding-account-authentication/v1";

/// Recompute Source's exact view of the unchanged pending FundingV2 account.
///
/// Product uses this same function while minting its compact pre-root
/// authority, so the later Source capitalization cannot substitute a stale
/// Funding body or a different pending reservation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn source_funding_account_authentication_id_v1(
    program_id: &Pubkey,
    funding_account: &Pubkey,
    funding_state_id: ContentId,
    funding_account_data_id: ContentId,
    funding_transition_sequence: u64,
    pending_reservation_receipt_id: ContentId,
) -> ContentId {
    ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_FUNDING_ACCOUNT_AUTHENTICATION_DOMAIN_V1,
            program_id.as_ref(),
            funding_account.as_ref(),
            &funding_state_id.bytes(),
            &funding_account_data_id.bytes(),
            &funding_transition_sequence.to_le_bytes(),
            &pending_reservation_receipt_id.bytes(),
        ])
        .to_bytes(),
    )
}

/// Exact immutable facts offered to Product's private founder preauthorization.
/// A caller cannot implement the authority trait or turn these projections
/// into authority merely by supplying matching IDs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SourceWorkCapitalizationFactsV1 {
    pub(crate) series_plan_id: ContentId,
    pub(crate) ordinal: u32,
    pub(crate) market_instance_id: ContentId,
    pub(crate) generation: u64,
    pub(crate) registry_release_id: ContentId,
    pub(crate) capability_profile_id: ContentId,
    pub(crate) compiler_bundle_id: ContentId,
    pub(crate) attachment_plan_id: ContentId,
    pub(crate) funding_account: Pubkey,
    pub(crate) funding_state_id: ContentId,
    pub(crate) funding_account_data_id: ContentId,
    pub(crate) funding_account_authentication_id: ContentId,
    pub(crate) funding_transition_sequence: u64,
    pub(crate) pending_reservation_receipt_id: ContentId,
    pub(crate) pending_source_work: ComponentDebitV1,
    pub(crate) source_route_id: ContentId,
    pub(crate) source_release_manifest_id: ContentId,
    pub(crate) source_plane_contract_id: ContentId,
    pub(crate) source_spec_id: ContentId,
    pub(crate) source_work_schedule_id: ContentId,
    pub(crate) lifecycle_id: ContentId,
    pub(crate) source_work_vault: Pubkey,
    pub(crate) source_funding_custody: Pubkey,
    pub(crate) source_principal_refund: clutch_source_plane_v3_runtime::RuntimeKey,
    pub(crate) source_vault_balance_before: u64,
    pub(crate) source_vault_balance_after: u64,
    pub(crate) custody_balance_before: u64,
    pub(crate) custody_balance_after: u64,
    pub(crate) capitalization_quote_id: ContentId,
}

/// Default-refusing semantic owner implemented only by Product's atomic
/// founder preauthorization receipt.
pub(crate) trait AuthenticatedSourceOccurrenceFoundationAuthorityV1 {
    /// Equality-check all retained Product/Funding facts and return the exact
    /// private Product preauthorization identity to bind into Source's receipt.
    fn authenticate_source_occurrence_foundation_v1(
        &self,
        _facts: &SourceWorkCapitalizationFactsV1,
    ) -> Outcome<ContentId> {
        Err(Refusal::Adapter(ClutchError::AuthorizationUnavailable))
    }
}

/// Private proof that one pending SourceWork debit was physically moved once.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedSourceWorkCapitalizationV1 {
    id: ContentId,
    product_preauthorization_id: ContentId,
    facts: SourceWorkCapitalizationFactsV1,
    quote: SourceLifecycleCapitalizationQuoteV1,
    custody: AuthenticatedSourceFundingCustodyV1,
}

impl AuthenticatedSourceWorkCapitalizationV1 {
    pub(crate) const fn id(self) -> ContentId {
        self.id
    }

    pub(crate) const fn product_preauthorization_id(self) -> ContentId {
        self.product_preauthorization_id
    }

    pub(crate) const fn custody(self) -> AuthenticatedSourceFundingCustodyV1 {
        self.custody
    }

    /// Exact pending FundingV2 debit and physical custody movement retained by
    /// Product's founder and later Source-custody retirement owners.
    pub(crate) const fn facts(self) -> SourceWorkCapitalizationFactsV1 {
        self.facts
    }

    /// Rent-derived complete lifecycle allocation. Product persists its ID and
    /// exact total so retirement cannot relabel spent principal or donations.
    pub(crate) const fn quote(self) -> SourceLifecycleCapitalizationQuoteV1 {
        self.quote
    }
}

/// Fully capitalize one Source lifecycle from FundingV2's exact pending debit.
#[allow(clippy::too_many_arguments)]
pub(crate) fn capitalize_source_work_v1<
    A: AuthenticatedSourceOccurrenceFoundationAuthorityV1 + ?Sized,
>(
    program_id: &Pubkey,
    authority: &A,
    source_route: AuthenticatedSourceRouteV1,
    publication: AuthenticatedSourceSemanticPublicationV1,
    schedule: SourceWorkScheduleBindingV1,
    funding: AuthenticatedSeriesFundingAccountV2,
    funding_account: &AccountInfo<'_>,
    source_work_vault: &AccountInfo<'_>,
    custody_account: &AccountInfo<'_>,
    source_principal_refund: clutch_source_plane_v3_runtime::RuntimeKey,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<AuthenticatedSourceWorkCapitalizationV1> {
    require_system_program(system_program)?;
    schedule
        .validate_against(source_route)
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?;
    let rent = read_rent(rent_sysvar)?;
    let quote = quote_source_lifecycle_capitalization_v1(schedule, &rent)?;
    let product_route = publication.route();
    let occurrence = publication.occurrence();
    let occurrence_id = ContentId::from_bytes(
        occurrence
            .id()
            .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
            .bytes(),
    );
    let state = funding.value().state;
    let state_id = state
        .id()
        .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
        .content_id();
    require(
        product_route.source_route_id() == source_route.route_id()
            && product_route.source_release_manifest_id() == source_route.release_manifest_id()
            && product_route.source_release_authentication_id()
                == source_route.release_authentication_id()
            && product_route.source_plane_contract_id()
                == source_route.source_plane_contract_id()
            && product_route.source_spec_id() == source_route.source_spec_id()
            && funding.account() == *funding_account.key
            && funding_account.owner == program_id
            && funding_account.is_writable
            && !funding_account.is_signer
            && !funding_account.executable
            && funding_account.data_len() == SERIES_FUNDING_ACCOUNT_BYTES_V2
            && state.phase == SeriesFundingPhaseV2::Pending
            && state.pending_disposition == Some(SeriesMarketDispositionV1::Founder)
            && state.pending_ordinal == occurrence.ordinal
            && state.pending_source_occurrence_id == occurrence_id
            && state.pending_market_instance_id == occurrence.market_instance_id.content_id()
            && state.series_plan_id == occurrence.series_plan_id
            && state.attachment_plan_id.content_id()
                == occurrence.attachment_plan_id.content_id()
            && state.compiler_bundle_id == product_route.compiler_bundle_id()
            && state.pending_debits[SeriesFundingComponentV2::SourceWork.index()]
                == publication.source_work_funding()
            && publication.source_work_funding().collateral_atoms == 0
            && publication.source_work_funding().lamports == quote.total_lamports
            && custody_account.owner == &SYSTEM_PROGRAM_ID
            && custody_account.data_is_empty()
            && custody_account.is_writable
            && !custody_account.is_signer
            && !custody_account.executable
            && custody_account.lamports() == 0,
        ClutchError::MismatchedState,
    )?;
    let funding_data = funding_account
        .try_borrow_data()
        .map_err(|_| Refusal::Adapter(ClutchError::AccountBorrowFailed))?;
    let observed = SeriesFundingAccountV2::decode(&funding_data)?;
    let funding_account_data_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[&funding_data[..]]).to_bytes(),
    );
    drop(funding_data);
    require(observed == funding.value(), ClutchError::MismatchedState)?;
    expect_pda(
        funding_account.key,
        seeds::series_funding_pda(program_id, &state.series_plan_id.bytes()),
        Some(observed.stored_bump),
    )?;
    let (expected_vault, vault_bump) = seeds::series_lamport_vault_pda(
        program_id,
        &state.series_plan_id.bytes(),
        SOURCE_WORK_COMPONENT_SEED_V2,
    );
    let (expected_custody, _) =
        seeds::source_funding_custody_pda(program_id, &schedule.lifecycle_id().bytes());
    require(
        source_work_vault.key == &expected_vault
            && custody_account.key == &expected_custody
            && source_work_vault.owner == &SYSTEM_PROGRAM_ID
            && source_work_vault.data_is_empty()
            && source_work_vault.is_writable
            && !source_work_vault.is_signer
            && !source_work_vault.executable
            && source_work_vault.key != custody_account.key,
        ClutchError::MismatchedState,
    )?;
    let source_component = state.components[SeriesFundingComponentV2::SourceWork.index()];
    let pending = state.pending_debits[SeriesFundingComponentV2::SourceWork.index()];
    let source_vault_balance_before = source_work_vault.lamports();
    let source_vault_balance_after = source_component
        .remaining_principal
        .lamports
        .checked_add(source_component.donations.lamports)
        .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?;
    require(
        source_component.remaining_principal.collateral_atoms == 0
            && source_component.donations.collateral_atoms == 0
            && source_vault_balance_before
                == source_vault_balance_after
                    .checked_add(pending.lamports)
                    .ok_or(Refusal::Adapter(ClutchError::Arithmetic))?,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let funding_account_authentication_id = source_funding_account_authentication_id_v1(
        program_id,
        funding_account.key,
        state_id,
        funding_account_data_id,
        state.transition_sequence,
        state.pending_reservation_receipt_id,
    );
    let facts = SourceWorkCapitalizationFactsV1 {
        series_plan_id: state.series_plan_id.content_id(),
        ordinal: state.pending_ordinal,
        market_instance_id: state.pending_market_instance_id,
        generation: schedule.generation(),
        registry_release_id: product_route.registry_release_id(),
        capability_profile_id: product_route.capability_profile_id(),
        compiler_bundle_id: product_route.compiler_bundle_id().content_id(),
        attachment_plan_id: state.attachment_plan_id.content_id(),
        funding_account: *funding_account.key,
        funding_state_id: state_id,
        funding_account_data_id,
        funding_account_authentication_id,
        funding_transition_sequence: state.transition_sequence,
        pending_reservation_receipt_id: state.pending_reservation_receipt_id,
        pending_source_work: pending,
        source_route_id: source_route.route_id(),
        source_release_manifest_id: source_route.release_manifest_id(),
        source_plane_contract_id: source_route.source_plane_contract_id(),
        source_spec_id: source_route.source_spec_id(),
        source_work_schedule_id: schedule.source_work_schedule_id(),
        lifecycle_id: schedule.lifecycle_id(),
        source_work_vault: *source_work_vault.key,
        source_funding_custody: *custody_account.key,
        source_principal_refund,
        source_vault_balance_before,
        source_vault_balance_after,
        custody_balance_before: 0,
        custody_balance_after: pending.lamports,
        capitalization_quote_id: quote.id,
    };
    let product_preauthorization_id =
        authority.authenticate_source_occurrence_foundation_v1(&facts)?;
    require(
        !product_preauthorization_id.is_zero(),
        ClutchError::AuthorizationUnavailable,
    )?;
    let series = state.series_plan_id.bytes();
    let component = [SOURCE_WORK_COMPONENT_SEED_V2];
    let bump = [vault_bump];
    let transfer = Instruction::new_with_bytes(
        SYSTEM_PROGRAM_ID,
        &transfer_data(pending.lamports),
        vec![
            AccountMeta::new(*source_work_vault.key, true),
            AccountMeta::new(*custody_account.key, false),
        ],
    );
    invoke_signed(
        &transfer,
        &[
            source_work_vault.clone(),
            custody_account.clone(),
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
        source_work_vault.lamports() == source_vault_balance_after
            && custody_account.lamports() == pending.lamports,
        ClutchError::SeriesCustodyDeltaMismatch,
    )?;
    let custody = initialize_source_funding_custody_v1(
        program_id,
        source_route,
        schedule,
        product_preauthorization_id,
        source_principal_refund,
        pending.lamports,
        custody_account,
        system_program,
        &rent,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_WORK_CAPITALIZATION_DOMAIN_V1,
            &product_preauthorization_id.bytes(),
            &funding_account_authentication_id.bytes(),
            &state.pending_reservation_receipt_id.bytes(),
            &occurrence_id.bytes(),
            &source_route.route_id().bytes(),
            &schedule.source_work_schedule_id().bytes(),
            &quote.id.bytes(),
            &custody.id().bytes(),
            &custody.account_data_id().bytes(),
            &custody.ledger().id().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?.bytes(),
            &source_principal_refund.bytes(),
            source_work_vault.key.as_ref(),
            custody_account.key.as_ref(),
            &source_vault_balance_before.to_le_bytes(),
            &source_vault_balance_after.to_le_bytes(),
            &pending.lamports.to_le_bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedSourceWorkCapitalizationV1 {
        id,
        product_preauthorization_id,
        facts,
        quote,
        custody,
    })
}

/// Non-copy private pre-root receipt consumed directly by Product's founder
/// composer. It commits exact physical Source accounts/data without adding
/// those accounts to the Product Foundation graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedPreRootSourceOccurrenceV1 {
    id: ContentId,
    product_preauthorization_id: ContentId,
    source_route: AuthenticatedSourceRouteV1,
    product_publication: AuthenticatedSourceSemanticPublicationV1,
    capitalization: AuthenticatedSourceWorkCapitalizationV1,
    lifecycle: AuthenticatedSourceLifecycleAdmissionV1,
    occurrence_publication: PublishedSourceOccurrenceV1,
    semantic_publication: PublishedSourceSemanticInputsV1,
    result_lineage: PreallocatedStatisticResultLineageV1,
    generation_request: PersistedSourceGenerationRequestV1,
    occurrence: OccurrenceSourceReceiptV1,
}

impl AuthenticatedPreRootSourceOccurrenceV1 {
    pub(crate) const fn id(&self) -> ContentId {
        self.id
    }

    pub(crate) const fn occurrence(&self) -> OccurrenceSourceReceiptV1 {
        self.occurrence
    }

    pub(crate) const fn capitalization(&self) -> AuthenticatedSourceWorkCapitalizationV1 {
        self.capitalization
    }

    pub(crate) const fn product_preauthorization_id(&self) -> ContentId {
        self.product_preauthorization_id
    }

    pub(crate) const fn source_route(&self) -> AuthenticatedSourceRouteV1 {
        self.source_route
    }

    pub(crate) const fn product_publication(&self) -> AuthenticatedSourceSemanticPublicationV1 {
        self.product_publication
    }

    pub(crate) const fn liveness_policy_account(&self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.lifecycle.policy_account()
    }

    pub(crate) const fn liveness_policy_data_id(&self) -> ContentId {
        self.lifecycle.policy_account_data_id()
    }

    pub(crate) const fn source_compartment_account(&self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.lifecycle.compartment_account()
    }

    pub(crate) const fn source_compartment_data_id(&self) -> ContentId {
        self.lifecycle.compartment_account_data_id()
    }

    pub(crate) const fn occurrence_account(&self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.occurrence_publication.funding().account
    }

    pub(crate) const fn occurrence_data_id(&self) -> ContentId {
        self.occurrence_publication.funding().account_data_id
    }

    pub(crate) const fn window_account(&self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.semantic_publication.window().account
    }

    pub(crate) const fn window_data_id(&self) -> ContentId {
        self.semantic_publication.window().account_data_id
    }

    pub(crate) const fn summary_account(&self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.semantic_publication.summary().account
    }

    pub(crate) const fn summary_data_id(&self) -> ContentId {
        self.semantic_publication.summary().account_data_id
    }

    pub(crate) const fn statistic_key_account(&self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.semantic_publication.statistic_key().account
    }

    pub(crate) const fn statistic_key_data_id(&self) -> ContentId {
        self.semantic_publication.statistic_key().account_data_id
    }

    pub(crate) const fn result_lineage_account(&self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.result_lineage.authenticated().lineage().lineage_account
    }

    pub(crate) const fn result_lineage_data_id(&self) -> ContentId {
        self.result_lineage.authenticated().account_data_id()
    }

    pub(crate) const fn generation_request_account(&self) -> clutch_source_plane_v3_runtime::RuntimeKey {
        self.generation_request.funding().account
    }

    pub(crate) const fn generation_request_data_id(&self) -> ContentId {
        self.generation_request.funding().account_data_id
    }
}

/// Publish every Source-owned pre-root account from the exact capitalized
/// pending occurrence. The GenerationRequest body is reconstructed here; no
/// instruction payload supplies its family, buckets, policy, or schedule.
#[allow(clippy::too_many_arguments)]
pub(crate) fn publish_pre_root_source_occurrence_v1(
    program_id: &Pubkey,
    source_route: AuthenticatedSourceRouteV1,
    publication: AuthenticatedSourceSemanticPublicationV1,
    schedule: SourceWorkScheduleBindingV1,
    liveness_policy: RuntimeLivenessPolicyV1,
    capitalization: AuthenticatedSourceWorkCapitalizationV1,
    custody_account: &AccountInfo<'_>,
    liveness_policy_account: &AccountInfo<'_>,
    source_compartment_account: &AccountInfo<'_>,
    occurrence_account: &AccountInfo<'_>,
    window_account: &AccountInfo<'_>,
    summary_account: &AccountInfo<'_>,
    statistic_key_account: &AccountInfo<'_>,
    result_lineage_account: &AccountInfo<'_>,
    generation_request_account: &AccountInfo<'_>,
    system_program: &AccountInfo<'_>,
    rent_sysvar: &AccountInfo<'_>,
) -> Outcome<AuthenticatedPreRootSourceOccurrenceV1> {
    let custody = capitalization.custody();
    let occurrence_body = publication.occurrence();
    let window = publication.window();
    let key = publication.statistic_key();
    require(
        capitalization.product_preauthorization_id() != ContentId::ZERO
            && capitalization.facts.source_route_id == source_route.route_id()
            && capitalization.facts.source_work_schedule_id
                == schedule.source_work_schedule_id()
            && capitalization.facts.source_funding_custody == *custody_account.key
            && capitalization.facts.ordinal == occurrence_body.ordinal
            && capitalization.facts.market_instance_id
                == occurrence_body.market_instance_id.content_id()
            && capitalization.facts.pending_source_work
                == publication.source_work_funding()
            && window.id().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == occurrence_body.source_window_id
            && key.id().map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                == occurrence_body.statistic_key_id,
        ClutchError::MismatchedState,
    )?;
    let lifecycle = admit_source_lifecycle_v1(
        program_id,
        source_route,
        schedule,
        liveness_policy,
        custody,
        custody_account,
        liveness_policy_account,
        source_compartment_account,
        system_program,
        rent_sysvar,
    )?;
    let occurrence_publication = publish_source_occurrence_v1(
        program_id,
        source_route,
        publication.id(),
        occurrence_body,
        custody,
        custody_account,
        occurrence_account,
        system_program,
        rent_sysvar,
    )?;
    let semantic_publication = publish_source_semantic_inputs_v1(
        program_id,
        publication,
        custody,
        custody_account,
        window_account,
        summary_account,
        statistic_key_account,
        system_program,
        rent_sysvar,
    )?;
    let result_lineage = preallocate_statistic_result_lineage_v1(
        program_id,
        source_route,
        &key,
        custody,
        custody_account,
        result_lineage_account,
        system_program,
        rent_sysvar,
    )?;
    let generation_policy_id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            SOURCE_GENERATION_POLICY_DOMAIN_V1,
            &capitalization.product_preauthorization_id().bytes(),
            &capitalization.id().bytes(),
            &lifecycle.id().bytes(),
            &occurrence_publication.id().bytes(),
            &semantic_publication.id().bytes(),
            &result_lineage.id().bytes(),
            &schedule.lifecycle_id().bytes(),
        ])
        .to_bytes(),
    );
    require(!generation_policy_id.is_zero(), ClutchError::MismatchedState)?;
    let request = SourceGenerationRequestV1 {
        source_plane_contract_id: source_route.source_plane_contract_id(),
        source_spec_id: source_route.source_spec_id(),
        repair_generation: window.repair_generation,
        first_bucket: window.start_bucket,
        required_end_bucket_exclusive: window.end_bucket_exclusive,
        generation_policy_id,
        source_work_schedule_id: schedule.source_work_schedule_id(),
    };
    let generation_request = persist_source_generation_request_v1(
        program_id,
        source_route,
        request,
        custody,
        custody_account,
        generation_request_account,
        system_program,
        rent_sysvar,
    )?;
    let occurrence = authenticate_occurrence(
        program_id,
        source_route,
        occurrence_account,
        OccurrenceDispositionV1::Created,
        &window,
        &key,
    )
    .map_err(Refusal::from)?;
    require(
        occurrence.occurrence_record_id().bytes()
            == occurrence_body
                .id()
                .map_err(|_| Refusal::Adapter(ClutchError::MismatchedState))?
                .bytes()
            && occurrence.window_id() == occurrence_body.source_window_id
            && occurrence.statistic_key_id() == occurrence_body.statistic_key_id,
        ClutchError::MismatchedState,
    )?;
    let id = ContentId::from_bytes(
        solana_sha256_hasher::hashv(&[
            PRE_ROOT_SOURCE_OCCURRENCE_DOMAIN_V1,
            &capitalization.product_preauthorization_id().bytes(),
            &capitalization.id().bytes(),
            &lifecycle.id().bytes(),
            &occurrence_publication.id().bytes(),
            &semantic_publication.id().bytes(),
            &result_lineage.id().bytes(),
            &generation_request.id().bytes(),
            &occurrence.id().bytes(),
            &source_route.release_manifest_id().bytes(),
            &source_route.release_authentication_id().bytes(),
            &source_route.route_id().bytes(),
            &source_route.clock_policy_id().bytes(),
            &source_route.source_plane_contract_id().bytes(),
            &source_route.source_spec_id().bytes(),
            occurrence_account.key.as_ref(),
            &occurrence_publication.funding().account_data_id.bytes(),
            window_account.key.as_ref(),
            &semantic_publication.window().account_data_id.bytes(),
            summary_account.key.as_ref(),
            &semantic_publication.summary().account_data_id.bytes(),
            statistic_key_account.key.as_ref(),
            &semantic_publication.statistic_key().account_data_id.bytes(),
            result_lineage_account.key.as_ref(),
            &result_lineage.authenticated().account_data_id().bytes(),
            generation_request_account.key.as_ref(),
            &generation_request.funding().account_data_id.bytes(),
            &lifecycle.policy_account_data_id().bytes(),
            &lifecycle.compartment_account_data_id().bytes(),
        ])
        .to_bytes(),
    );
    require(!id.is_zero(), ClutchError::MismatchedState)?;
    Ok(AuthenticatedPreRootSourceOccurrenceV1 {
        id,
        product_preauthorization_id: capitalization.product_preauthorization_id(),
        source_route,
        product_publication: publication,
        capitalization,
        lifecycle,
        occurrence_publication,
        semantic_publication,
        result_lineage,
        generation_request,
        occurrence,
    })
}
