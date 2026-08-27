//! Chain-derived Source creation and primary-provider acceptance.

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
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use super::{
    VerticalError, authenticate_system_actor, authenticate_system_program, decode_clock,
    source_market,
};
use crate::{
    Observation, ObservedAccount, authenticate_rent_credit,
    foundation::{self, FinalizedRecordProof},
    select_release,
};

const RECEIVER_CONFIG_SEED: &[u8] = b"config";
const RECEIVER_TREASURY_SEED: &[u8] = b"treasury";

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
) -> Result<SourceCreateResolutionReport, VerticalError> {
    let observation = source_create_observation(state)?;
    authenticate_system_actor(&state.payer)?;
    authenticate_system_program(&state.system_program)?;
    let rent =
        foundation::decode_rent(&state.rent_sysvar).map_err(|_| VerticalError::InvalidState)?;
    let market = source_market(program_id, &state.market)?;
    if market.phase != dclutch_core_contract::Phase::Open {
        return Err(VerticalError::InvalidPhase);
    }
    let material = authenticate_material(program_id, state, &rent)?;
    if hash(material.as_bytes()).to_bytes() != market.resolution_policy_id
        || material
            .result_domain()
            .map_err(|_| VerticalError::InvalidState)?
            .outcome_count()
            != market.outcome_count
    {
        return Err(VerticalError::ContentMismatch);
    }
    authenticate_rent_credit(
        program_id,
        &state.rent_credit,
        Pubkey::new_from_array(market.rent_refund),
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    if !rent.is_exempt(state.rent_credit.lamports, state.rent_credit.data.len()) {
        return Err(VerticalError::InvalidState);
    }
    if state.resolution_state_destination.owner != system_program::ID
        || state.resolution_state_destination.executable
        || !state.resolution_state_destination.data.is_empty()
    {
        return Err(VerticalError::InvalidState);
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
        return Err(VerticalError::PdaMismatch);
    }
    let material_id = dclutch_source_contract::ContentId::new(market.resolution_policy_id)
        .map_err(|_| VerticalError::ContentMismatch)?;
    let wire = CreateResolutionInstructionV1::new(
        state.market.key.to_bytes(),
        market.generation,
        material_id,
        market.rent_refund,
        market.child_count,
        bump,
        None,
    )
    .map_err(|_| VerticalError::InvalidState)?;
    let minimum = rent.minimum_balance(SOURCE_RESOLUTION_STATE_BYTES);
    let state_rent_top_up = minimum.saturating_sub(state.resolution_state_destination.lamports);
    if state.payer.lamports < state_rent_top_up {
        return Err(VerticalError::InvalidAuthority);
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
) -> Result<SourceAcceptPrimaryInlineReport, VerticalError> {
    let observation = source_accept_observation(state)?;
    authenticate_system_program(&state.system_program)?;
    let rent =
        foundation::decode_rent(&state.rent_sysvar).map_err(|_| VerticalError::InvalidState)?;
    let clock = decode_clock(&state.clock_sysvar)?;
    if clock.slot == 0 {
        return Err(VerticalError::InvalidState);
    }
    let source = super::decode_owned(
        &state.resolution_state,
        program_id,
        SourceResolutionStateV1::decode,
    )?;
    if source.to_bytes().as_slice() != state.resolution_state.data.as_slice()
        || source.phase() != SourceResolutionPhaseV1::Primary
    {
        return Err(VerticalError::InvalidPhase);
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
    if market.phase != dclutch_core_contract::Phase::Open
        || source.market() != state.market.key.to_bytes()
        || source.generation() != market.generation
        || source.material_id().to_bytes() != market.resolution_policy_id
    {
        return Err(VerticalError::ContentMismatch);
    }
    let material = authenticate_accept_material(program_id, state, &rent)?;
    if hash(material.as_bytes()).to_bytes() != market.resolution_policy_id
        || material
            .result_domain()
            .map_err(|_| VerticalError::InvalidState)?
            .outcome_count()
            != market.outcome_count
    {
        return Err(VerticalError::ContentMismatch);
    }
    let (source_id, source_spec) = material
        .primary_source()
        .map_err(|_| VerticalError::InvalidState)?;
    if source_spec.access_profile() != SourceAccessProfile::PythTerminalOneTransaction {
        return Err(VerticalError::AbiUnavailable);
    }
    let (_, provider) = material
        .primary_provider_release()
        .map_err(|_| VerticalError::InvalidState)?;
    let release = select_release(
        provider.provider_deployment_release_id().to_bytes(),
        provider.decoding_rules_id().to_bytes(),
        provider.transport_profile_id().to_bytes(),
        clock.unix_timestamp,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    let obligation = dclutch_source_contract::PythProviderAdapterObligationV1::from_material_view(
        material, source_id,
    )
    .map_err(|_| VerticalError::ContentMismatch)?;
    if obligation.provider_release() != provider {
        return Err(VerticalError::ContentMismatch);
    }
    let post =
        PostUpdateParamsView::parse(post_update_body).map_err(|_| VerticalError::InvalidState)?;
    authenticate_provider_accounts(state, &release, &post)?;
    let config = ReceiverConfigV2View::parse(&state.receiver_config.data)
        .map_err(|_| VerticalError::InvalidState)?;
    let required_resolver_lamports = rent
        .minimum_balance(FULL_PRICE_UPDATE_V2_LEN)
        .checked_add(config.fee())
        .ok_or(VerticalError::InvalidState)?;
    if state.resolver.lamports < required_resolver_lamports {
        return Err(VerticalError::InvalidAuthority);
    }
    let terminal_sequence = clock.slot;
    let prefix = AcceptEvidenceInstructionV1::new(source.generation(), terminal_sequence)
        .map_err(|_| VerticalError::InvalidState)?
        .to_prefix_bytes();
    let mut data = Vec::with_capacity(
        ACCEPT_EVIDENCE_INSTRUCTION_PREFIX_BYTES
            .checked_add(post_update_body.len())
            .ok_or(VerticalError::InvalidState)?,
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
) -> Result<Observation, VerticalError> {
    super::observation(&[
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
) -> Result<Observation, VerticalError> {
    super::observation(&[
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
) -> Result<SourceMaterialViewV1<'a>, VerticalError> {
    super::authenticate_finalized_bytes(
        program_id,
        rent,
        &state.resolution_material,
        &state.resolution_material_finalization,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    )?;
    SourceMaterialViewV1::decode(&state.resolution_material.data)
        .map_err(|_| VerticalError::InvalidState)
}

fn authenticate_accept_material<'a>(
    program_id: Pubkey,
    state: &'a SourceAcceptPrimaryInlineState,
    rent: &solana_program::rent::Rent,
) -> Result<SourceMaterialViewV1<'a>, VerticalError> {
    super::authenticate_finalized_bytes(
        program_id,
        rent,
        &state.resolution_material,
        &state.resolution_material_finalization,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
    )?;
    SourceMaterialViewV1::decode(&state.resolution_material.data)
        .map_err(|_| VerticalError::InvalidState)
}

fn authenticate_provider_accounts(
    state: &SourceAcceptPrimaryInlineState,
    release: &dclutch_pyth_svm::PythReleaseV1,
    post: &PostUpdateParamsView<'_>,
) -> Result<(), VerticalError> {
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
        return Err(VerticalError::InvalidOwner);
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
        .map_err(|_| VerticalError::InvalidState)?;
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
        return Err(VerticalError::ContentMismatch);
    }
    Ok(())
}

fn authenticate_loader(
    program: &ObservedAccount,
    programdata: &ObservedAccount,
    expected_programdata: [u8; 32],
    expected_slot: u64,
) -> Result<(), VerticalError> {
    let view = ProgramV3View::parse(&program.data).map_err(|_| VerticalError::InvalidState)?;
    let (derived, _) =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID);
    let data =
        ProgramDataV3View::parse(&programdata.data).map_err(|_| VerticalError::InvalidState)?;
    if view.programdata_key() != expected_programdata
        || programdata.key.to_bytes() != expected_programdata
        || programdata.key != derived
        || data.deployment_slot() != expected_slot
    {
        return Err(VerticalError::ContentMismatch);
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
) -> Result<(), VerticalError> {
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
) -> Result<(), VerticalError> {
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
) -> Result<(), VerticalError> {
    if metas.len() != N {
        return Err(VerticalError::InvalidState);
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
    validate_source_frame_v1(kind, &privileges).map_err(|_| VerticalError::InvalidState)
}
