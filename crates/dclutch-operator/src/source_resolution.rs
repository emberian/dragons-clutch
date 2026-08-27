//! Chain-derived Source-resolution creation and primary-provider acceptance.
//!
//! This is the live half of the retired `verticals` module. It builds the two
//! unsigned Source instructions the current generation actually submits:
//! `CreateResolution`, which opens the canonical per-generation resolution
//! state, and the primary inline `AcceptEvidence`, which carries a real Pyth
//! Receiver post in the same transaction. Both are constructed only from one
//! finalized observation; neither performs RPC, signs, or submits.

use dclutch_core_contract::Phase;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_pyth_svm::{
    FULL_PRICE_UPDATE_V2_LEN, PostUpdateParamsView, ProgramDataV3View, ProgramV3View,
    ReceiverConfigV2View,
};
use dclutch_source_contract::{
    ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES, AcceptEvidenceInstructionV1,
    CreateResolutionInstructionV1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    SOURCE_RESOLUTION_STATE_BYTES, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1, SourceAccessProfile,
    SourceAccountPrivilegeV1, SourceFrameKindV1, SourceMaterialViewV1, SourceResolutionPhaseV1,
    SourceResolutionStateV1, validate_source_frame_v1,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program};

use crate::{
    Finality, MARKET_SEED, Observation, ObservedAccount, authenticate_rent_credit,
    foundation::{self, FinalizedRecordProof, decode_clock},
    select_release,
};

const RECEIVER_CONFIG_SEED: &[u8] = b"config";
const RECEIVER_TREASURY_SEED: &[u8] = b"treasury";

/// Refusal from finalized Source observation or exact instruction construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceResolutionError {
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

/// Finalized observations required to create the current Market generation's
/// canonical Source-resolution state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCreateResolutionState {
    /// System-owned signing payer of any state-rent shortfall.
    pub payer: ObservedAccount,
    /// Vacant, possibly dust-funded canonical state PDA.
    pub resolution_state_destination: ObservedAccount,
    /// Mutable open Market which owns generation and child-count replay.
    pub market: ObservedAccount,
    /// Finalized SourceMaterial selected by the Market identity.
    pub resolution_material: ObservedAccount,
    /// Finalized-record proof for `resolution_material`.
    pub resolution_material_finalization: FinalizedRecordProof,
    /// Pre-existing permanent credit bound to the Market refund beneficiary.
    pub rent_credit: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
}

/// Exact chain-derived Source-state creation report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCreateResolutionReport {
    /// Unsigned exact eight-account Source instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting all fields.
    pub observation: Observation,
    /// Canonical Source-resolution state PDA.
    pub resolution_state: Pubkey,
    /// Current Market child-count replay guard.
    pub expected_market_child_count: u64,
    /// Exact payer top-up after harmless destination dust.
    pub state_rent_top_up: u64,
}

/// Finalized real-provider accounts for primary inline Source acceptance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAcceptPrimaryInlineState {
    /// Mutable canonical Source-resolution state.
    pub resolution_state: ObservedAccount,
    /// Mutable open Market resolved atomically with the Source state.
    pub market: ObservedAccount,
    /// Finalized SourceMaterial selected by both state and Market.
    pub resolution_material: ObservedAccount,
    /// Finalized-record proof for `resolution_material`.
    pub resolution_material_finalization: FinalizedRecordProof,
    /// Canonical Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Canonical Clock sysvar selecting time and terminal replay sequence.
    pub clock_sysvar: ObservedAccount,
    /// System-owned signing resolver funding Receiver rent and fee.
    pub resolver: ObservedAccount,
    /// Vacant System-owned signing Pyth update destination.
    pub update: ObservedAccount,
    /// Executable release-selected Pyth Receiver program.
    pub receiver_program: ObservedAccount,
    /// Loader-owned Receiver ProgramData.
    pub receiver_programdata: ObservedAccount,
    /// Release-selected Receiver configuration.
    pub receiver_config: ObservedAccount,
    /// Router-owned encoded VAA consumed by Receiver.
    pub encoded_vaa: ObservedAccount,
    /// Executable release-selected router program.
    pub router_program: ObservedAccount,
    /// Loader-owned router ProgramData.
    pub router_programdata: ObservedAccount,
    /// Canonical Receiver treasury selected by the post body.
    pub receiver_treasury: ObservedAccount,
    /// Canonical executable System Program.
    pub system_program: ObservedAccount,
}

/// Exact primary-inline evidence acceptance report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceAcceptPrimaryInlineReport {
    /// Unsigned exact sixteen-account Source instruction.
    pub instruction: Instruction,
    /// Shared finalized observation selecting every account and replay field.
    pub observation: Observation,
    /// Positive terminal sequence derived from the authenticated Clock slot.
    pub terminal_sequence: u64,
    /// Exact resolver balance required for update rent plus provider fee.
    pub required_resolver_lamports: u64,
}

/// Construct the fresh eight-account `CreateResolution` action.
///
/// Generation, SourceMaterial identity, beneficiary, and child replay guard are
/// copied only from authenticated Market/material state. The destination may
/// contain harmless System-owned dust; the report names only the exact top-up.
pub fn build_source_create_resolution_v1(
    program_id: Pubkey,
    state: &SourceCreateResolutionState,
) -> Result<SourceCreateResolutionReport, SourceResolutionError> {
    let observation = source_create_observation(state)?;
    authenticate_system_actor(&state.payer)?;
    authenticate_system_program(&state.system_program)?;
    let rent = foundation::decode_rent(&state.rent_sysvar)
        .map_err(|_| SourceResolutionError::InvalidState)?;
    let market = source_market(program_id, &state.market)?;
    if market.phase != dclutch_core_contract::Phase::Open {
        return Err(SourceResolutionError::InvalidPhase);
    }
    let material = authenticate_material(program_id, state, &rent)?;
    if hash(material.as_bytes()).to_bytes() != market.resolution_policy_id
        || material
            .result_domain()
            .map_err(|_| SourceResolutionError::InvalidState)?
            .outcome_count()
            != market.outcome_count
    {
        return Err(SourceResolutionError::ContentMismatch);
    }
    authenticate_rent_credit(
        program_id,
        &state.rent_credit,
        Pubkey::new_from_array(market.rent_refund),
    )
    .map_err(|_| SourceResolutionError::ContentMismatch)?;
    if !rent.is_exempt(state.rent_credit.lamports, state.rent_credit.data.len()) {
        return Err(SourceResolutionError::InvalidState);
    }
    if state.resolution_state_destination.owner != system_program::ID
        || state.resolution_state_destination.executable
        || !state.resolution_state_destination.data.is_empty()
    {
        return Err(SourceResolutionError::InvalidState);
    }
    let generation_bytes = market.generation.to_le_bytes();
    let (resolution_state, bump) = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
            state.market.key.as_ref(),
            generation_bytes.as_slice(),
        ],
        &program_id,
    );
    if state.resolution_state_destination.key != resolution_state {
        return Err(SourceResolutionError::PdaMismatch);
    }
    let material_id = dclutch_source_contract::ContentId::new(market.resolution_policy_id)
        .map_err(|_| SourceResolutionError::ContentMismatch)?;
    let wire = CreateResolutionInstructionV1::new(
        state.market.key.to_bytes(),
        market.generation,
        material_id,
        market.rent_refund,
        market.child_count,
        bump,
        None,
    )
    .map_err(|_| SourceResolutionError::InvalidState)?;
    let minimum = rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES);
    let state_rent_top_up = minimum.saturating_sub(state.resolution_state_destination.lamports);
    if state.payer.lamports < state_rent_top_up {
        return Err(SourceResolutionError::InvalidAuthority);
    }
    let accounts = vec![
        AccountMeta::new(state.payer.key, true),
        AccountMeta::new(resolution_state, false),
        AccountMeta::new(state.market.key, false),
        AccountMeta::new_readonly(state.resolution_material.key, false),
        AccountMeta::new_readonly(
            state.resolution_material_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
        AccountMeta::new_readonly(state.rent_credit.key, false),
        AccountMeta::new_readonly(state.system_program.key, false),
    ];
    validate_source_metas(SourceFrameKindV1::CreateResolutionFresh, &accounts, state)?;
    Ok(SourceCreateResolutionReport {
        instruction: Instruction {
            program_id,
            accounts,
            data: wire.to_bytes().to_vec(),
        },
        observation,
        resolution_state,
        expected_market_child_count: market.child_count,
        state_rent_top_up,
    })
}

/// Construct primary inline `AcceptEvidence` from a finalized Source snapshot
/// and exact real-provider account observations.
///
/// `post_update_body` is transport plumbing, not result authority. Its shape,
/// treasury, provider release, loader links, config digest, funding, and owner
/// bindings are all re-authenticated before the unsigned instruction is built.
pub fn build_source_accept_primary_inline_v1(
    program_id: Pubkey,
    state: &SourceAcceptPrimaryInlineState,
    post_update_body: &[u8],
) -> Result<SourceAcceptPrimaryInlineReport, SourceResolutionError> {
    let observation = source_accept_observation(state)?;
    authenticate_system_program(&state.system_program)?;
    let rent = foundation::decode_rent(&state.rent_sysvar)
        .map_err(|_| SourceResolutionError::InvalidState)?;
    let clock =
        decode_clock(&state.clock_sysvar).map_err(|_| SourceResolutionError::InvalidState)?;
    if clock.slot == 0 {
        return Err(SourceResolutionError::InvalidState);
    }
    let source = decode_owned(
        &state.resolution_state,
        program_id,
        SourceResolutionStateV1::decode,
    )?;
    if source.to_bytes().as_slice() != state.resolution_state.data.as_slice()
        || source.phase() != SourceResolutionPhaseV1::Primary
    {
        return Err(SourceResolutionError::InvalidPhase);
    }
    let seeds = source.pda_seeds();
    let (expected_state, bump) = Pubkey::find_program_address(
        &[seeds.domain(), &seeds.market(), &seeds.generation_le()],
        &program_id,
    );
    if state.resolution_state.key != expected_state || seeds.bump() != bump {
        return Err(SourceResolutionError::PdaMismatch);
    }
    let market = source_market(program_id, &state.market)?;
    if market.phase != dclutch_core_contract::Phase::Open
        || source.market() != state.market.key.to_bytes()
        || source.generation() != market.generation
        || source.material_id().to_bytes() != market.resolution_policy_id
    {
        return Err(SourceResolutionError::ContentMismatch);
    }
    let material = authenticate_accept_material(program_id, state, &rent)?;
    if hash(material.as_bytes()).to_bytes() != market.resolution_policy_id
        || material
            .result_domain()
            .map_err(|_| SourceResolutionError::InvalidState)?
            .outcome_count()
            != market.outcome_count
    {
        return Err(SourceResolutionError::ContentMismatch);
    }
    let (source_id, source_spec) = material
        .primary_source()
        .map_err(|_| SourceResolutionError::InvalidState)?;
    if source_spec.access_profile() != SourceAccessProfile::PythTerminalOneTransaction {
        return Err(SourceResolutionError::AbiUnavailable);
    }
    let (_, provider) = material
        .primary_provider_release()
        .map_err(|_| SourceResolutionError::InvalidState)?;
    let release = select_release(
        provider.provider_deployment_release_id().to_bytes(),
        provider.decoding_rules_id().to_bytes(),
        provider.transport_profile_id().to_bytes(),
        clock.unix_timestamp,
    )
    .map_err(|_| SourceResolutionError::ContentMismatch)?;
    let obligation = dclutch_source_contract::PythProviderAdapterObligationV1::from_material_view(
        material, source_id,
    )
    .map_err(|_| SourceResolutionError::ContentMismatch)?;
    if obligation.provider_release() != provider {
        return Err(SourceResolutionError::ContentMismatch);
    }
    let post = PostUpdateParamsView::parse(post_update_body)
        .map_err(|_| SourceResolutionError::InvalidState)?;
    authenticate_provider_accounts(state, &release, &post)?;
    let config = ReceiverConfigV2View::parse(&state.receiver_config.data)
        .map_err(|_| SourceResolutionError::InvalidState)?;
    let required_resolver_lamports = rent
        .minimum_balance(FULL_PRICE_UPDATE_V2_LEN)
        .checked_add(config.fee())
        .ok_or(SourceResolutionError::InvalidState)?;
    if state.resolver.lamports < required_resolver_lamports {
        return Err(SourceResolutionError::InvalidAuthority);
    }
    let terminal_sequence = clock.slot;
    let prefix = AcceptEvidenceInstructionV1::new(source.generation(), terminal_sequence)
        .map_err(|_| SourceResolutionError::InvalidState)?
        .to_prefix_bytes();
    let mut data = Vec::with_capacity(
        ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES
            .checked_add(post_update_body.len())
            .ok_or(SourceResolutionError::InvalidState)?,
    );
    data.extend_from_slice(&prefix);
    data.extend_from_slice(post_update_body);
    let accounts = accept_primary_accounts(state);
    validate_accept_metas(&accounts, state)?;
    Ok(SourceAcceptPrimaryInlineReport {
        instruction: Instruction {
            program_id,
            accounts,
            data,
        },
        observation,
        terminal_sequence,
        required_resolver_lamports,
    })
}

fn source_create_observation(
    state: &SourceCreateResolutionState,
) -> Result<Observation, SourceResolutionError> {
    observation(&[
        &state.payer,
        &state.resolution_state_destination,
        &state.market,
        &state.resolution_material,
        &state.resolution_material_finalization.staging_cursor,
        &state.rent_credit,
        &state.system_program,
        &state.rent_sysvar,
    ])
}

fn source_accept_observation(
    state: &SourceAcceptPrimaryInlineState,
) -> Result<Observation, SourceResolutionError> {
    observation(&[
        &state.resolution_state,
        &state.market,
        &state.resolution_material,
        &state.resolution_material_finalization.staging_cursor,
        &state.rent_sysvar,
        &state.clock_sysvar,
        &state.resolver,
        &state.update,
        &state.receiver_program,
        &state.receiver_programdata,
        &state.receiver_config,
        &state.encoded_vaa,
        &state.router_program,
        &state.router_programdata,
        &state.receiver_treasury,
        &state.system_program,
    ])
}

fn authenticate_material<'a>(
    program_id: Pubkey,
    state: &'a SourceCreateResolutionState,
    rent: &solana_program::rent::Rent,
) -> Result<SourceMaterialViewV1<'a>, SourceResolutionError> {
    authenticate_finalized_bytes(
        program_id,
        rent,
        &state.resolution_material,
        &state.resolution_material_finalization,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    )?;
    SourceMaterialViewV1::decode(&state.resolution_material.data)
        .map_err(|_| SourceResolutionError::InvalidState)
}

fn authenticate_accept_material<'a>(
    program_id: Pubkey,
    state: &'a SourceAcceptPrimaryInlineState,
    rent: &solana_program::rent::Rent,
) -> Result<SourceMaterialViewV1<'a>, SourceResolutionError> {
    authenticate_finalized_bytes(
        program_id,
        rent,
        &state.resolution_material,
        &state.resolution_material_finalization,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    )?;
    SourceMaterialViewV1::decode(&state.resolution_material.data)
        .map_err(|_| SourceResolutionError::InvalidState)
}

fn authenticate_provider_accounts(
    state: &SourceAcceptPrimaryInlineState,
    release: &dclutch_pyth_svm::PythReleaseV1,
    post: &PostUpdateParamsView<'_>,
) -> Result<(), SourceResolutionError> {
    let receiver = Pubkey::new_from_array(release.receiver_program());
    let router = Pubkey::new_from_array(release.router_program());
    let (config, _) = Pubkey::find_program_address(&[RECEIVER_CONFIG_SEED], &receiver);
    let (treasury, _) =
        Pubkey::find_program_address(&[RECEIVER_TREASURY_SEED, &[post.treasury_id()]], &receiver);
    if state.receiver_program.key != receiver
        || state.router_program.key != router
        || state.receiver_programdata.key.to_bytes() != release.receiver_programdata()
        || state.router_programdata.key.to_bytes() != release.router_programdata()
        || state.receiver_config.key != config
        || release.receiver_config() != config.to_bytes()
        || state.receiver_treasury.key != treasury
        || state.receiver_program.owner != bpf_loader_upgradeable::ID
        || state.receiver_programdata.owner != bpf_loader_upgradeable::ID
        || state.router_program.owner != bpf_loader_upgradeable::ID
        || state.router_programdata.owner != bpf_loader_upgradeable::ID
        || state.receiver_config.owner != receiver
        || state.encoded_vaa.owner != router
        || !state.receiver_program.executable
        || !state.router_program.executable
        || state.receiver_programdata.executable
        || state.router_programdata.executable
        || state.receiver_config.executable
        || state.encoded_vaa.executable
        || state.receiver_treasury.executable
    {
        return Err(SourceResolutionError::InvalidOwner);
    }
    authenticate_loader(
        &state.receiver_program,
        &state.receiver_programdata,
        release.receiver_programdata(),
        release.receiver_deployment_slot(),
    )?;
    authenticate_loader(
        &state.router_program,
        &state.router_programdata,
        release.router_programdata(),
        release.router_deployment_slot(),
    )?;
    let receiver_config = ReceiverConfigV2View::parse(&state.receiver_config.data)
        .map_err(|_| SourceResolutionError::InvalidState)?;
    if hash(&state.receiver_config.data).to_bytes() != release.config_digest()
        || receiver_config.router_program() != release.router_program()
        || state.update.owner != system_program::ID
        || state.update.executable
        || state.update.lamports != 0
        || !state.update.data.is_empty()
        || state.resolver.owner != system_program::ID
        || state.resolver.executable
        || !state.resolver.data.is_empty()
    {
        return Err(SourceResolutionError::ContentMismatch);
    }
    Ok(())
}

fn authenticate_loader(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    expected_programdata: [u8; 32],
    expected_slot: u64,
) -> Result<(), SourceResolutionError> {
    let view =
        ProgramV3View::parse(&program.data).map_err(|_| SourceResolutionError::InvalidState)?;
    let (derived, _) =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID);
    let data = ProgramDataV3View::parse(&programdata.data)
        .map_err(|_| SourceResolutionError::InvalidState)?;
    if view.programdata_key() != expected_programdata
        || programdata.key.to_bytes() != expected_programdata
        || programdata.key != derived
        || data.deployment_slot() != expected_slot
    {
        return Err(SourceResolutionError::ContentMismatch);
    }
    Ok(())
}

fn accept_primary_accounts(state: &SourceAcceptPrimaryInlineState) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(state.resolution_state.key, false),
        AccountMeta::new(state.market.key, false),
        AccountMeta::new_readonly(state.resolution_material.key, false),
        AccountMeta::new_readonly(
            state.resolution_material_finalization.staging_cursor.key,
            false,
        ),
        AccountMeta::new_readonly(state.rent_sysvar.key, false),
        AccountMeta::new_readonly(state.clock_sysvar.key, false),
        AccountMeta::new(state.resolver.key, true),
        AccountMeta::new(state.update.key, true),
        AccountMeta::new_readonly(state.receiver_program.key, false),
        AccountMeta::new_readonly(state.receiver_programdata.key, false),
        AccountMeta::new_readonly(state.receiver_config.key, false),
        AccountMeta::new_readonly(state.encoded_vaa.key, false),
        AccountMeta::new_readonly(state.router_program.key, false),
        AccountMeta::new_readonly(state.router_programdata.key, false),
        AccountMeta::new(state.receiver_treasury.key, false),
        AccountMeta::new_readonly(state.system_program.key, false),
    ]
}

fn validate_source_metas(
    kind: SourceFrameKindV1,
    metas: &[AccountMeta],
    state: &SourceCreateResolutionState,
) -> Result<(), SourceResolutionError> {
    let observed = [
        &state.payer,
        &state.resolution_state_destination,
        &state.market,
        &state.resolution_material,
        &state.resolution_material_finalization.staging_cursor,
        &state.rent_sysvar,
        &state.rent_credit,
        &state.system_program,
    ];
    validate_privileges(kind, metas, &observed)
}

fn validate_accept_metas(
    metas: &[AccountMeta],
    state: &SourceAcceptPrimaryInlineState,
) -> Result<(), SourceResolutionError> {
    let observed = [
        &state.resolution_state,
        &state.market,
        &state.resolution_material,
        &state.resolution_material_finalization.staging_cursor,
        &state.rent_sysvar,
        &state.clock_sysvar,
        &state.resolver,
        &state.update,
        &state.receiver_program,
        &state.receiver_programdata,
        &state.receiver_config,
        &state.encoded_vaa,
        &state.router_program,
        &state.router_programdata,
        &state.receiver_treasury,
        &state.system_program,
    ];
    validate_privileges(SourceFrameKindV1::AcceptPrimaryInline, metas, &observed)
}

fn validate_privileges<const N: usize>(
    kind: SourceFrameKindV1,
    metas: &[AccountMeta],
    observed: &[&ObservedAccount; N],
) -> Result<(), SourceResolutionError> {
    if metas.len() != N {
        return Err(SourceResolutionError::InvalidState);
    }
    let privileges: Vec<_> = metas
        .iter()
        .zip(observed)
        .map(|(meta, account)| SourceAccountPrivilegeV1 {
            key: meta.pubkey.to_bytes(),
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
            is_executable: account.executable,
        })
        .collect();
    validate_source_frame_v1(kind, &privileges).map_err(|_| SourceResolutionError::InvalidState)
}

/// The exact Market facts a Source builder is allowed to copy, recomputed from
/// canonically re-encoded, PDA-checked Market state rather than trusted.
struct SourceMarketFacts {
    generation: u64,
    phase: Phase,
    child_count: u64,
    outcome_count: u8,
    resolution_policy_id: [u8; 32],
    rent_refund: [u8; 32],
}

fn source_market(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<SourceMarketFacts, SourceResolutionError> {
    match decode_market_outcome_count(&account.data)
        .map_err(|_| SourceResolutionError::InvalidState)?
    {
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
        _ => Err(SourceResolutionError::InvalidState),
    }
}

fn source_market_width<const N: usize>(
    program_id: Pubkey,
    account: &ObservedAccount,
) -> Result<SourceMarketFacts, SourceResolutionError> {
    let market: CategoricalMarketV1<N> =
        decode_owned(account, program_id, CategoricalMarketV1::decode)?;
    let encoded_len =
        CategoricalMarketV1::<N>::encoded_len().map_err(|_| SourceResolutionError::InvalidState)?;
    let mut canonical = vec![0; encoded_len];
    market
        .encode(&mut canonical)
        .map_err(|_| SourceResolutionError::InvalidState)?;
    if account.data != canonical {
        return Err(SourceResolutionError::InvalidState);
    }
    let identity = hash(&market.root().identity().to_bytes()).to_bytes();
    let (expected, _) = Pubkey::find_program_address(&[MARKET_SEED, &identity], &program_id);
    if account.key != expected {
        return Err(SourceResolutionError::PdaMismatch);
    }
    Ok(SourceMarketFacts {
        generation: market.root().identity().generation(),
        phase: market.root().phase(),
        child_count: market.root().outstanding_children(),
        outcome_count: u8::try_from(N).map_err(|_| SourceResolutionError::InvalidState)?,
        resolution_policy_id: market.root().identity().resolution_policy_id().to_bytes(),
        rent_refund: market.root().rent_refund(),
    })
}

fn authenticate_finalized_bytes(
    program_id: Pubkey,
    rent: &solana_program::rent::Rent,
    account: &ObservedAccount,
    proof: &FinalizedRecordProof,
    schema: [u8; 32],
) -> Result<(), SourceResolutionError> {
    if proof.schema_release_id != schema {
        return Err(SourceResolutionError::FinalizationMismatch);
    }
    foundation::authenticate_finalized_record(program_id, rent, account, proof)
        .map_err(|_| SourceResolutionError::FinalizationMismatch)
}

fn decode_owned<T, E>(
    account: &ObservedAccount,
    program_id: Pubkey,
    decode: impl FnOnce(&[u8]) -> Result<T, E>,
) -> Result<T, SourceResolutionError> {
    if account.owner != program_id || account.executable {
        return Err(SourceResolutionError::InvalidOwner);
    }
    decode(&account.data).map_err(|_| SourceResolutionError::InvalidState)
}

fn observation(accounts: &[&ObservedAccount]) -> Result<Observation, SourceResolutionError> {
    let first = accounts
        .first()
        .ok_or(SourceResolutionError::ObservationMismatch)?
        .observation;
    if first.finality != Finality::Finalized {
        return Err(SourceResolutionError::ObservationNotFinalized);
    }
    if accounts.iter().all(|account| account.observation == first) {
        Ok(first)
    } else {
        Err(SourceResolutionError::ObservationMismatch)
    }
}

fn authenticate_system_actor(account: &ObservedAccount) -> Result<(), SourceResolutionError> {
    if account.owner == system_program::ID && !account.executable && account.data.is_empty() {
        Ok(())
    } else {
        Err(SourceResolutionError::InvalidAuthority)
    }
}

fn authenticate_system_program(account: &ObservedAccount) -> Result<(), SourceResolutionError> {
    if account.key == system_program::ID
        && account.owner == native_loader::ID
        && account.executable
        && account.data.is_empty()
    {
        Ok(())
    } else {
        Err(SourceResolutionError::InvalidOwner)
    }
}
