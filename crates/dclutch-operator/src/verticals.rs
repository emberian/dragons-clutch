//! Finalized-state instruction construction for executable verticals.
//!
//! The functions here deliberately accept complete observed accounts rather
//! than a convenient client DTO.  They re-decode every persisted fact that
//! selects an instruction field, recompute all relevant PDAs, and emit the
//! exact SBF account order and privileges.  They neither sign nor submit.

use dclutch_core_contract::Phase;
use dclutch_dealer_contract::{
    LiquidityConfigV1, PoolState,
    frame::{
        DEALER_CONFIG_SCHEMA_RELEASE_ID_V1, DealerAccountMetaV1, DealerFrameV1, PoolPdaSeedsV1,
    },
    instruction::{DealerActionV1, DealerInstructionV1},
};
use dclutch_direct_contract::{
    MAKER_REPLAY_ROOT_PDA_DOMAIN_V2, MakerReplayRootV2,
    adapter::{
        AdapterAccountMetaV2, AdapterActionV2, MarketPhaseV2,
        encode_close_replay_root_instruction_v2, validate_account_frame_v2,
    },
    prepare_replay_root_close_v2,
};
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_product_contract::capacity::CapacityProfileV1;
use dclutch_series_contract::{
    CapitalizationAggregateV1, DerivedOccurrenceV1, IdentityV1, InstantiateNextV1,
    OccurrenceCapitalizationV1, SERIES_ESCROW_PDA_DOMAIN_V1, SERIES_ROOT_PDA_DOMAIN_V1,
    SERIES_TICKET_PDA_DOMAIN_V1, SeriesEscrowV1, SeriesRecipeV1, SeriesRootV1,
    VacantAccountFactsV1, plan_instantiate_next_v1,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{native_loader, system_program};

use crate::{
    Finality, Observation, ObservedAccount, authenticate_rent_credit,
    foundation::{self, FinalizedRecordProof},
};

/// The Series capacity-profile schema selected by the current SBF V1 ABI.
pub const SERIES_CAPACITY_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0xed, 0x25, 0x2a, 0x2a, 0xc5, 0x55, 0xf0, 0xe3, 0x4f, 0xfc, 0x23, 0xac, 0x91, 0xd8, 0x6c, 0x61,
    0xbe, 0x6d, 0xd9, 0x81, 0x24, 0x47, 0x57, 0x49, 0x94, 0x69, 0xbb, 0x99, 0xba, 0x55, 0x36, 0x50,
];
/// The Series recipe schema selected by the current SBF V1 ABI.
pub const SERIES_RECIPE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x25, 0xd2, 0x2f, 0x56, 0x52, 0x55, 0x02, 0x03, 0x77, 0x15, 0xb0, 0x74, 0xfe, 0xd8, 0xcf, 0x37,
    0x31, 0x8c, 0xdc, 0x40, 0x75, 0xfa, 0xb0, 0x86, 0x8a, 0x1e, 0x2f, 0x11, 0x85, 0x91, 0x97, 0xf6,
];
/// The Series aggregate schema selected by the current SBF V1 ABI.
pub const SERIES_AGGREGATE_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x36, 0xdc, 0xc9, 0xd8, 0x3c, 0x7a, 0x89, 0xeb, 0xb2, 0x4f, 0xd2, 0x44, 0x79, 0x23, 0xca, 0x68,
    0x4c, 0x3c, 0x2c, 0x28, 0x20, 0x54, 0xc7, 0x58, 0x9c, 0x4a, 0xb3, 0x9d, 0xad, 0xec, 0x9a, 0xd5,
];
/// The Series derived-occurrence schema selected by the current SBF V1 ABI.
pub const SERIES_DERIVED_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x44, 0x14, 0xd1, 0xe5, 0x40, 0x3a, 0x59, 0x42, 0xfb, 0xf2, 0x88, 0x8f, 0x4a, 0x54, 0x84, 0x75,
    0x85, 0x62, 0xcc, 0xc7, 0xd4, 0xb0, 0x53, 0xd8, 0x96, 0x42, 0xc7, 0xee, 0x02, 0xd3, 0x5c, 0xc9,
];
/// The Series capitalization schema selected by the current SBF V1 ABI.
pub const SERIES_CAPITALIZATION_SCHEMA_RELEASE_ID_V1: [u8; 32] = [
    0x55, 0x8b, 0xc4, 0xd4, 0x83, 0xae, 0xc0, 0x5c, 0x81, 0x85, 0x33, 0x65, 0x6c, 0x58, 0xf7, 0x7c,
    0x7f, 0x16, 0xe0, 0xb3, 0x42, 0x8f, 0x05, 0xe5, 0xa5, 0xfc, 0x97, 0x69, 0x3a, 0x1d, 0x9f, 0xdf,
];

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

/// Construct the canonical 16-account V1 instantiate-next frame.
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
        SERIES_RECIPE_SCHEMA_RELEASE_ID_V1,
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
            accounts: vec![
                AccountMeta::new(state.actor.key, true),
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
                AccountMeta::new_readonly(
                    state.capitalization_finalization.staging_cursor.key,
                    false,
                ),
                AccountMeta::new_readonly(state.system_program.key, false),
                AccountMeta::new_readonly(state.rent_sysvar.key, false),
            ],
            data,
        },
        observation,
        ticket,
        occurrence_index: index,
    })
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
        || cursor.lamports != 0
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn direct_close_refuses_a_root_with_open_registration() {
        let root =
            MakerReplayRootV2::new([1; 32], 0, [2; 32], [3; 32], 7).expect("canonical replay root");
        assert!(prepare_replay_root_close_v2(root, MarketPhaseV2::Retiring).is_err());
    }
}
