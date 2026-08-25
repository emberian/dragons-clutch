//! Chain-derived finite-Series creation and atomic ticket consumption.

use dclutch_capability_contract::CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1;
use dclutch_product_contract::capacity::CapacityProfileV1;
use dclutch_rent_contract::{RENT_CREDIT_BYTES_V1, RentCreditV1};
use dclutch_series_contract::{
    AccountMetaV1, CapitalizationAggregateV1, ConsumeTicketV1, CreateSeriesFrameV1, CreateSeriesV1,
    DerivedOccurrenceV1, IdentityV1, OccurrenceCapitalizationV1, OccurrenceTicketV1,
    SERIES_ESCROW_BYTES_V1, SERIES_ESCROW_PDA_DOMAIN_V1, SERIES_REPLAY_GUARD_BYTES_V1,
    SERIES_REPLAY_GUARD_PDA_DOMAIN_V1, SERIES_ROOT_BYTES_V1, SERIES_ROOT_PDA_DOMAIN_V1,
    SERIES_TICKET_PDA_DOMAIN_V1, SeriesRecipeV1, SeriesRootV1, VacantAccountFactsV1,
    authenticate_occurrence_capability_manifest_v1, authenticate_occurrence_source_material_v1,
    authenticate_series_capability_template_v1, plan_consume_ticket_v1, plan_create_series_v1,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

use super::{
    SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1, SERIES_CAPACITY_SCHEMA_RELEASE_ID_V1,
    SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1, SERIES_DERIVED_SCHEMA_RELEASE_ID_V1,
    SERIES_RECIPE_SCHEMA_RELEASE_ID_V3, VerticalError, authenticate_system_actor,
    authenticate_system_program, finalized, identity, observation,
};
use crate::{
    Observation, ObservedAccount,
    foundation::{self, FinalizedRecordProof, FoundMarketReport, FoundMarketState},
};

/// Finalized snapshot required to create one fully capitalized finite Series.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesCreateState {
    /// System-owned signing payer for root, escrow, guard, and finite principal.
    pub payer: ObservedAccount,
    /// Finalized immutable Series recipe.
    pub recipe: ObservedAccount,
    /// Finalization proof for `recipe`.
    pub recipe_finalization: FinalizedRecordProof,
    /// Finalized immutable aggregate proving present total principal.
    pub aggregate: ObservedAccount,
    /// Finalization proof for `aggregate`.
    pub aggregate_finalization: FinalizedRecordProof,
    /// Finalized Product capacity profile selected by the recipe.
    pub capacity_profile: ObservedAccount,
    /// Finalization proof for `capacity_profile`.
    pub capacity_profile_finalization: FinalizedRecordProof,
    /// Vacant canonical Series root destination.
    pub root_destination: ObservedAccount,
    /// Vacant canonical Series escrow destination.
    pub escrow_destination: ObservedAccount,
    /// Vacant permanent replay-guard destination.
    pub replay_guard_destination: ObservedAccount,
    /// Permanent credit whose persisted beneficiary selects every refund.
    pub rent_credit: ObservedAccount,
    /// Finalized reusable capability template selected by the recipe.
    pub capability_template: ObservedAccount,
    /// Finalization proof for `capability_template`.
    pub capability_template_finalization: FinalizedRecordProof,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
}

/// Exact finite-Series creation report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesCreateReport {
    /// Unsigned exact fifteen-account Series instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting every account and bump.
    pub observation: Observation,
    /// Canonical new Series root.
    pub root: Pubkey,
    /// Canonical finite-principal escrow.
    pub escrow: Pubkey,
    /// Canonical permanent replay guard.
    pub replay_guard: Pubkey,
    /// Exact payer debit after accounting for harmless destination dust.
    pub payer_debit: u64,
    /// Exact presently capitalized Series principal deposited into escrow.
    pub total_principal: u64,
}

/// Build the exact fifteen-account `CreateSeries` action from finalized state.
pub fn build_series_create_v1(
    program_id: Pubkey,
    state: &SeriesCreateState,
) -> Result<SeriesCreateReport, VerticalError> {
    let observation = series_create_observation(state)?;
    authenticate_system_actor(&state.payer)?;
    authenticate_system_program(&state.system_program)?;
    let rent =
        foundation::decode_rent(&state.rent_sysvar).map_err(|_| VerticalError::InvalidState)?;
    let recipe = finalized(
        program_id,
        &rent,
        &state.recipe,
        &state.recipe_finalization,
        SERIES_RECIPE_SCHEMA_RELEASE_ID_V3,
        SeriesRecipeV1::decode,
    )?;
    let aggregate = finalized(
        program_id,
        &rent,
        &state.aggregate,
        &state.aggregate_finalization,
        SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1,
        CapitalizationAggregateV1::decode,
    )?;
    let capacity = finalized(
        program_id,
        &rent,
        &state.capacity_profile,
        &state.capacity_profile_finalization,
        SERIES_CAPACITY_SCHEMA_RELEASE_ID_V1,
        CapacityProfileV1::decode,
    )?;
    super::authenticate_finalized_bytes(
        program_id,
        &rent,
        &state.capability_template,
        &state.capability_template_finalization,
        CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1,
    )?;
    let recipe_id = identity(&state.recipe.data)?;
    let aggregate_id = identity(&state.aggregate.data)?;
    if recipe.to_bytes().as_slice() != state.recipe.data.as_slice()
        || aggregate.to_bytes().as_slice() != state.aggregate.data.as_slice()
        || capacity.to_bytes().as_slice() != state.capacity_profile.data.as_slice()
        || hash(&state.capacity_profile.data).to_bytes() != recipe.capacity_profile_id.to_bytes()
    {
        return Err(VerticalError::ContentMismatch);
    }
    capacity
        .validate_artifact(104, 1)
        .and_then(|()| capacity.validate_partition(u32::from(recipe.outcome_count)))
        .map_err(|_| VerticalError::ContentMismatch)?;
    let template = authenticate_series_capability_template_v1(
        recipe.capability_template_id,
        &state.capability_template.data,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let credit = authenticate_credit(program_id, &rent, &state.rent_credit)?;
    let refund_authority = IdentityV1::new(credit.refund_authority().to_bytes())
        .map_err(|_| VerticalError::ContentMismatch)?;
    let refund = refund_authority.to_bytes();
    let recipe_id_bytes = recipe_id.to_bytes();
    let aggregate_id_bytes = aggregate_id.to_bytes();
    let (root, root_bump) = Pubkey::find_program_address(
        &[
            SERIES_ROOT_PDA_DOMAIN_V1,
            recipe_id_bytes.as_slice(),
            aggregate_id_bytes.as_slice(),
            refund.as_slice(),
        ],
        &program_id,
    );
    let (escrow, escrow_bump) =
        Pubkey::find_program_address(&[SERIES_ESCROW_PDA_DOMAIN_V1, root.as_ref()], &program_id);
    let (replay_guard, replay_guard_bump) = Pubkey::find_program_address(
        &[SERIES_REPLAY_GUARD_PDA_DOMAIN_V1, root.as_ref()],
        &program_id,
    );
    authenticate_destination(&state.root_destination, root)?;
    authenticate_destination(&state.escrow_destination, escrow)?;
    authenticate_destination(&state.replay_guard_destination, replay_guard)?;
    let wire = CreateSeriesV1 {
        refund_authority,
        root_bump,
        escrow_bump,
        replay_guard_bump,
    };
    let semantic = plan_create_series_v1(
        IdentityV1::new(root.to_bytes()).map_err(|_| VerticalError::PdaMismatch)?,
        recipe_id,
        aggregate_id,
        &recipe,
        template,
        &aggregate,
        wire,
        state.payer.lamports,
        vacancy(&state.root_destination)?,
        vacancy(&state.escrow_destination)?,
        vacancy(&state.replay_guard_destination)?,
        rent.minimum_balance(SERIES_ROOT_BYTES_V1),
        rent.minimum_balance(SERIES_ESCROW_BYTES_V1),
        rent.minimum_balance(SERIES_REPLAY_GUARD_BYTES_V1),
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let payer_debit = semantic
        .payer_before
        .checked_sub(semantic.payer_after)
        .ok_or(VerticalError::InvalidState)?;
    let accounts = series_create_accounts(state);
    validate_series_create_frame(state)?;
    Ok(SeriesCreateReport {
        instruction: Instruction {
            program_id,
            accounts,
            data: wire.to_bytes().to_vec(),
        },
        observation,
        root,
        escrow,
        replay_guard,
        payer_debit,
        total_principal: aggregate.total_principal,
    })
}

/// Finalized snapshot for atomic one-use ticket consumption and exact Found.
///
/// `found` remains the sole operator representation of the Found18 semantic
/// accounts. The additional fields are only the Series-owned frame tail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesConsumeAndFoundState {
    /// Complete chain-derived Found18 state; its sponsor is the permissionless actor.
    pub found: FoundMarketState,
    /// Mutable Series root owning outstanding-ticket replay.
    pub root: ObservedAccount,
    /// Finalized immutable recipe.
    pub recipe: ObservedAccount,
    /// Finalization proof for `recipe`.
    pub recipe_finalization: FinalizedRecordProof,
    /// Finalized immutable capitalization aggregate.
    pub aggregate: ObservedAccount,
    /// Finalization proof for `aggregate`.
    pub aggregate_finalization: FinalizedRecordProof,
    /// Finalized exact derived occurrence.
    pub derived: ObservedAccount,
    /// Finalization proof for `derived`.
    pub derived_finalization: FinalizedRecordProof,
    /// Finalized exact occurrence capitalization.
    pub capitalization: ObservedAccount,
    /// Finalization proof for `capitalization`.
    pub capitalization_finalization: FinalizedRecordProof,
    /// Mutable canonical one-use ticket.
    pub ticket: ObservedAccount,
    /// Finalized reusable capability template selected by the recipe.
    pub capability_template: ObservedAccount,
    /// Finalization proof for `capability_template`.
    pub capability_template_finalization: FinalizedRecordProof,
}

/// Exact atomic ConsumeTicket-plus-Found report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesConsumeAndFoundReport {
    /// Unsigned exact thirty-account Series instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting replay and every immutable input.
    pub observation: Observation,
    /// Fully authenticated Found subplan executed atomically by Series SBF.
    pub found: FoundMarketReport,
    /// Exact consumed occurrence index derived from the ticket.
    pub occurrence_index: u64,
    /// Exact ticket principal routed solely into Found.
    pub market_principal: u64,
    /// Exact ticket rent and unsolicited excess returned to RentCredit.
    pub ticket_refund: u64,
}

/// Construct the exact thirty-account atomic `ConsumeTicket` + Found action.
pub fn build_series_consume_ticket_and_found_v1(
    program_id: Pubkey,
    state: &SeriesConsumeAndFoundState,
) -> Result<SeriesConsumeAndFoundReport, VerticalError> {
    let observation = series_consume_observation(state)?;
    authenticate_system_actor(&state.found.sponsor)?;
    authenticate_system_program(&state.found.system_program)?;
    let rent = foundation::decode_rent(&state.found.rent_sysvar)
        .map_err(|_| VerticalError::InvalidState)?;
    let root = super::decode_owned(&state.root, program_id, SeriesRootV1::decode)?;
    if root.to_bytes().as_slice() != state.root.data.as_slice()
        || !rent.is_exempt(state.root.lamports, SERIES_ROOT_BYTES_V1)
    {
        return Err(VerticalError::InvalidState);
    }
    let recipe = finalized(
        program_id,
        &rent,
        &state.recipe,
        &state.recipe_finalization,
        SERIES_RECIPE_SCHEMA_RELEASE_ID_V3,
        SeriesRecipeV1::decode,
    )?;
    let aggregate = finalized(
        program_id,
        &rent,
        &state.aggregate,
        &state.aggregate_finalization,
        SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1,
        CapitalizationAggregateV1::decode,
    )?;
    let derived = finalized(
        program_id,
        &rent,
        &state.derived,
        &state.derived_finalization,
        SERIES_DERIVED_SCHEMA_RELEASE_ID_V1,
        DerivedOccurrenceV1::decode,
    )?;
    let capitalization = finalized(
        program_id,
        &rent,
        &state.capitalization,
        &state.capitalization_finalization,
        SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1,
        OccurrenceCapitalizationV1::decode,
    )?;
    super::authenticate_finalized_bytes(
        program_id,
        &rent,
        &state.capability_template,
        &state.capability_template_finalization,
        CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1,
    )?;
    super::authenticate_finalized_bytes(
        program_id,
        &rent,
        &state.found.resolution_material,
        &state.found.resolution_material_finalization,
        dclutch_source_contract::SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    )?;
    super::authenticate_finalized_bytes(
        program_id,
        &rent,
        &state.found.capability_manifest,
        &state.found.capability_manifest_finalization,
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let recipe_id = identity(&state.recipe.data)?;
    let aggregate_id = identity(&state.aggregate.data)?;
    let derived_id = identity(&state.derived.data)?;
    let capitalization_id = identity(&state.capitalization.data)?;
    let source_id = identity(&state.found.resolution_material.data)?;
    let source = authenticate_occurrence_source_material_v1(
        source_id,
        &state.found.resolution_material.data,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let manifest_id = identity(&state.found.capability_manifest.data)?;
    let manifest = authenticate_occurrence_capability_manifest_v1(
        recipe.capability_template_id,
        &state.capability_template.data,
        source.material_id(),
        manifest_id,
        &state.found.capability_manifest.data,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    authenticate_series_capability_template_v1(
        recipe.capability_template_id,
        &state.capability_template.data,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let root_key =
        IdentityV1::new(state.root.key.to_bytes()).map_err(|_| VerticalError::PdaMismatch)?;
    let refund = root.refund_authority.to_bytes();
    let root_recipe_id = root.recipe_id.to_bytes();
    let root_aggregate_id = root.aggregate_id.to_bytes();
    let (expected_root, _) = Pubkey::find_program_address(
        &[
            SERIES_ROOT_PDA_DOMAIN_V1,
            root_recipe_id.as_slice(),
            root_aggregate_id.as_slice(),
            refund.as_slice(),
        ],
        &program_id,
    );
    if state.root.key != expected_root
        || root.recipe_id != recipe_id
        || root.aggregate_id != aggregate_id
    {
        return Err(VerticalError::PdaMismatch);
    }
    let ticket = super::decode_owned(&state.ticket, program_id, OccurrenceTicketV1::decode)?;
    if ticket.to_bytes().as_slice() != state.ticket.data.as_slice() {
        return Err(VerticalError::InvalidState);
    }
    let index_bytes = ticket.occurrence_index.to_le_bytes();
    let (expected_ticket, ticket_bump) = Pubkey::find_program_address(
        &[
            SERIES_TICKET_PDA_DOMAIN_V1,
            state.root.key.as_ref(),
            index_bytes.as_slice(),
        ],
        &program_id,
    );
    if state.ticket.key != expected_ticket || ticket.pda_bump != ticket_bump {
        return Err(VerticalError::PdaMismatch);
    }
    let wire = ConsumeTicketV1 {
        expected_index: ticket.occurrence_index,
    };
    let semantic = plan_consume_ticket_v1(
        root,
        root_key,
        recipe_id,
        &recipe,
        aggregate_id,
        &aggregate,
        derived_id,
        &derived,
        source,
        manifest,
        capitalization_id,
        &capitalization,
        ticket,
        wire,
        state.ticket.lamports,
        state.found.rent_credit.lamports,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let found = foundation::build_found_market_and_fund_with_sponsor_credit_v1(
        program_id,
        &state.found,
        semantic.market_principal,
    )
    .map_err(map_found_error)?;
    authenticate_found_composition(&found, &semantic.found_obligations)?;
    if found.debit.total_sponsor_debit != semantic.market_principal
        || state.found.sponsor.key.to_bytes()
            != semantic.found_obligations.refund_authority.to_bytes()
    {
        return Err(VerticalError::ContentMismatch);
    }
    let mut accounts = found.instruction.accounts.clone();
    accounts.extend(series_consume_tail(state));
    validate_distinct_metas(&accounts)?;
    if accounts.len() != 30 {
        return Err(VerticalError::InvalidState);
    }
    let ticket_refund = semantic
        .ticket_lamports_before
        .checked_sub(semantic.market_principal)
        .ok_or(VerticalError::InvalidState)?;
    Ok(SeriesConsumeAndFoundReport {
        instruction: Instruction {
            program_id,
            accounts,
            data: wire.to_bytes().to_vec(),
        },
        observation,
        found,
        occurrence_index: ticket.occurrence_index,
        market_principal: semantic.market_principal,
        ticket_refund,
    })
}

fn authenticate_found_composition(
    found: &FoundMarketReport,
    obligation: &dclutch_series_contract::FoundCompositionObligationsV1,
) -> Result<(), VerticalError> {
    let identity = found.identity;
    if identity.realm_id().to_bytes() != obligation.realm_id.to_bytes()
        || identity.product_instance_id().to_bytes() != obligation.product_instance_id.to_bytes()
        || identity.claim_basis_id().to_bytes() != obligation.claim_basis_id.to_bytes()
        || identity.resolution_policy_id().to_bytes() != obligation.resolution_policy_id.to_bytes()
        || identity.capability_manifest_id().to_bytes()
            != obligation.capability_manifest_id.to_bytes()
        || identity.generation() != obligation.generation
        || hash(&identity.to_bytes()).to_bytes() != obligation.market_identity_id.to_bytes()
    {
        return Err(VerticalError::ContentMismatch);
    }
    Ok(())
}

fn map_found_error(error: foundation::FoundationError) -> VerticalError {
    match error {
        foundation::FoundationError::ObservationNotFinalized => {
            VerticalError::ObservationNotFinalized
        }
        foundation::FoundationError::ObservationMismatch => VerticalError::ObservationMismatch,
        foundation::FoundationError::AddressMismatch
        | foundation::FoundationError::DestinationNotVacant => VerticalError::PdaMismatch,
        foundation::FoundationError::InvalidOwner | foundation::FoundationError::InvalidSponsor => {
            VerticalError::InvalidOwner
        }
        foundation::FoundationError::AccountAlias => VerticalError::InvalidState,
        _ => VerticalError::ContentMismatch,
    }
}

fn series_consume_observation(
    state: &SeriesConsumeAndFoundState,
) -> Result<Observation, VerticalError> {
    let found = &state.found;
    observation(&[
        &found.sponsor,
        &found.rent_credit,
        &found.realm,
        &found.realm_finalization.staging_cursor,
        &found.product_instance,
        &found.product_instance_finalization.staging_cursor,
        &found.claim_basis,
        &found.claim_basis_finalization.staging_cursor,
        &found.capacity_profile,
        &found.capacity_profile_finalization.staging_cursor,
        &found.resolution_material,
        &found.resolution_material_finalization.staging_cursor,
        &found.capability_manifest,
        &found.capability_manifest_finalization.staging_cursor,
        &found.system_program,
        &found.rent_sysvar,
        &state.root,
        &state.recipe,
        &state.recipe_finalization.staging_cursor,
        &state.aggregate,
        &state.aggregate_finalization.staging_cursor,
        &state.derived,
        &state.derived_finalization.staging_cursor,
        &state.capitalization,
        &state.capitalization_finalization.staging_cursor,
        &state.ticket,
        &state.capability_template,
        &state.capability_template_finalization.staging_cursor,
    ])
}

fn series_consume_tail(state: &SeriesConsumeAndFoundState) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(state.root.key, false),
        AccountMeta::new_readonly(state.recipe.key, false),
        AccountMeta::new_readonly(state.aggregate.key, false),
        AccountMeta::new_readonly(state.derived.key, false),
        AccountMeta::new_readonly(state.capitalization.key, false),
        AccountMeta::new(state.ticket.key, false),
        AccountMeta::new_readonly(state.recipe_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(state.aggregate_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(state.derived_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(state.capitalization_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(state.capability_template.key, false),
        AccountMeta::new_readonly(
            state.capability_template_finalization.staging_cursor.key,
            false,
        ),
    ]
}

fn validate_distinct_metas(accounts: &[AccountMeta]) -> Result<(), VerticalError> {
    for (index, left) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index + 1)
            .any(|right| right.pubkey == left.pubkey)
        {
            return Err(VerticalError::InvalidState);
        }
    }
    Ok(())
}

fn series_create_observation(state: &SeriesCreateState) -> Result<Observation, VerticalError> {
    observation(&[
        &state.payer,
        &state.recipe,
        &state.recipe_finalization.staging_cursor,
        &state.aggregate,
        &state.aggregate_finalization.staging_cursor,
        &state.capacity_profile,
        &state.capacity_profile_finalization.staging_cursor,
        &state.root_destination,
        &state.escrow_destination,
        &state.replay_guard_destination,
        &state.rent_credit,
        &state.capability_template,
        &state.capability_template_finalization.staging_cursor,
        &state.system_program,
        &state.rent_sysvar,
    ])
}

fn authenticate_credit(
    program_id: Pubkey,
    rent: &solana_program::rent::Rent,
    account: &ObservedAccount,
) -> Result<RentCreditV1, VerticalError> {
    let credit = super::decode_owned(account, program_id, RentCreditV1::decode)?;
    let seeds = credit.pda_seeds();
    let authority = seeds.refund_authority().to_bytes();
    let (expected, bump) =
        Pubkey::find_program_address(&[seeds.domain(), authority.as_slice()], &program_id);
    if account.key != expected
        || credit.pda_bump() != bump
        || credit.to_bytes().as_slice() != account.data.as_slice()
        || !rent.is_exempt(account.lamports, RENT_CREDIT_BYTES_V1)
    {
        return Err(VerticalError::ContentMismatch);
    }
    Ok(credit)
}

fn authenticate_destination(
    account: &ObservedAccount,
    expected: Pubkey,
) -> Result<(), VerticalError> {
    if account.key == expected
        && account.owner == system_program::ID
        && !account.executable
        && account.data.is_empty()
    {
        Ok(())
    } else {
        Err(VerticalError::PdaMismatch)
    }
}

fn vacancy(account: &ObservedAccount) -> Result<VacantAccountFactsV1, VerticalError> {
    Ok(VacantAccountFactsV1 {
        lamports: account.lamports,
        owner: account.owner.to_bytes(),
        data_len: u64::try_from(account.data.len()).map_err(|_| VerticalError::InvalidState)?,
        is_executable: account.executable,
    })
}

fn series_meta(account: &ObservedAccount, is_signer: bool, is_writable: bool) -> AccountMetaV1 {
    AccountMetaV1 {
        key: account.key.to_bytes(),
        is_signer,
        is_writable,
        is_executable: account.executable,
    }
}

fn validate_series_create_frame(state: &SeriesCreateState) -> Result<(), VerticalError> {
    CreateSeriesFrameV1::validate(&[
        series_meta(&state.payer, true, true),
        series_meta(&state.recipe, false, false),
        series_meta(&state.aggregate, false, false),
        series_meta(&state.root_destination, false, true),
        series_meta(&state.escrow_destination, false, true),
        series_meta(&state.replay_guard_destination, false, true),
        series_meta(&state.rent_credit, false, false),
        series_meta(&state.system_program, false, false),
        series_meta(&state.rent_sysvar, false, false),
    ])
    .map_err(|_| VerticalError::InvalidState)?;
    let all = series_create_accounts(state);
    for (index, left) in all.iter().enumerate() {
        if all
            .iter()
            .skip(index + 1)
            .any(|right| right.pubkey == left.pubkey)
        {
            return Err(VerticalError::InvalidState);
        }
    }
    Ok(())
}

fn series_create_accounts(state: &SeriesCreateState) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(state.payer.key, true),
        AccountMeta::new_readonly(state.recipe.key, false),
        AccountMeta::new_readonly(state.aggregate.key, false),
        AccountMeta::new_readonly(state.capacity_profile.key, false),
        AccountMeta::new(state.root_destination.key, false),
        AccountMeta::new(state.escrow_destination.key, false),
        AccountMeta::new(state.replay_guard_destination.key, false),
        AccountMeta::new_readonly(state.rent_credit.key, false),
        AccountMeta::new_readonly(state.recipe_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(state.aggregate_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(
            state.capacity_profile_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(state.capability_template.key, false),
        AccountMeta::new_readonly(
            state.capability_template_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(state.system_program.key, false),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
    ]
}
