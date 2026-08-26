//! Finalized-state instruction construction for executable verticals.
//!
//! The functions here deliberately accept complete observed accounts rather
//! than a convenient client DTO.  They re-decode every persisted fact that
//! selects an instruction field, recompute all relevant PDAs, and emit the
//! exact SBF account order and privileges.  They neither sign nor submit.

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1,
};
use dclutch_core_contract::Phase;
use dclutch_dealer_contract::{
    LiquidityConfigV1, PoolState,
    frame::{
        DEALER_CONFIG_SCHEMA_RELEASE_ID_V1, DealerAccountMetaV1, DealerFrameV1, PoolPdaSeedsV1,
    },
    instruction::{DealerActionV1, DealerInstructionV1},
};
use dclutch_direct_contract::{
    CancelThroughV1, MAKER_REPLAY_ROOT_PDA_DOMAIN_V2, MakerReplayRootV2,
    adapter::{
        AdapterAccountMetaV2, AdapterActionV2, MarketPhaseV2, decode_cancel_through_instruction_v1,
        encode_cancel_through_instruction_v1, encode_close_replay_registration_instruction_v2,
        encode_close_replay_root_instruction_v2, validate_account_frame_v2,
    },
    close_replay_registration_v2, prepare_replay_root_close_v2,
};
use dclutch_general_contract::{
    BATCH_ROOT_BYTES, BatchRentObservationV1, GENERAL_CONFIG_SCHEMA_ID_V1, GENERAL_ROOT_BYTES,
    GeneralAccountFrameV1, GeneralAccountMetaV1, GeneralBatchPdaSeedsV1, GeneralBatchReplayV1,
    GeneralConfigV1, GeneralInstructionTagV1, GeneralInstructionV1, GeneralRootPdaSeedsV1,
    GeneralRootV1, open_general_batch_v1,
};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
pub use dclutch_product_contract::capacity::CAPACITY_PROFILE_SCHEMA_RELEASE_ID_V1 as SERIES_CAPACITY_SCHEMA_RELEASE_ID_V1;
use dclutch_product_contract::capacity::CapacityProfileV1;
use dclutch_rent_contract::RentCreditV1;
use dclutch_series_contract::{
    CapitalizationAggregateV1, DerivedOccurrenceV1, IdentityV1, InstantiateNextV1,
    OccurrenceCapitalizationV1, SERIES_ESCROW_PDA_DOMAIN_V1, SERIES_ROOT_PDA_DOMAIN_V1,
    SERIES_TICKET_PDA_DOMAIN_V1, SeriesEscrowV1, SeriesRecipeV1, SeriesRootV1,
    VacantAccountFactsV1, authenticate_occurrence_capability_manifest_v1,
    authenticate_occurrence_source_material_v1, plan_instantiate_next_v1,
};
pub use dclutch_series_contract::{
    SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1, SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1,
    SERIES_DERIVED_SCHEMA_RELEASE_ID_V1, SERIES_RECIPE_SCHEMA_RELEASE_ID_V3,
};
use dclutch_source_contract::{
    RetireInstructionV1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1, SourceAccountPrivilegeV1,
    SourceActionV1, SourceFrameKindV1, SourceResolutionStateV1, validate_source_frame_v1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};

use crate::{
    Finality, Observation, ObservedAccount, authenticate_rent_credit,
    foundation::{self, FinalizedRecordProof},
};

/// Refusal from finalized vertical observation or exact instruction construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerticalError {
    /// At least one input was not finalized.
    ObservationNotFinalized,
    /// Inputs did not originate from one identical finalized observation.
    ObservationMismatch,
    /// An account was not owned by its required program or was executable.
    InvalidOwner,
    /// A raw record, state account, or its canonical re-encoding was invalid.
    InvalidState,
    /// A finalized-record schema, raw PDA, or staging cursor differed.
    FinalizationMismatch,
    /// A persisted content identity or cross-record relation differed.
    ContentMismatch,
    /// A derived PDA or a claimed vacant destination differed.
    PdaMismatch,
    /// The selected lifecycle phase does not admit the action.
    InvalidPhase,
    /// A required payer/actor was not a plain System account.
    InvalidAuthority,
    /// The current immutable ABI does not provide a safe operator builder.
    AbiUnavailable,
}

/// Finalized state required to construct an exact-next Series instantiation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesInstantiateState {
    /// Permissionless System actor paying no semantic authority role.
    pub actor: ObservedAccount,
    /// Mutable Series root.
    pub root: ObservedAccount,
    /// Mutable Series escrow.
    pub escrow: ObservedAccount,
    /// Vacant destination for the derived one-use ticket.
    pub ticket: ObservedAccount,
    /// Finalized immutable recipe.
    pub recipe: ObservedAccount,
    /// Finalization proof for `recipe`.
    pub recipe_finalization: FinalizedRecordProof,
    /// Finalized immutable aggregate.
    pub aggregate: ObservedAccount,
    /// Finalization proof for `aggregate`.
    pub aggregate_finalization: FinalizedRecordProof,
    /// Finalized capacity profile.
    pub capacity_profile: ObservedAccount,
    /// Finalization proof for `capacity_profile`.
    pub capacity_profile_finalization: FinalizedRecordProof,
    /// Finalized derived occurrence.
    pub derived: ObservedAccount,
    /// Finalization proof for `derived`.
    pub derived_finalization: FinalizedRecordProof,
    /// Finalized occurrence capitalization.
    pub capitalization: ObservedAccount,
    /// Finalization proof for `capitalization`.
    pub capitalization_finalization: FinalizedRecordProof,
    /// Finalized occurrence-specific Source material.
    pub resolution_material: ObservedAccount,
    /// Finalization proof for `resolution_material`.
    pub resolution_material_finalization: FinalizedRecordProof,
    /// Finalized reusable capability template selected by the recipe.
    pub capability_template: ObservedAccount,
    /// Finalization proof for `capability_template`.
    pub capability_template_finalization: FinalizedRecordProof,
    /// Finalized occurrence-specific realized capability manifest.
    pub capability_manifest: ObservedAccount,
    /// Finalization proof for `capability_manifest`.
    pub capability_manifest_finalization: FinalizedRecordProof,
    /// Canonical System Program observation.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar observation.
    pub rent_sysvar: ObservedAccount,
}

/// Chain-derived instantiate result and independently useful exact plan facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesInstantiateReport {
    /// Unsigned exact SBF instruction.
    pub instruction: Instruction,
    /// Finalized observation selecting every account and replay field.
    pub observation: Observation,
    /// Exact ticket PDA derived from root state.
    pub ticket: Pubkey,
    /// Exact occurrence index derived from the root, never supplied by a client.
    pub occurrence_index: u64,
}

/// Construct the canonical 22-account V1 instantiate-next frame.
pub fn build_series_instantiate_next_v1(
    program_id: Pubkey,
    state: &SeriesInstantiateState,
) -> Result<SeriesInstantiateReport, VerticalError> {
    let observation = observation(&[
        &state.actor,
        &state.root,
        &state.escrow,
        &state.ticket,
        &state.recipe,
        &state.aggregate,
        &state.capacity_profile,
        &state.derived,
        &state.capitalization,
        &state.resolution_material,
        &state.capability_template,
        &state.capability_manifest,
        &state.system_program,
        &state.rent_sysvar,
    ])?;
    authenticate_system_actor(&state.actor)?;
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
    authenticate_finalized_bytes(
        program_id,
        &rent,
        &state.resolution_material,
        &state.resolution_material_finalization,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    )?;
    authenticate_finalized_bytes(
        program_id,
        &rent,
        &state.capability_template,
        &state.capability_template_finalization,
        CAPABILITY_TEMPLATE_SCHEMA_RELEASE_ID_V1,
    )?;
    authenticate_finalized_bytes(
        program_id,
        &rent,
        &state.capability_manifest,
        &state.capability_manifest_finalization,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    let source_material_id = identity(&state.resolution_material.data)?;
    let source_material = authenticate_occurrence_source_material_v1(
        source_material_id,
        &state.resolution_material.data,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let capability_manifest_id = identity(&state.capability_manifest.data)?;
    let capability_manifest = authenticate_occurrence_capability_manifest_v1(
        recipe.capability_template_id,
        &state.capability_template.data,
        source_material.material_id(),
        capability_manifest_id,
        &state.capability_manifest.data,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    if state.capacity_profile.data != capacity.to_bytes()
        || hash(&capacity.to_bytes()).to_bytes() != recipe.capacity_profile_id.to_bytes()
    {
        return Err(VerticalError::ContentMismatch);
    }
    capacity
        .validate_artifact(104, 1)
        .map_err(|_| VerticalError::ContentMismatch)?;
    capacity
        .validate_partition(u32::from(recipe.outcome_count))
        .map_err(|_| VerticalError::ContentMismatch)?;
    let root = decode_owned(&state.root, program_id, SeriesRootV1::decode)?;
    let escrow = decode_owned(&state.escrow, program_id, SeriesEscrowV1::decode)?;
    if root.to_bytes().as_slice() != state.root.data.as_slice()
        || escrow.to_bytes().as_slice() != state.escrow.data.as_slice()
        || !rent.is_exempt(state.root.lamports, state.root.data.len())
    {
        return Err(VerticalError::InvalidState);
    }
    let recipe_id = identity(&state.recipe.data)?;
    let aggregate_id = identity(&state.aggregate.data)?;
    let root_key =
        IdentityV1::new(state.root.key.to_bytes()).map_err(|_| VerticalError::PdaMismatch)?;
    let refund = root.refund_authority.to_bytes();
    let (expected_root, root_bump) = Pubkey::find_program_address(
        &[
            SERIES_ROOT_PDA_DOMAIN_V1,
            &recipe_id.to_bytes(),
            &aggregate_id.to_bytes(),
            &refund,
        ],
        &program_id,
    );
    if state.root.key != expected_root || root.pda_bump != root_bump {
        return Err(VerticalError::PdaMismatch);
    }
    let (expected_escrow, escrow_bump) = Pubkey::find_program_address(
        &[SERIES_ESCROW_PDA_DOMAIN_V1, state.root.key.as_ref()],
        &program_id,
    );
    if state.escrow.key != expected_escrow || escrow.pda_bump != escrow_bump {
        return Err(VerticalError::PdaMismatch);
    }
    let index = root.next_occurrence_index;
    let (ticket, ticket_bump) = Pubkey::find_program_address(
        &[
            SERIES_TICKET_PDA_DOMAIN_V1,
            state.root.key.as_ref(),
            &index.to_le_bytes(),
        ],
        &program_id,
    );
    if state.ticket.key != ticket
        || state.ticket.owner != system_program::ID
        || state.ticket.executable
        || !state.ticket.data.is_empty()
    {
        return Err(VerticalError::PdaMismatch);
    }
    let wire = InstantiateNextV1 {
        expected_index: index,
        expected_time: root.next_occurrence_time,
        ticket_bump,
    };
    plan_instantiate_next_v1(
        root,
        root_key,
        escrow,
        recipe_id,
        &recipe,
        aggregate_id,
        &aggregate,
        identity(&state.derived.data)?,
        &derived,
        source_material,
        capability_manifest,
        identity(&state.capitalization.data)?,
        &capitalization,
        wire,
        observation.unix_timestamp,
        rent.minimum_balance(dclutch_series_contract::SERIES_ESCROW_BYTES_V1),
        rent.minimum_balance(dclutch_series_contract::OCCURRENCE_TICKET_BYTES_V1),
        state.escrow.lamports,
        VacantAccountFactsV1 {
            lamports: state.ticket.lamports,
            owner: state.ticket.owner.to_bytes(),
            data_len: u64::try_from(state.ticket.data.len())
                .map_err(|_| VerticalError::InvalidState)?,
            is_executable: state.ticket.executable,
        },
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let data = wire.to_bytes().to_vec();
    Ok(SeriesInstantiateReport {
        instruction: Instruction {
            program_id,
            accounts: series_instantiate_accounts(state),
            data,
        },
        observation,
        ticket,
        occurrence_index: index,
    })
}

fn series_instantiate_accounts(state: &SeriesInstantiateState) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new_readonly(state.actor.key, true),
        AccountMeta::new(state.root.key, false),
        AccountMeta::new_readonly(state.recipe.key, false),
        AccountMeta::new_readonly(state.aggregate.key, false),
        AccountMeta::new_readonly(state.capacity_profile.key, false),
        AccountMeta::new_readonly(state.derived.key, false),
        AccountMeta::new_readonly(state.capitalization.key, false),
        AccountMeta::new(state.escrow.key, false),
        AccountMeta::new(state.ticket.key, false),
        AccountMeta::new_readonly(state.recipe_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(state.aggregate_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(
            state.capacity_profile_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(state.derived_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(state.capitalization_finalization.staging_cursor.key, false),
        AccountMeta::new_readonly(state.resolution_material.key, false),
        AccountMeta::new_readonly(
            state.resolution_material_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(state.capability_template.key, false),
        AccountMeta::new_readonly(
            state.capability_template_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(state.capability_manifest.key, false),
        AccountMeta::new_readonly(
            state.capability_manifest_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(state.system_program.key, false),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
    ]
}

/// Finalized state required to construct the fixed Dealer reset-ladder frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerResetState {
    /// Canonical mutable Market root.
    pub market: ObservedAccount,
    /// Mutable Dealer Pool.
    pub pool: ObservedAccount,
    /// Finalized immutable Dealer configuration.
    pub config: ObservedAccount,
    /// Finalization proof for `config`.
    pub config_finalization: FinalizedRecordProof,
}

/// Constructed reset instruction with its immutable config identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerResetReport {
    /// Exact unsigned four-account instruction.
    pub instruction: Instruction,
    /// Finalized observation selecting the replay sequence.
    pub observation: Observation,
    /// SHA-256 config identity selected from the finalized config bytes.
    pub config_id: [u8; 32],
}

/// Finalized state required to close one retired, zero-live Direct replay root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCloseReplayRootState {
    /// Retiring canonical Market root.
    pub market: ObservedAccount,
    /// Direct's mutable maker replay root.
    pub replay_root: ObservedAccount,
    /// Permanent credit selected by the replay root's immutable rent payer.
    pub rent_credit: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
}

/// Chain-derived Direct replay-root-close instruction and final rent destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCloseReplayRootReport {
    /// Exact unsigned five-account Direct V2 instruction.
    pub instruction: Instruction,
    /// Finalized observation selecting every close fact.
    pub observation: Observation,
    /// Immutable rent-credit beneficiary selected from the replay root.
    pub rent_beneficiary: Pubkey,
}

/// Finalized state plus exact signed payload for a Direct O(1) cancel-through.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCancelThroughState {
    /// Canonical Market holding the immutable generation and lifecycle phase.
    pub market: ObservedAccount,
    /// Canonical maker replay root.
    pub replay_root: ObservedAccount,
    /// Instructions sysvar, authenticated by runtime when the transaction executes.
    pub instructions_sysvar: ObservedAccount,
    /// Exact untrusted signed Direct instruction bytes, strictly decoded and re-encoded here.
    pub signed_instruction: Vec<u8>,
}

/// Constructed cancellation instruction whose maker identity is only from root and signed bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCancelThroughReport {
    /// Exact unsigned Direct V2 instruction.
    pub instruction: Instruction,
    /// Finalized account observation used for all persisted facts.
    pub observation: Observation,
}

/// Finalized Market and replay root required to close Direct registration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCloseReplayRegistrationState {
    /// Retiring canonical Market.
    pub market: ObservedAccount,
    /// Mutable canonical maker replay root.
    pub replay_root: ObservedAccount,
}

/// Chain-derived Direct registration-close instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCloseReplayRegistrationReport {
    /// Exact unsigned two-account Direct V2 instruction.
    pub instruction: Instruction,
    /// One finalized observation selecting the Market phase and replay root.
    pub observation: Observation,
}

mod bearer;
mod series_lifecycle;
mod source;

pub use bearer::*;
pub use series_lifecycle::*;
pub use source::*;
mod dealer;
pub use dealer::*;

/// Construct Direct's exact permissionless registration-close frame.
pub fn build_direct_close_replay_registration_v2(
    program_id: Pubkey,
    state: &DirectCloseReplayRegistrationState,
) -> Result<DirectCloseReplayRegistrationReport, VerticalError> {
    let observation = observation(&[&state.market, &state.replay_root])?;
    let root = decode_owned(&state.replay_root, program_id, MakerReplayRootV2::decode)?;
    let mut encoded = [0; dclutch_direct_contract::MAKER_REPLAY_ROOT_BYTES_V2];
    root.encode(&mut encoded)
        .map_err(|_| VerticalError::InvalidState)?;
    if state.replay_root.data.as_slice() != encoded {
        return Err(VerticalError::InvalidState);
    }
    let (expected, bump) = Pubkey::find_program_address(
        &[
            MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
            root.market(),
            &root.generation().to_le_bytes(),
            root.maker(),
        ],
        &program_id,
    );
    if state.replay_root.key != expected || root.bump() != bump {
        return Err(VerticalError::PdaMismatch);
    }
    let market = direct_market(program_id, &state.market)?;
    if root.market() != state.market.key.as_ref()
        || root.generation() != market.generation
        || market.phase != Phase::Retiring
    {
        return Err(VerticalError::InvalidPhase);
    }
    close_replay_registration_v2(root, MarketPhaseV2::Retiring)
        .map_err(|_| VerticalError::InvalidPhase)?;
    let frame = [
        AdapterAccountMetaV2 {
            key: state.market.key.to_bytes(),
            is_signer: false,
            is_writable: true,
        },
        AdapterAccountMetaV2 {
            key: state.replay_root.key.to_bytes(),
            is_signer: false,
            is_writable: true,
        },
    ];
    validate_account_frame_v2(AdapterActionV2::CloseReplayRegistration, 1, &frame)
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(DirectCloseReplayRegistrationReport {
        instruction: Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(state.market.key, false),
                AccountMeta::new(state.replay_root.key, false),
            ],
            data: encode_close_replay_registration_instruction_v2().to_vec(),
        },
        observation,
    })
}

/// Build Direct's three-account cancel-through frame from the decoded root and
/// canonical signed message. The native signature itself remains checked by
/// the runtime against the immediately preceding instruction.
pub fn build_direct_cancel_through_v1(
    program_id: Pubkey,
    state: &DirectCancelThroughState,
) -> Result<DirectCancelThroughReport, VerticalError> {
    let observation = observation(&[
        &state.market,
        &state.replay_root,
        &state.instructions_sysvar,
    ])?;
    let root = decode_owned(&state.replay_root, program_id, MakerReplayRootV2::decode)?;
    let mut root_bytes = [0; dclutch_direct_contract::MAKER_REPLAY_ROOT_BYTES_V2];
    root.encode(&mut root_bytes)
        .map_err(|_| VerticalError::InvalidState)?;
    if root_bytes.as_slice() != state.replay_root.data.as_slice() {
        return Err(VerticalError::InvalidState);
    }
    let (expected, bump) = Pubkey::find_program_address(
        &[
            MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
            root.market(),
            &root.generation().to_le_bytes(),
            root.maker(),
        ],
        &program_id,
    );
    if state.replay_root.key != expected || root.bump() != bump {
        return Err(VerticalError::PdaMismatch);
    }
    let market = source_market(program_id, &state.market)?;
    if root.market() != state.market.key.as_ref()
        || root.generation() != market.generation
        || !matches!(
            market.phase,
            Phase::Open | Phase::Resolved | Phase::Retiring
        )
    {
        return Err(VerticalError::InvalidPhase);
    }
    if state.instructions_sysvar.key != sysvar::instructions::ID
        || state.instructions_sysvar.owner != sysvar::ID
        || state.instructions_sysvar.executable
    {
        return Err(VerticalError::InvalidState);
    }
    let message = decode_cancel_through_instruction_v1(&state.signed_instruction)
        .map_err(|_| VerticalError::InvalidState)?;
    let expected_message = CancelThroughV1::new(root, message.minimum_live_nonce())
        .map_err(|_| VerticalError::InvalidPhase)?;
    if message.signed_preimage() != expected_message.signed_preimage() {
        return Err(VerticalError::ContentMismatch);
    }
    let frame = [
        AdapterAccountMetaV2 {
            key: state.market.key.to_bytes(),
            is_signer: false,
            is_writable: false,
        },
        AdapterAccountMetaV2 {
            key: state.replay_root.key.to_bytes(),
            is_signer: false,
            is_writable: true,
        },
        AdapterAccountMetaV2 {
            key: state.instructions_sysvar.key.to_bytes(),
            is_signer: false,
            is_writable: false,
        },
    ];
    validate_account_frame_v2(AdapterActionV2::CancelThrough, 1, &frame)
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(DirectCancelThroughReport {
        instruction: Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new_readonly(state.market.key, false),
                AccountMeta::new(state.replay_root.key, false),
                AccountMeta::new_readonly(state.instructions_sysvar.key, false),
            ],
            data: encode_cancel_through_instruction_v1(message).to_vec(),
        },
        observation,
    })
}

/// Finalized state required to retire one terminal Source-resolution child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRetireResolutionState {
    /// Terminal Source-resolution state to close.
    pub resolution_state: ObservedAccount,
    /// Canonical Market which owns the direct-child count.
    pub market: ObservedAccount,
    /// Permanent credit selected by the Source state's immutable beneficiary.
    pub rent_credit: ObservedAccount,
    /// Canonical Clock sysvar observed with the terminal state.
    pub clock_sysvar: ObservedAccount,
}

/// Chain-derived Source-resolution retirement instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceRetireResolutionReport {
    /// Exact unsigned four-account Source V1 instruction.
    pub instruction: Instruction,
    /// Finalized observation selecting terminal timing and child replay state.
    pub observation: Observation,
    /// Exact Market child count guarded in the emitted wire.
    pub expected_market_child_count: u64,
}

/// Finalized state required to permissionlessly open the next General batch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralOpenBatchState {
    /// Permissionless System actor capitalizing this finite batch.
    pub actor: ObservedAccount,
    /// Canonical Market bound to the General root and config.
    pub market: ObservedAccount,
    /// Finalized immutable General configuration.
    pub config: ObservedAccount,
    /// Mutable canonical General root.
    pub root: ObservedAccount,
    /// Prefunded vacant PDA destination for the derived batch.
    pub batch: ObservedAccount,
    /// Permanent RentCredit selected by the persisted General root.
    pub rent_credit: ObservedAccount,
    /// Finalization proof for the immutable configuration.
    pub config_finalization: FinalizedRecordProof,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar used for exact batch capitalization.
    pub rent_sysvar: ObservedAccount,
    /// Canonical Clock sysvar used for collection timing.
    pub clock_sysvar: ObservedAccount,
}

/// Exact next-batch General instruction derived from finalized chain state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralOpenBatchReport {
    /// Exact unsigned ten-account General instruction.
    pub instruction: Instruction,
    /// Finalized observation which selected every replay field.
    pub observation: Observation,
    /// Derived batch PDA selected from the persisted root sequence.
    pub batch: Pubkey,
    /// Persisted next batch sequence, never caller supplied.
    pub sequence: u64,
}

/// Construct the exact General V1 `OpenBatch` frame without caller-selected
/// generation, sequence, config, batch rent, or rent destination.
pub fn build_general_open_batch_v1(
    program_id: Pubkey,
    state: &GeneralOpenBatchState,
) -> Result<GeneralOpenBatchReport, VerticalError> {
    let observation = observation(&[
        &state.actor,
        &state.market,
        &state.config,
        &state.root,
        &state.batch,
        &state.rent_credit,
        &state.config_finalization.staging_cursor,
        &state.system_program,
        &state.rent_sysvar,
        &state.clock_sysvar,
    ])?;
    authenticate_system_actor(&state.actor)?;
    authenticate_system_program(&state.system_program)?;
    let rent =
        foundation::decode_rent(&state.rent_sysvar).map_err(|_| VerticalError::InvalidState)?;
    let clock = decode_clock(&state.clock_sysvar)?;
    let config = finalized(
        program_id,
        &rent,
        &state.config,
        &state.config_finalization,
        GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes(),
        GeneralConfigV1::decode,
    )?;
    if state.config.data != config.to_bytes() {
        return Err(VerticalError::InvalidState);
    }
    let config_id = dclutch_general_contract::ContentId::new(hash(&state.config.data).to_bytes())
        .map_err(|_| VerticalError::ContentMismatch)?;
    let root = decode_owned(&state.root, program_id, GeneralRootV1::decode)?;
    let mut root_bytes = [0; GENERAL_ROOT_BYTES];
    root.encode(&mut root_bytes)
        .map_err(|_| VerticalError::InvalidState)?;
    if state.root.data != root_bytes {
        return Err(VerticalError::InvalidState);
    }
    let root_seeds = GeneralRootPdaSeedsV1::new(root.market(), root.generation(), config_id)
        .map_err(|_| VerticalError::PdaMismatch)?;
    let (expected_root, _) =
        Pubkey::find_program_address(&root_seeds.seed_components(), &program_id);
    if state.root.key != expected_root
        || root.config_id() != config_id
        || root.generation() != config.generation()
    {
        return Err(VerticalError::PdaMismatch);
    }
    let market = source_market(program_id, &state.market)?;
    if root.market() != state.market.key.to_bytes()
        || market.generation != config.generation()
        || market.claim_basis_id != config.claim_basis_id().to_bytes()
    {
        return Err(VerticalError::ContentMismatch);
    }
    let sequence = root.next_batch_sequence();
    let batch_seeds = GeneralBatchPdaSeedsV1::new(state.root.key.to_bytes(), sequence)
        .map_err(|_| VerticalError::PdaMismatch)?;
    let (batch, _) = Pubkey::find_program_address(&batch_seeds.seed_components(), &program_id);
    if state.batch.key != batch
        || state.batch.owner != system_program::ID
        || state.batch.executable
        || !state.batch.data.is_empty()
    {
        return Err(VerticalError::PdaMismatch);
    }
    authenticate_rent_credit_at_key(program_id, &state.rent_credit, root.rent_beneficiary())
        .map_err(|_| VerticalError::ContentMismatch)?;
    let args = (
        program_id,
        state,
        config_id,
        config,
        root,
        rent.minimum_balance(BATCH_ROOT_BYTES),
        clock.slot,
        sequence,
        observation,
        batch,
    );
    match config.outcome_count() {
        2 => general_open_batch_instruction::<2>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        3 => general_open_batch_instruction::<3>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        4 => general_open_batch_instruction::<4>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        5 => general_open_batch_instruction::<5>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        6 => general_open_batch_instruction::<6>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        7 => general_open_batch_instruction::<7>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        8 => general_open_batch_instruction::<8>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        9 => general_open_batch_instruction::<9>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        10 => general_open_batch_instruction::<10>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        11 => general_open_batch_instruction::<11>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        12 => general_open_batch_instruction::<12>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        13 => general_open_batch_instruction::<13>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        14 => general_open_batch_instruction::<14>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        15 => general_open_batch_instruction::<15>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        16 => general_open_batch_instruction::<16>(
            args.0, args.1, args.2, args.3, args.4, args.5, args.6, args.7, args.8, args.9,
        ),
        _ => Err(VerticalError::InvalidState),
    }
}

#[allow(clippy::too_many_arguments)]
fn general_open_batch_instruction<const N: usize>(
    program_id: Pubkey,
    state: &GeneralOpenBatchState,
    config_id: dclutch_general_contract::ContentId,
    config: GeneralConfigV1,
    root: GeneralRootV1,
    batch_rent: u64,
    slot: u64,
    sequence: u64,
    observation: Observation,
    batch: Pubkey,
) -> Result<GeneralOpenBatchReport, VerticalError> {
    let replay = GeneralBatchReplayV1 {
        generation: root.generation(),
        batch_sequence: sequence,
    };
    let metas = [
        general_meta(&state.actor, true, true),
        general_meta(&state.market, false, false),
        general_meta(&state.config, false, false),
        general_meta(&state.config_finalization.staging_cursor, false, false),
        general_meta(&state.root, false, true),
        general_meta(&state.batch, false, true),
        general_meta(&state.rent_credit, false, true),
        general_meta(&state.system_program, false, false),
        general_meta(&state.rent_sysvar, false, false),
        general_meta(&state.clock_sysvar, false, false),
    ];
    let frame = GeneralAccountFrameV1::new(GeneralInstructionTagV1::OpenBatch, 0, &metas)
        .map_err(|_| VerticalError::InvalidState)?;
    open_general_batch_v1(
        frame,
        replay,
        config_id,
        config,
        root,
        BatchRentObservationV1 {
            exact_batch_rent_lamports: batch_rent,
            precreation_lamports: state.batch.lamports,
        },
        slot,
    )
    .map_err(|_| VerticalError::InvalidPhase)?;
    let wire = GeneralInstructionV1::<N>::OpenBatch(replay);
    let mut data = vec![
        0;
        wire.encoded_len()
            .map_err(|_| VerticalError::InvalidState)?
    ];
    wire.encode(&mut data)
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(GeneralOpenBatchReport {
        instruction: Instruction {
            program_id,
            accounts: metas
                .iter()
                .map(|m| {
                    if m.is_writable {
                        AccountMeta::new(Pubkey::new_from_array(m.key), m.is_signer)
                    } else {
                        AccountMeta::new_readonly(Pubkey::new_from_array(m.key), m.is_signer)
                    }
                })
                .collect(),
            data,
        },
        observation,
        batch,
        sequence,
    })
}

fn general_meta(
    account: &ObservedAccount,
    is_signer: bool,
    is_writable: bool,
) -> GeneralAccountMetaV1 {
    GeneralAccountMetaV1 {
        key: account.key.to_bytes(),
        is_signer,
        is_writable,
        is_executable: account.executable,
    }
}

/// Construct the exact Source V1 terminal-resolution retirement frame.
///
/// The expected generation and child count are copied only from decoded state
/// and Market state. The terminal-time check runs against exact Clock sysvar
/// bytes, never an operator wall clock.
pub fn build_source_retire_resolution_v1(
    program_id: Pubkey,
    state: &SourceRetireResolutionState,
) -> Result<SourceRetireResolutionReport, VerticalError> {
    let observation = observation(&[
        &state.resolution_state,
        &state.market,
        &state.rent_credit,
        &state.clock_sysvar,
    ])?;
    let clock = decode_clock(&state.clock_sysvar)?;
    let source = decode_owned(
        &state.resolution_state,
        program_id,
        SourceResolutionStateV1::decode,
    )?;
    if source.to_bytes().as_slice() != state.resolution_state.data.as_slice() {
        return Err(VerticalError::InvalidState);
    }
    let seeds = source.pda_seeds();
    let (expected_state, bump) = Pubkey::find_program_address(
        &[seeds.domain(), &seeds.market(), &seeds.generation_le()],
        &program_id,
    );
    if state.resolution_state.key != expected_state || seeds.bump() != bump {
        return Err(VerticalError::PdaMismatch);
    }
    let market = source_market(program_id, &state.market)?;
    if source.market() != state.market.key.to_bytes()
        || source.generation() != market.generation
        || source.material_id().to_bytes() != market.resolution_policy_id
        || market.phase == Phase::Retired
    {
        return Err(VerticalError::ContentMismatch);
    }
    let beneficiary = Pubkey::new_from_array(source.rent_beneficiary());
    authenticate_rent_credit(program_id, &state.rent_credit, beneficiary)
        .map_err(|_| VerticalError::ContentMismatch)?;
    let mut transition = source;
    transition
        .retire(
            source.generation(),
            clock.unix_timestamp,
            market.child_count,
            market.child_count,
        )
        .map_err(|_| VerticalError::InvalidPhase)?;
    let wire = RetireInstructionV1::new(
        SourceActionV1::RetireResolution,
        source.generation(),
        market.child_count,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let frame = [
        SourceAccountPrivilegeV1 {
            key: state.resolution_state.key.to_bytes(),
            is_signer: false,
            is_writable: true,
            is_executable: false,
        },
        SourceAccountPrivilegeV1 {
            key: state.market.key.to_bytes(),
            is_signer: false,
            is_writable: true,
            is_executable: false,
        },
        SourceAccountPrivilegeV1 {
            key: state.rent_credit.key.to_bytes(),
            is_signer: false,
            is_writable: true,
            is_executable: false,
        },
        SourceAccountPrivilegeV1 {
            key: state.clock_sysvar.key.to_bytes(),
            is_signer: false,
            is_writable: false,
            is_executable: false,
        },
    ];
    validate_source_frame_v1(SourceFrameKindV1::RetireResolution, &frame)
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(SourceRetireResolutionReport {
        instruction: Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(state.resolution_state.key, false),
                AccountMeta::new(state.market.key, false),
                AccountMeta::new(state.rent_credit.key, false),
                AccountMeta::new_readonly(state.clock_sysvar.key, false),
            ],
            data: wire.to_bytes().to_vec(),
        },
        observation,
        expected_market_child_count: market.child_count,
    })
}

#[derive(Clone, Copy)]
struct SourceMarketFacts {
    generation: u64,
    phase: Phase,
    child_count: u64,
    outcome_count: u8,
    resolution_policy_id: [u8; 32],
    claim_basis_id: [u8; 32],
    rent_refund: [u8; 32],
}

fn source_market(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<SourceMarketFacts, VerticalError> {
    match decode_market_outcome_count(&account.data).map_err(|_| VerticalError::InvalidState)? {
        2 => source_market_width::<2>(program_id, account),
        3 => source_market_width::<3>(program_id, account),
        4 => source_market_width::<4>(program_id, account),
        5 => source_market_width::<5>(program_id, account),
        6 => source_market_width::<6>(program_id, account),
        7 => source_market_width::<7>(program_id, account),
        8 => source_market_width::<8>(program_id, account),
        9 => source_market_width::<9>(program_id, account),
        10 => source_market_width::<10>(program_id, account),
        11 => source_market_width::<11>(program_id, account),
        12 => source_market_width::<12>(program_id, account),
        13 => source_market_width::<13>(program_id, account),
        14 => source_market_width::<14>(program_id, account),
        15 => source_market_width::<15>(program_id, account),
        16 => source_market_width::<16>(program_id, account),
        _ => Err(VerticalError::InvalidState),
    }
}

fn source_market_width<const N: usize>(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<SourceMarketFacts, VerticalError> {
    let market: CategoricalMarketV1<N> =
        decode_owned(account, program_id, CategoricalMarketV1::decode)?;
    let encoded_len =
        CategoricalMarketV1::<N>::encoded_len().map_err(|_| VerticalError::InvalidState)?;
    let mut canonical = vec![0; encoded_len];
    market
        .encode(&mut canonical)
        .map_err(|_| VerticalError::InvalidState)?;
    if account.data != canonical {
        return Err(VerticalError::InvalidState);
    }
    let identity = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[crate::MARKET_SEED, &identity], &program_id);
    if account.key != expected {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(SourceMarketFacts {
        generation: market.root().identity().generation(),
        phase: market.root().phase(),
        child_count: market.root().outstanding_children(),
        outcome_count: u8::try_from(N).map_err(|_| VerticalError::InvalidState)?,
        resolution_policy_id: market.root().identity().resolution_policy_id().to_bytes(),
        claim_basis_id: market.root().identity().claim_basis_id().to_bytes(),
        rent_refund: market.root().rent_refund(),
    })
}

/// Construct the exact Direct V2 replay-root-close frame.
///
/// This route is permissionless: the builder has no maker, key, price, or
/// limit input. It refuses a root whose registration is still open, whose live
/// intent count is nonzero, or whose Market has not reached retirement.
pub fn build_direct_close_replay_root_v2(
    program_id: Pubkey,
    state: &DirectCloseReplayRootState,
) -> Result<DirectCloseReplayRootReport, VerticalError> {
    let observation = observation(&[
        &state.market,
        &state.replay_root,
        &state.rent_credit,
        &state.system_program,
        &state.rent_sysvar,
    ])?;
    authenticate_system_program(&state.system_program)?;
    let _rent =
        foundation::decode_rent(&state.rent_sysvar).map_err(|_| VerticalError::InvalidState)?;
    let root = decode_owned(&state.replay_root, program_id, MakerReplayRootV2::decode)?;
    let mut canonical_root = [0; dclutch_direct_contract::MAKER_REPLAY_ROOT_BYTES_V2];
    root.encode(&mut canonical_root)
        .map_err(|_| VerticalError::InvalidState)?;
    if canonical_root.as_slice() != state.replay_root.data.as_slice() {
        return Err(VerticalError::InvalidState);
    }
    let market = direct_market(program_id, &state.market)?;
    if market.phase != Phase::Retiring
        || root.market() != state.market.key.as_ref()
        || root.generation() != market.generation
    {
        return Err(VerticalError::InvalidPhase);
    }
    let (expected_root, bump) = Pubkey::find_program_address(
        &[
            MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
            state.market.key.as_ref(),
            &root.generation().to_le_bytes(),
            root.maker(),
        ],
        &program_id,
    );
    if state.replay_root.key != expected_root || root.bump() != bump {
        return Err(VerticalError::PdaMismatch);
    }
    let close = prepare_replay_root_close_v2(root, MarketPhaseV2::Retiring)
        .map_err(|_| VerticalError::InvalidPhase)?;
    let beneficiary = Pubkey::new_from_array(close.rent_refund_payer);
    authenticate_rent_credit(program_id, &state.rent_credit, beneficiary)
        .map_err(|_| VerticalError::ContentMismatch)?;
    let frame = [
        AdapterAccountMetaV2 {
            key: state.market.key.to_bytes(),
            is_signer: false,
            is_writable: true,
        },
        AdapterAccountMetaV2 {
            key: state.replay_root.key.to_bytes(),
            is_signer: false,
            is_writable: true,
        },
        AdapterAccountMetaV2 {
            key: state.rent_credit.key.to_bytes(),
            is_signer: false,
            is_writable: true,
        },
        AdapterAccountMetaV2 {
            key: state.system_program.key.to_bytes(),
            is_signer: false,
            is_writable: false,
        },
        AdapterAccountMetaV2 {
            key: state.rent_sysvar.key.to_bytes(),
            is_signer: false,
            is_writable: false,
        },
    ];
    validate_account_frame_v2(AdapterActionV2::CloseReplayRoot, 1, &frame)
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(DirectCloseReplayRootReport {
        instruction: Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(state.market.key, false),
                AccountMeta::new(state.replay_root.key, false),
                AccountMeta::new(state.rent_credit.key, false),
                AccountMeta::new_readonly(state.system_program.key, false),
                AccountMeta::new_readonly(state.rent_sysvar.key, false),
            ],
            data: encode_close_replay_root_instruction_v2().to_vec(),
        },
        observation,
        rent_beneficiary: beneficiary,
    })
}

#[derive(Clone, Copy)]
struct DirectMarketFacts {
    generation: u64,
    phase: Phase,
}

fn direct_market(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<DirectMarketFacts, VerticalError> {
    match decode_market_outcome_count(&account.data).map_err(|_| VerticalError::InvalidState)? {
        2 => direct_market_width::<2>(program_id, account),
        3 => direct_market_width::<3>(program_id, account),
        4 => direct_market_width::<4>(program_id, account),
        5 => direct_market_width::<5>(program_id, account),
        6 => direct_market_width::<6>(program_id, account),
        7 => direct_market_width::<7>(program_id, account),
        8 => direct_market_width::<8>(program_id, account),
        9 => direct_market_width::<9>(program_id, account),
        10 => direct_market_width::<10>(program_id, account),
        11 => direct_market_width::<11>(program_id, account),
        12 => direct_market_width::<12>(program_id, account),
        13 => direct_market_width::<13>(program_id, account),
        14 => direct_market_width::<14>(program_id, account),
        15 => direct_market_width::<15>(program_id, account),
        16 => direct_market_width::<16>(program_id, account),
        _ => Err(VerticalError::InvalidState),
    }
}

fn direct_market_width<const N: usize>(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<DirectMarketFacts, VerticalError> {
    let market: CategoricalMarketV1<N> =
        decode_owned(account, program_id, CategoricalMarketV1::decode)?;
    let identity = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[crate::MARKET_SEED, &identity], &program_id);
    if account.key != expected {
        return Err(VerticalError::PdaMismatch);
    }
    Ok(DirectMarketFacts {
        generation: market.root().identity().generation(),
        phase: market.root().phase(),
    })
}

/// Construct the canonical four-account Dealer reset-ladder frame.
pub fn build_dealer_reset_ladder_v1(
    program_id: Pubkey,
    state: &DealerResetState,
) -> Result<DealerResetReport, VerticalError> {
    let observation = observation(&[&state.market, &state.pool, &state.config])?;
    let config_id = hash(&state.config.data).to_bytes();
    if state.config_finalization.schema_release_id != DEALER_CONFIG_SCHEMA_RELEASE_ID_V1 {
        return Err(VerticalError::FinalizationMismatch);
    }
    // Dealer's config frame contains its finalized cursor but no Rent account.  The SBF adapter authenticates
    // it; the operator can still require its exact raw/cursor derivation and canonical decoding.
    let cursor = &state.config_finalization.staging_cursor;
    if cursor.observation != observation
        || cursor.owner != system_program::ID
        || cursor.executable
        || !cursor.data.is_empty()
    {
        return Err(VerticalError::FinalizationMismatch);
    }
    let (raw, _) = Pubkey::find_program_address(
        &[
            dclutch_record_contract::RAW_RECORD_PDA_SEED_V1,
            &DEALER_CONFIG_SCHEMA_RELEASE_ID_V1,
            &config_id,
        ],
        &program_id,
    );
    let (expected_cursor, _) = Pubkey::find_program_address(
        &[
            dclutch_record_contract::STAGING_CURSOR_PDA_SEED_V1,
            &DEALER_CONFIG_SCHEMA_RELEASE_ID_V1,
            &config_id,
        ],
        &program_id,
    );
    if state.config.key != raw
        || cursor.key != expected_cursor
        || state.config.owner != program_id
        || state.config.executable
    {
        return Err(VerticalError::FinalizationMismatch);
    }
    match decode_market_outcome_count(&state.market.data)
        .map_err(|_| VerticalError::InvalidState)?
    {
        2 => dealer_reset::<2>(program_id, state, observation, config_id),
        3 => dealer_reset::<3>(program_id, state, observation, config_id),
        4 => dealer_reset::<4>(program_id, state, observation, config_id),
        5 => dealer_reset::<5>(program_id, state, observation, config_id),
        6 => dealer_reset::<6>(program_id, state, observation, config_id),
        7 => dealer_reset::<7>(program_id, state, observation, config_id),
        8 => dealer_reset::<8>(program_id, state, observation, config_id),
        9 => dealer_reset::<9>(program_id, state, observation, config_id),
        10 => dealer_reset::<10>(program_id, state, observation, config_id),
        11 => dealer_reset::<11>(program_id, state, observation, config_id),
        12 => dealer_reset::<12>(program_id, state, observation, config_id),
        13 => dealer_reset::<13>(program_id, state, observation, config_id),
        14 => dealer_reset::<14>(program_id, state, observation, config_id),
        15 => dealer_reset::<15>(program_id, state, observation, config_id),
        16 => dealer_reset::<16>(program_id, state, observation, config_id),
        _ => Err(VerticalError::InvalidState),
    }
}

fn dealer_reset<const N: usize>(
    program_id: Pubkey,
    state: &DealerResetState,
    observation: Observation,
    config_id: [u8; 32],
) -> Result<DealerResetReport, VerticalError> {
    let market: CategoricalMarketV1<N> =
        decode_owned(&state.market, program_id, CategoricalMarketV1::decode)?;
    if market.root().phase() != Phase::Open {
        return Err(VerticalError::InvalidPhase);
    }
    let market_identity = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[crate::MARKET_SEED, &market_identity], &program_id);
    if state.market.key != expected_market {
        return Err(VerticalError::PdaMismatch);
    }
    let config_content = dclutch_core_contract::ContentId::new(config_id)
        .map_err(|_| VerticalError::InvalidState)?;
    let seeds = PoolPdaSeedsV1::new(
        state.market.key.to_bytes(),
        market.root().identity().generation(),
        config_content,
    )
    .map_err(|_| VerticalError::PdaMismatch)?;
    let (pool_key, _) = Pubkey::find_program_address(&seeds.seed_components(), &program_id);
    if state.pool.key != pool_key {
        return Err(VerticalError::PdaMismatch);
    }
    if config_width::<N, 1>()? == state.config.data.len() {
        return dealer_reset_bin::<N, 1>(program_id, state, observation, config_id, pool_key);
    }
    if config_width::<N, 2>()? == state.config.data.len() {
        return dealer_reset_bin::<N, 2>(program_id, state, observation, config_id, pool_key);
    }
    if config_width::<N, 3>()? == state.config.data.len() {
        return dealer_reset_bin::<N, 3>(program_id, state, observation, config_id, pool_key);
    }
    if config_width::<N, 4>()? == state.config.data.len() {
        return dealer_reset_bin::<N, 4>(program_id, state, observation, config_id, pool_key);
    }
    if config_width::<N, 5>()? == state.config.data.len() {
        return dealer_reset_bin::<N, 5>(program_id, state, observation, config_id, pool_key);
    }
    if config_width::<N, 6>()? == state.config.data.len() {
        return dealer_reset_bin::<N, 6>(program_id, state, observation, config_id, pool_key);
    }
    if config_width::<N, 7>()? == state.config.data.len() {
        return dealer_reset_bin::<N, 7>(program_id, state, observation, config_id, pool_key);
    }
    if config_width::<N, 8>()? == state.config.data.len() {
        return dealer_reset_bin::<N, 8>(program_id, state, observation, config_id, pool_key);
    }
    Err(VerticalError::AbiUnavailable)
}

fn config_width<const N: usize, const B: usize>() -> Result<usize, VerticalError> {
    LiquidityConfigV1::<N, B>::encoded_len().map_err(|_| VerticalError::AbiUnavailable)
}

fn dealer_reset_bin<const N: usize, const B: usize>(
    program_id: Pubkey,
    state: &DealerResetState,
    observation: Observation,
    config_id: [u8; 32],
    _pool_key: Pubkey,
) -> Result<DealerResetReport, VerticalError> {
    let capability_config_id = dclutch_capability_contract::ContentId::new(config_id)
        .map_err(|_| VerticalError::InvalidState)?;
    let config = LiquidityConfigV1::<N, B>::decode(capability_config_id, &state.config.data)
        .map_err(|_| VerticalError::AbiUnavailable)?;
    let pool = decode_owned(&state.pool, program_id, PoolState::decode)?;
    let mut canonical_config = vec![0; config_width::<N, B>()?];
    config
        .encode_into(&mut canonical_config)
        .map_err(|_| VerticalError::InvalidState)?;
    let mut canonical_pool =
        vec![0; PoolState::<N, B>::encoded_len().map_err(|_| VerticalError::InvalidState)?];
    pool.encode_into(&mut canonical_pool)
        .map_err(|_| VerticalError::InvalidState)?;
    if canonical_config != state.config.data || canonical_pool != state.pool.data {
        return Err(VerticalError::ContentMismatch);
    }
    pool.validate_against(state.pool.key.to_bytes(), &config)
        .map_err(|_| VerticalError::ContentMismatch)?;
    let instruction = DealerInstructionV1::<N>::ResetLadder {
        expected_pool_sequence: pool.next_sequence(),
    };
    let mut data = vec![
        0;
        instruction
            .encoded_len()
            .map_err(|_| VerticalError::InvalidState)?
    ];
    instruction
        .encode_into(&mut data)
        .map_err(|_| VerticalError::InvalidState)?;
    let accounts = [
        state.market.key,
        state.pool.key,
        state.config.key,
        state.config_finalization.staging_cursor.key,
    ];
    let frame = accounts.map(|key| DealerAccountMetaV1 {
        key: key.to_bytes(),
        is_signer: false,
        is_writable: key == state.pool.key,
        is_executable: false,
    });
    DealerFrameV1::<N>::new(DealerActionV1::ResetLadder, &frame)
        .map_err(|_| VerticalError::InvalidState)?;
    Ok(DealerResetReport {
        instruction: Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new_readonly(state.market.key, false),
                AccountMeta::new(state.pool.key, false),
                AccountMeta::new_readonly(state.config.key, false),
                AccountMeta::new_readonly(state.config_finalization.staging_cursor.key, false),
            ],
            data,
        },
        observation,
        config_id,
    })
}

fn finalized<T, E>(
    program_id: Pubkey,
    rent: &solana_program::rent::Rent,
    account: &ObservedAccount,
    proof: &FinalizedRecordProof,
    schema: [u8; 32],
    decode: impl FnOnce(&[u8]) -> Result<T, E>,
) -> Result<T, VerticalError> {
    if proof.schema_release_id != schema {
        return Err(VerticalError::FinalizationMismatch);
    }
    foundation::authenticate_finalized_record(program_id, rent, account, proof)
        .map_err(|_| VerticalError::FinalizationMismatch)?;
    let value = decode(&account.data).map_err(|_| VerticalError::InvalidState)?;
    Ok(value)
}

fn authenticate_finalized_bytes(
    program_id: Pubkey,
    rent: &solana_program::rent::Rent,
    account: &ObservedAccount,
    proof: &FinalizedRecordProof,
    schema: [u8; 32],
) -> Result<(), VerticalError> {
    if proof.schema_release_id != schema {
        return Err(VerticalError::FinalizationMismatch);
    }
    foundation::authenticate_finalized_record(program_id, rent, account, proof)
        .map_err(|_| VerticalError::FinalizationMismatch)
}

fn decode_owned<T, E>(
    account: &ObservedAccount,
    program_id: Pubkey,
    decode: impl FnOnce(&[u8]) -> Result<T, E>,
) -> Result<T, VerticalError> {
    if account.owner != program_id || account.executable {
        return Err(VerticalError::InvalidOwner);
    }
    decode(&account.data).map_err(|_| VerticalError::InvalidState)
}

fn authenticate_rent_credit_at_key(
    program_id: Pubkey,
    account: &ObservedAccount,
    expected_key: [u8; 32],
) -> Result<RentCreditV1, VerticalError> {
    if account.key.to_bytes() != expected_key || account.owner != program_id || account.executable {
        return Err(VerticalError::ContentMismatch);
    }
    let credit = RentCreditV1::decode(&account.data).map_err(|_| VerticalError::InvalidState)?;
    let seeds = credit.pda_seeds();
    let authority = seeds.refund_authority().to_bytes();
    let (derived, bump) =
        Pubkey::find_program_address(&[seeds.domain(), authority.as_slice()], &program_id);
    if account.key != derived
        || credit.pda_bump() != bump
        || credit.to_bytes().as_slice() != account.data.as_slice()
    {
        return Err(VerticalError::ContentMismatch);
    }
    Ok(credit)
}

fn identity(bytes: &[u8]) -> Result<IdentityV1, VerticalError> {
    IdentityV1::new(hash(bytes).to_bytes()).map_err(|_| VerticalError::ContentMismatch)
}

fn observation(accounts: &[&ObservedAccount]) -> Result<Observation, VerticalError> {
    let first = accounts
        .first()
        .ok_or(VerticalError::ObservationMismatch)?
        .observation;
    if first.finality != Finality::Finalized {
        return Err(VerticalError::ObservationNotFinalized);
    }
    if accounts.iter().all(|account| account.observation == first) {
        Ok(first)
    } else {
        Err(VerticalError::ObservationMismatch)
    }
}

fn authenticate_system_actor(account: &ObservedAccount) -> Result<(), VerticalError> {
    if account.owner == system_program::ID && !account.executable && account.data.is_empty() {
        Ok(())
    } else {
        Err(VerticalError::InvalidAuthority)
    }
}

fn authenticate_system_program(account: &ObservedAccount) -> Result<(), VerticalError> {
    if account.key == system_program::ID
        && account.owner == native_loader::ID
        && account.executable
        && account.data.is_empty()
    {
        Ok(())
    } else {
        Err(VerticalError::InvalidOwner)
    }
}

pub(crate) fn decode_clock(account: &ObservedAccount) -> Result<Clock, VerticalError> {
    if account.key != sysvar::clock::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Clock::size_of()
    {
        return Err(VerticalError::InvalidState);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        false,
    );
    Clock::from_account_info(&info).map_err(|_| VerticalError::InvalidState)
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot};
    use dclutch_general_contract::{GENERAL_CAPABILITY_RELEASE_ID_V1, GeneralConfigV1Input};
    use dclutch_market_contract::market::CategoricalSettlementSummaryV1;
    use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority};
    use solana_program::{rent::Rent, sysvar::SysvarSerialize};

    fn account(slot: u64, finality: Finality) -> ObservedAccount {
        ObservedAccount {
            observation: Observation {
                slot,
                unix_timestamp: 1_800_000_000,
                finality,
            },
            key: Pubkey::new_from_array([1; 32]),
            owner: system_program::ID,
            lamports: 0,
            executable: false,
            data: Vec::new(),
        }
    }

    fn observed(
        observation: Observation,
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: Vec<u8>,
    ) -> ObservedAccount {
        ObservedAccount {
            observation,
            key,
            owner,
            lamports,
            executable,
            data,
        }
    }

    fn rent_account(observation: Observation) -> ObservedAccount {
        let rent = Rent::default();
        let mut data = vec![0; Rent::size_of()];
        let mut lamports = 1;
        let mut info = AccountInfo::new(
            &sysvar::rent::ID,
            false,
            false,
            &mut lamports,
            &mut data,
            &sysvar::ID,
            false,
        );
        rent.to_account_info(&mut info).expect("serialize Rent");
        drop(info);
        observed(observation, sysvar::rent::ID, sysvar::ID, 1, false, data)
    }

    fn clock_account(observation: Observation) -> ObservedAccount {
        let clock = Clock {
            slot: observation.slot,
            unix_timestamp: observation.unix_timestamp,
            ..Clock::default()
        };
        let mut data = vec![0; Clock::size_of()];
        let mut lamports = 1;
        let mut info = AccountInfo::new(
            &sysvar::clock::ID,
            false,
            false,
            &mut lamports,
            &mut data,
            &sysvar::ID,
            false,
        );
        clock.to_account_info(&mut info).expect("serialize Clock");
        drop(info);
        observed(observation, sysvar::clock::ID, sysvar::ID, 1, false, data)
    }

    fn finalized_record(
        program_id: Pubkey,
        observation: Observation,
        schema: [u8; 32],
        data: Vec<u8>,
        cursor_dust: u64,
    ) -> (ObservedAccount, FinalizedRecordProof) {
        let digest = hash(&data).to_bytes();
        let (raw, _) = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
            &program_id,
        );
        let (cursor, _) = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                schema.as_slice(),
                digest.as_slice(),
            ],
            &program_id,
        );
        (
            observed(observation, raw, program_id, u64::MAX, false, data),
            FinalizedRecordProof {
                schema_release_id: schema,
                staging_cursor: observed(
                    observation,
                    cursor,
                    system_program::ID,
                    cursor_dust,
                    false,
                    Vec::new(),
                ),
            },
        )
    }

    fn general_open_fixture(batch_dust: u64) -> (Pubkey, GeneralOpenBatchState) {
        let program_id = Pubkey::new_from_array([70; 32]);
        let observation = Observation {
            slot: 41,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        };
        let beneficiary = RefundAuthority::new([71; 32]).expect("beneficiary");
        let (rent_credit_key, rent_credit_bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, beneficiary.to_bytes().as_slice()],
            &program_id,
        );
        let rent_credit = RentCreditV1::new(beneficiary, rent_credit_bump);
        let claim_id = ContentId::new([72; 32]).expect("claim ID");
        let generation = 7;
        let identity = MarketIdentity::new(
            ContentId::new([73; 32]).expect("Realm ID"),
            ContentId::new([74; 32]).expect("Product ID"),
            claim_id,
            ContentId::new([75; 32]).expect("Source ID"),
            ContentId::new([76; 32]).expect("manifest ID"),
            generation,
        );
        let identity_digest = hash(&identity.to_bytes()).to_bytes();
        let (market_key, _) =
            Pubkey::find_program_address(&[crate::MARKET_SEED, &identity_digest], &program_id);
        let mut market_root =
            MarketRoot::founding(identity, rent_credit_key.to_bytes()).expect("Market root");
        market_root
            .transition_phase(generation, Phase::Open)
            .expect("open Market");
        market_root
            .register_child(generation, 0)
            .expect("General child");
        let market = CategoricalMarketV1::<2>::new(
            market_root,
            0,
            [0; 2],
            CategoricalSettlementSummaryV1::empty(),
        )
        .expect("Market");
        let mut market_data =
            vec![0; CategoricalMarketV1::<2>::encoded_len().expect("Market width")];
        market.encode(&mut market_data).expect("encode Market");
        let config = GeneralConfigV1::new(GeneralConfigV1Input {
            capacity_profile_id: dclutch_general_contract::ContentId::new([77; 32])
                .expect("capacity ID"),
            claim_basis_id: dclutch_general_contract::ContentId::new(claim_id.to_bytes())
                .expect("claim ID"),
            capability_release_id: GENERAL_CAPABILITY_RELEASE_ID_V1,
            generation,
            price_scale: 1_000_000,
            collection_slots: 10,
            selection_slots: 10,
            settlement_slots: 10,
            max_orders_per_candidate: 1,
            max_pages_per_candidate: 1,
            continuation_reward_lamports: 1,
            outcome_count: 2,
        })
        .expect("General config");
        let config_bytes = config.to_bytes().to_vec();
        let config_id = dclutch_general_contract::ContentId::new(hash(&config_bytes).to_bytes())
            .expect("config ID");
        let (config_account, config_finalization) = finalized_record(
            program_id,
            observation,
            GENERAL_CONFIG_SCHEMA_ID_V1.to_bytes(),
            config_bytes,
            3,
        );
        let root = GeneralRootV1::founding(
            market_key.to_bytes(),
            config_id,
            generation,
            rent_credit_key.to_bytes(),
        )
        .expect("General root");
        let root_seeds = GeneralRootPdaSeedsV1::new(market_key.to_bytes(), generation, config_id)
            .expect("root seeds");
        let (root_key, _) =
            Pubkey::find_program_address(&root_seeds.seed_components(), &program_id);
        let mut root_data = vec![0; GENERAL_ROOT_BYTES];
        root.encode(&mut root_data).expect("encode General root");
        let batch_seeds = GeneralBatchPdaSeedsV1::new(root_key.to_bytes(), 0).expect("batch seeds");
        let (batch_key, _) =
            Pubkey::find_program_address(&batch_seeds.seed_components(), &program_id);
        (
            program_id,
            GeneralOpenBatchState {
                actor: observed(
                    observation,
                    Pubkey::new_from_array([78; 32]),
                    system_program::ID,
                    u64::MAX,
                    false,
                    Vec::new(),
                ),
                market: observed(
                    observation,
                    market_key,
                    program_id,
                    u64::MAX,
                    false,
                    market_data,
                ),
                config: config_account,
                root: observed(
                    observation,
                    root_key,
                    program_id,
                    u64::MAX,
                    false,
                    root_data,
                ),
                batch: observed(
                    observation,
                    batch_key,
                    system_program::ID,
                    batch_dust,
                    false,
                    Vec::new(),
                ),
                rent_credit: observed(
                    observation,
                    rent_credit_key,
                    program_id,
                    u64::MAX,
                    false,
                    rent_credit.to_bytes().to_vec(),
                ),
                config_finalization,
                system_program: observed(
                    observation,
                    system_program::ID,
                    native_loader::ID,
                    1,
                    true,
                    Vec::new(),
                ),
                rent_sysvar: rent_account(observation),
                clock_sysvar: clock_account(observation),
            },
        )
    }

    #[test]
    fn vertical_builders_refuse_nonfinal_or_mixed_snapshots() {
        let finalized = account(9, Finality::Finalized);
        let confirmed = account(9, Finality::Confirmed);
        let later = account(10, Finality::Finalized);
        assert_eq!(
            observation(&[&confirmed]),
            Err(VerticalError::ObservationNotFinalized)
        );
        assert_eq!(
            observation(&[&finalized, &later]),
            Err(VerticalError::ObservationMismatch)
        );
    }

    #[test]
    fn general_open_uses_successor_frame_and_accepts_safe_dust() {
        let (program_id, state) = general_open_fixture(5);
        let report = build_general_open_batch_v1(program_id, &state)
            .expect("successor General batch opens from caller capitalization");
        assert_eq!(report.sequence, 0);
        assert_eq!(report.batch, state.batch.key);
        assert_eq!(report.instruction.accounts.len(), 10);
        assert_eq!(
            report.instruction.accounts,
            vec![
                AccountMeta::new(state.actor.key, true),
                AccountMeta::new_readonly(state.market.key, false),
                AccountMeta::new_readonly(state.config.key, false),
                AccountMeta::new_readonly(state.config_finalization.staging_cursor.key, false),
                AccountMeta::new(state.root.key, false),
                AccountMeta::new(state.batch.key, false),
                AccountMeta::new(state.rent_credit.key, false),
                AccountMeta::new_readonly(state.system_program.key, false),
                AccountMeta::new_readonly(state.rent_sysvar.key, false),
                AccountMeta::new_readonly(state.clock_sysvar.key, false),
            ]
        );
        assert_eq!(state.config_finalization.staging_cursor.lamports, 3);
        assert_eq!(state.batch.lamports, 5);
    }

    #[test]
    fn general_open_refuses_credit_and_finalization_substitution() {
        let (program_id, state) = general_open_fixture(0);

        let mut wrong_credit = state.clone();
        wrong_credit.rent_credit.key = Pubkey::new_from_array([79; 32]);
        assert_eq!(
            build_general_open_batch_v1(program_id, &wrong_credit),
            Err(VerticalError::ContentMismatch)
        );

        let mut live_cursor = state;
        live_cursor.config_finalization.staging_cursor.data.push(1);
        assert_eq!(
            build_general_open_batch_v1(program_id, &live_cursor),
            Err(VerticalError::FinalizationMismatch)
        );
    }

    #[test]
    fn series_successor_occurrence_authorities_share_one_snapshot() {
        let observation = Observation {
            slot: 51,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        };
        let raw = |byte| {
            observed(
                observation,
                Pubkey::new_from_array([byte; 32]),
                Pubkey::new_from_array([90; 32]),
                1,
                false,
                vec![1],
            )
        };
        let proof = |byte| FinalizedRecordProof {
            schema_release_id: [byte; 32],
            staging_cursor: observed(
                observation,
                Pubkey::new_from_array([byte.wrapping_add(30); 32]),
                system_program::ID,
                1,
                false,
                Vec::new(),
            ),
        };
        let state = SeriesInstantiateState {
            actor: raw(1),
            root: raw(2),
            escrow: raw(3),
            ticket: raw(4),
            recipe: raw(5),
            recipe_finalization: proof(5),
            aggregate: raw(6),
            aggregate_finalization: proof(6),
            capacity_profile: raw(7),
            capacity_profile_finalization: proof(7),
            derived: raw(8),
            derived_finalization: proof(8),
            capitalization: raw(9),
            capitalization_finalization: proof(9),
            resolution_material: raw(10),
            resolution_material_finalization: proof(10),
            capability_template: raw(11),
            capability_template_finalization: proof(11),
            capability_manifest: raw(12),
            capability_manifest_finalization: proof(12),
            system_program: raw(13),
            rent_sysvar: raw(14),
        };
        let metas = series_instantiate_accounts(&state);
        assert_eq!(metas.len(), 22);
        let actor_meta = metas.first().expect("actor meta");
        assert!(actor_meta.is_signer);
        assert!(!actor_meta.is_writable);
        assert_eq!(
            metas.get(14).map(|meta| meta.pubkey),
            Some(state.resolution_material.key)
        );
        assert_eq!(
            metas.get(16).map(|meta| meta.pubkey),
            Some(state.capability_template.key)
        );
        assert_eq!(
            metas.get(18).map(|meta| meta.pubkey),
            Some(state.capability_manifest.key)
        );
        assert!(metas.iter().skip(14).take(6).all(|meta| !meta.is_writable));
        let mut material_substitution = state.clone();
        material_substitution.resolution_material.observation.slot += 1;
        assert_eq!(
            build_series_instantiate_next_v1(
                Pubkey::new_from_array([90; 32]),
                &material_substitution,
            ),
            Err(VerticalError::ObservationMismatch)
        );
        let mut template_substitution = state.clone();
        template_substitution.capability_template.observation.slot += 1;
        assert_eq!(
            build_series_instantiate_next_v1(
                Pubkey::new_from_array([90; 32]),
                &template_substitution,
            ),
            Err(VerticalError::ObservationMismatch)
        );
        let mut manifest_substitution = state;
        manifest_substitution.capability_manifest.observation.slot += 1;
        assert_eq!(
            build_series_instantiate_next_v1(
                Pubkey::new_from_array([90; 32]),
                &manifest_substitution,
            ),
            Err(VerticalError::ObservationMismatch)
        );
    }

    #[test]
    fn direct_close_refuses_a_root_with_open_registration() {
        let root =
            MakerReplayRootV2::new([1; 32], 0, [2; 32], [3; 32], 7).expect("canonical replay root");
        assert!(prepare_replay_root_close_v2(root, MarketPhaseV2::Retiring).is_err());
    }

    fn direct_registration_close_fixture(
        phase: Phase,
    ) -> (Pubkey, DirectCloseReplayRegistrationState) {
        let program_id = Pubkey::new_from_array([90; 32]);
        let observation = Observation {
            slot: 41,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        };
        let content = |byte| ContentId::new([byte; 32]).expect("nonzero fixture content ID");
        let identity = MarketIdentity::new(
            content(1),
            content(2),
            content(3),
            content(4),
            content(5),
            7,
        );
        let identity_digest = hash(&identity.to_bytes()).to_bytes();
        let market_key =
            Pubkey::find_program_address(&[crate::MARKET_SEED, &identity_digest], &program_id).0;
        let mut market_root = MarketRoot::founding(identity, [6; 32]).expect("founding Market");
        if phase != Phase::Founding {
            market_root
                .transition_phase(7, phase)
                .expect("fixture phase is a direct founding transition");
        }
        let market = CategoricalMarketV1::<2>::new(
            market_root,
            0,
            [0, 0],
            CategoricalSettlementSummaryV1::empty(),
        )
        .expect("canonical fixture Market");
        let mut market_data =
            vec![0; CategoricalMarketV1::<2>::encoded_len().expect("supported width")];
        market
            .encode(&mut market_data)
            .expect("exact Market buffer");

        let maker = [7; 32];
        let generation = 7_u64;
        let (replay_key, bump) = Pubkey::find_program_address(
            &[
                MAKER_REPLAY_ROOT_PDA_DOMAIN_V2,
                market_key.as_ref(),
                &generation.to_le_bytes(),
                &maker,
            ],
            &program_id,
        );
        let replay =
            MakerReplayRootV2::new(market_key.to_bytes(), generation, maker, [8; 32], bump)
                .expect("canonical fixture replay root");
        let mut replay_data = vec![0; dclutch_direct_contract::MAKER_REPLAY_ROOT_BYTES_V2];
        replay
            .encode(&mut replay_data)
            .expect("exact replay buffer");
        (
            program_id,
            DirectCloseReplayRegistrationState {
                market: ObservedAccount {
                    observation,
                    key: market_key,
                    owner: program_id,
                    lamports: 100,
                    executable: false,
                    data: market_data,
                },
                replay_root: ObservedAccount {
                    observation,
                    key: replay_key,
                    owner: program_id,
                    lamports: 100,
                    executable: false,
                    data: replay_data,
                },
            },
        )
    }

    #[test]
    fn direct_registration_close_is_exact_and_chain_derived() {
        let (program_id, state) = direct_registration_close_fixture(Phase::Retiring);
        let report = build_direct_close_replay_registration_v2(program_id, &state)
            .expect("retiring Market admits exact registration close");
        assert_eq!(report.observation, state.market.observation);
        assert_eq!(report.instruction.program_id, program_id);
        assert_eq!(
            report.instruction.accounts,
            vec![
                AccountMeta::new(state.market.key, false),
                AccountMeta::new(state.replay_root.key, false),
            ]
        );
        assert_eq!(
            report.instruction.data,
            encode_close_replay_registration_instruction_v2()
        );
    }

    #[test]
    fn direct_registration_close_refuses_hostile_observations_and_bindings() {
        let (program_id, state) = direct_registration_close_fixture(Phase::Retiring);

        let mut nonfinal = state.clone();
        nonfinal.market.observation.finality = Finality::Confirmed;
        nonfinal.replay_root.observation.finality = Finality::Confirmed;
        assert_eq!(
            build_direct_close_replay_registration_v2(program_id, &nonfinal),
            Err(VerticalError::ObservationNotFinalized)
        );

        let mut mixed = state.clone();
        mixed.replay_root.observation.slot += 1;
        assert_eq!(
            build_direct_close_replay_registration_v2(program_id, &mixed),
            Err(VerticalError::ObservationMismatch)
        );

        let mut wrong_owner = state.clone();
        wrong_owner.replay_root.owner = system_program::ID;
        assert_eq!(
            build_direct_close_replay_registration_v2(program_id, &wrong_owner),
            Err(VerticalError::InvalidOwner)
        );

        let mut malformed = state.clone();
        *malformed
            .replay_root
            .data
            .get_mut(12)
            .expect("fixture replay body") = 1;
        assert_eq!(
            build_direct_close_replay_registration_v2(program_id, &malformed),
            Err(VerticalError::InvalidState)
        );

        let mut wrong_pda = state.clone();
        wrong_pda.replay_root.key = Pubkey::new_from_array([55; 32]);
        assert_eq!(
            build_direct_close_replay_registration_v2(program_id, &wrong_pda),
            Err(VerticalError::PdaMismatch)
        );
    }

    #[test]
    fn direct_registration_close_refuses_nonretiring_or_already_closed_state() {
        let (program_id, founding) = direct_registration_close_fixture(Phase::Founding);
        assert_eq!(
            build_direct_close_replay_registration_v2(program_id, &founding),
            Err(VerticalError::InvalidPhase)
        );

        let (program_id, mut closed) = direct_registration_close_fixture(Phase::Retiring);
        let root = MakerReplayRootV2::decode(&closed.replay_root.data)
            .expect("fixture replay root decodes");
        let root = close_replay_registration_v2(root, MarketPhaseV2::Retiring)
            .expect("fixture registration closes");
        root.encode(&mut closed.replay_root.data)
            .expect("exact replay buffer");
        assert_eq!(
            build_direct_close_replay_registration_v2(program_id, &closed),
            Err(VerticalError::InvalidPhase)
        );
    }

    #[test]
    fn source_retirement_refuses_nonterminal_state_and_manual_clock() {
        let material = dclutch_source_contract::ContentId::new([9; 32])
            .expect("nonzero source material identity");
        let mut primary = SourceResolutionStateV1::fresh([1; 32], 0, material, [2; 32], 7, 0, 0)
            .expect("fresh source state")
            .state();
        assert!(primary.retire(0, 1, 1, 1).is_err());
        assert_eq!(
            decode_clock(&account(9, Finality::Finalized)),
            Err(VerticalError::InvalidState)
        );
    }
}
