//! Chain-derived construction for real Pyth Receiver submission and reclaim.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_market_core_codec::{
    Action, CoreState, Identity, Phase as CorePhase, Readiness, Request,
};
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_pyth_svm::{PostUpdateParamsView, PythReleaseV1, VerifiedEncodedVaaV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1};
use dclutch_release_set_contract::{
    CallerAuthoritySeedsV1, ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2,
    ProtocolInfrastructureProfileV2,
};
#[cfg(feature = "transaction-planning")]
use dclutch_resolution_codec::{
    PROVIDER_EXECUTION_REQUEST_BYTES_V3, PROVIDER_RECLAIM_REQUEST_BYTES_V3,
    PROVIDER_SUBMIT_REQUEST_BYTES_V3,
};
use dclutch_resolution_codec::{
    PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3, PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
    PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3, PYTH_RELEASE_RECORD_SCHEMA_ID_V1, ProviderCallerV3,
    ProviderExecutionRequestV3, ProviderReclaimRequestV3, ProviderSubmitRequestV3,
    ProviderUpdateLifecycleV3, ProviderUpdateStatusV3, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
};
use dclutch_source_contract::{
    PROVIDER_RELEASE_SCHEMA_ID_V1, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1,
    SourceMaterialV3, SourceResolutionPhaseV1, SourceResolutionStateV2, SourceSpecV1,
    WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};
#[cfg(feature = "transaction-planning")]
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};
#[cfg(feature = "transaction-planning")]
use solana_system_interface::instruction::transfer;

use dclutch_resolution_core_v3_operator::derive_resolution_funding_base_coordinates_v3;
pub use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
#[cfg(feature = "transaction-planning")]
use dclutch_versioned_message_operator::{
    VersionedMessagePlanV0, compile_v0_message_with_optional_tables,
};

/// Resolution submission account count frozen by the physical adapter.
pub const PROVIDER_SUBMIT_ACCOUNT_COUNT_V3: usize = 38;
/// Resolution reclaim account count frozen by the physical adapter.
pub const PROVIDER_RECLAIM_ACCOUNT_COUNT_V3: usize = 18;
/// Core provider execution account count frozen by Resolution V3.
pub const PROVIDER_EXECUTE_ACCOUNT_COUNT_V3: usize = PROVIDER_RESOLUTION_CORE_ACCOUNT_COUNT_V3;

/// First-stage chain coordinates for browser provider submission discovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSubmitBaseCoordinatesV3 {
    /// Canonical Source state for the Market.
    pub source_state: Pubkey,
    /// Finalized SourceMaterial raw record.
    pub source_material: Pubkey,
    /// Immutable refund recipient selected by the Market.
    pub refund_recipient: Pubkey,
    /// Core-owned immutable infrastructure profile.
    pub infrastructure: Pubkey,
    /// Core ProgramData derived from the executable program account.
    pub core_programdata: Pubkey,
    /// Resolution ProgramData derived from the executable program account.
    pub resolution_programdata: Pubkey,
}

/// Second-stage record coordinates derived from authenticated Market material.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSubmitMaterialCoordinatesV3 {
    /// Finalized SourceSpec raw record.
    pub source_spec: Pubkey,
    /// Vacant SourceSpec staging cursor.
    pub source_spec_staging: Pubkey,
    /// Finalized WindowSpec raw record.
    pub window: Pubkey,
    /// Vacant WindowSpec staging cursor.
    pub window_staging: Pubkey,
    /// Infrastructure-selected Registry artifact raw record.
    pub registry_artifact: Pubkey,
    /// Vacant Registry artifact staging cursor.
    pub registry_artifact_staging: Pubkey,
}

/// One raw/staging record pair in the provider discovery graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSubmitRecordCoordinatesV3 {
    /// Finalized raw record.
    pub raw: Pubkey,
    /// Vacant staging cursor.
    pub staging: Pubkey,
}

/// Provider deployment accounts selected by Pyth release and verified VAA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSubmitPythCoordinatesV3 {
    /// Pyth Receiver program.
    pub receiver_program: Pubkey,
    /// Pyth Receiver ProgramData.
    pub receiver_programdata: Pubkey,
    /// Receiver Config.
    pub receiver_config: Pubkey,
    /// Wormhole Router program.
    pub router_program: Pubkey,
    /// Wormhole Router ProgramData.
    pub router_programdata: Pubkey,
    /// GuardianSet selected by the verified EncodedVaa.
    pub guardian_set: Pubkey,
}

/// Fresh account coordinates selected by one submit update signer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSubmitFreshCoordinatesV3 {
    /// Resolution-owned lifecycle PDA.
    pub lifecycle: Pubkey,
    /// Resolution-owned Receiver write-authority PDA.
    pub update_authority: Pubkey,
}

/// Host-side refusal from inconsistent chain observations or intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderTransportOperatorErrorV3 {
    /// Market or Source state was not a current primary pair.
    State,
    /// Finalized record content or graph linkage differed.
    Record,
    /// Provider release, account owner, or router evidence differed.
    Provider,
    /// A deterministic address or exact frame account differed.
    Address,
    /// Submission intent was invalid or contradicted observed state.
    Intent,
    /// Lifecycle was not an exact consumed reclaim candidate.
    Lifecycle,
}

/// Derive the Market-owned starting coordinates for one provider submission.
pub fn derive_provider_submit_base_coordinates_v3(
    market: &ObservedAccount,
    core_program: Pubkey,
    registry_program: Pubkey,
    resolution_program: Pubkey,
) -> Result<ProviderSubmitBaseCoordinatesV3, ProviderTransportOperatorErrorV3> {
    if market.observation.finality != Finality::Finalized {
        return Err(ProviderTransportOperatorErrorV3::State);
    }
    let base = derive_resolution_funding_base_coordinates_v3(
        market.key,
        market.owner,
        market.executable,
        &market.data,
        core_program,
        registry_program,
        resolution_program,
    )
    .map_err(|_| ProviderTransportOperatorErrorV3::State)?;
    Ok(ProviderSubmitBaseCoordinatesV3 {
        source_state: base.source_state,
        source_material: base.source_material,
        refund_recipient: base.beneficiary,
        infrastructure: Pubkey::find_program_address(
            &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
            &core_program,
        )
        .0,
        core_programdata: base.core_programdata,
        resolution_programdata: base.resolution_programdata,
    })
}

/// Continue provider discovery through SourceMaterial and infrastructure.
pub fn derive_provider_submit_material_coordinates_v3(
    market: &ObservedAccount,
    source_material: &ObservedAccount,
    infrastructure: &ObservedAccount,
) -> Result<ProviderSubmitMaterialCoordinatesV3, ProviderTransportOperatorErrorV3> {
    let observation =
        require_same_finalized_observation(&[market, source_material, infrastructure])?;
    if observation != market.observation {
        return Err(ProviderTransportOperatorErrorV3::State);
    }
    let state =
        CoreState::decode(&market.data).map_err(|_| ProviderTransportOperatorErrorV3::State)?;
    let registry = Pubkey::new_from_array(state.identity.registry_program.to_bytes());
    authenticate_raw(
        registry,
        source_material,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        state.identity.resolution_policy.to_bytes(),
    )?;
    let material = SourceMaterialV3::decode(&source_material.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    let expected_infrastructure = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &market.owner,
    )
    .0;
    if infrastructure.key != expected_infrastructure
        || infrastructure.owner != market.owner
        || infrastructure.executable
        || infrastructure.lamports == 0
    {
        return Err(ProviderTransportOperatorErrorV3::Address);
    }
    let profile = ProtocolInfrastructureProfileV2::decode(&infrastructure.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    if profile.registry().program().to_bytes() != registry.to_bytes() {
        return Err(ProviderTransportOperatorErrorV3::Record);
    }
    let source_spec = record_pair(
        registry,
        SOURCE_SPEC_SCHEMA_ID_V1,
        material.primary_source_spec().to_bytes(),
    );
    let window = record_pair(
        registry,
        WINDOW_SPEC_SCHEMA_ID_V1,
        material.window_spec().to_bytes(),
    );
    let registry_artifact = record_pair(
        registry,
        ARTIFACT_RELEASE_SCHEMA_ID_V1,
        profile.registry().artifact_release().to_bytes(),
    );
    Ok(ProviderSubmitMaterialCoordinatesV3 {
        source_spec: source_spec.raw,
        source_spec_staging: source_spec.staging,
        window: window.raw,
        window_staging: window.staging,
        registry_artifact: registry_artifact.raw,
        registry_artifact_staging: registry_artifact.staging,
    })
}

/// Derive the Source ProviderRelease pair from one authenticated SourceSpec.
pub fn derive_provider_submit_provider_release_coordinates_v3(
    registry: Pubkey,
    source_spec: &ObservedAccount,
) -> Result<ProviderSubmitRecordCoordinatesV3, ProviderTransportOperatorErrorV3> {
    if source_spec.observation.finality != Finality::Finalized {
        return Err(ProviderTransportOperatorErrorV3::State);
    }
    authenticate_raw(
        registry,
        source_spec,
        SOURCE_SPEC_SCHEMA_ID_V1,
        hash(&source_spec.data).to_bytes(),
    )?;
    let source = SourceSpecV1::decode(&source_spec.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    Ok(record_pair(
        registry,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        source.provider_release_id().to_bytes(),
    ))
}

/// Derive the Pyth release pair from one authenticated provider release.
pub fn derive_provider_submit_pyth_release_coordinates_v3(
    registry: Pubkey,
    provider_release: &ObservedAccount,
) -> Result<ProviderSubmitRecordCoordinatesV3, ProviderTransportOperatorErrorV3> {
    if provider_release.observation.finality != Finality::Finalized {
        return Err(ProviderTransportOperatorErrorV3::State);
    }
    authenticate_raw(
        registry,
        provider_release,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        hash(&provider_release.data).to_bytes(),
    )?;
    let provider = dclutch_source_contract::ProviderReleaseV1::decode(&provider_release.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    Ok(record_pair(
        registry,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        provider.provider_deployment_release_id().to_bytes(),
    ))
}

/// Derive Receiver and Router coordinates from the selected Pyth release and VAA.
pub fn derive_provider_submit_pyth_coordinates_v3(
    registry: Pubkey,
    pyth_release: &ObservedAccount,
    encoded_vaa: &ObservedAccount,
) -> Result<ProviderSubmitPythCoordinatesV3, ProviderTransportOperatorErrorV3> {
    require_same_finalized_observation(&[pyth_release, encoded_vaa])?;
    authenticate_raw(
        registry,
        pyth_release,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        hash(&pyth_release.data).to_bytes(),
    )?;
    let pyth = PythReleaseV1::decode(&pyth_release.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Provider)?;
    let router_program = Pubkey::new_from_array(pyth.router_program());
    if encoded_vaa.owner != router_program || encoded_vaa.executable {
        return Err(ProviderTransportOperatorErrorV3::Provider);
    }
    let encoded = VerifiedEncodedVaaV1::parse(&encoded_vaa.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Provider)?;
    Ok(ProviderSubmitPythCoordinatesV3 {
        receiver_program: Pubkey::new_from_array(pyth.receiver_program()),
        receiver_programdata: Pubkey::new_from_array(pyth.receiver_programdata()),
        receiver_config: Pubkey::new_from_array(pyth.receiver_config()),
        router_program,
        router_programdata: Pubkey::new_from_array(pyth.router_programdata()),
        guardian_set: Pubkey::find_program_address(
            &[b"GuardianSet", &encoded.guardian_set_index().to_be_bytes()],
            &router_program,
        )
        .0,
    })
}

/// Derive the lifecycle and Receiver authority from immutable submit inputs.
pub fn derive_provider_submit_fresh_coordinates_v3(
    market: Pubkey,
    source_state: Pubkey,
    update_account: Pubkey,
    resolution_program: Pubkey,
) -> ProviderSubmitFreshCoordinatesV3 {
    ProviderSubmitFreshCoordinatesV3 {
        lifecycle: Pubkey::find_program_address(
            &[
                PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
                update_account.as_ref(),
            ],
            &resolution_program,
        )
        .0,
        update_authority: Pubkey::find_program_address(
            &[
                PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
                market.as_ref(),
                source_state.as_ref(),
                update_account.as_ref(),
            ],
            &resolution_program,
        )
        .0,
    }
}

/// Minimal caller intent; all protocol and provider authority is derived from observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSubmitIntentV3 {
    /// Provider fee/rent payer and EncodedVaa write authority.
    pub submitter: Pubkey,
    /// Immutable update-rent refund recipient.
    pub refund_recipient: Pubkey,
    /// Vacant signer generated for Receiver PriceUpdate.
    pub update_account: Pubkey,
    /// Earliest reclaim time after terminal consumption.
    pub reclaim_after_unix_seconds: i64,
    /// Exact Receiver PostUpdateParams body without its Anchor discriminator.
    pub post_update_body: Vec<u8>,
}

/// Same-finalized records that derive one real provider submission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSubmitSnapshotV3 {
    /// Current Market Core state.
    pub market: ObservedAccount,
    /// Current Runtime V2 Source state.
    pub source_state: ObservedAccount,
    /// Finalized `SourceMaterialV3` raw record.
    pub source_material: ObservedAccount,
    /// Finalized primary SourceSpec raw record.
    pub source_spec: ObservedAccount,
    /// Finalized Source ProviderRelease raw record.
    pub source_provider_release: ObservedAccount,
    /// Finalized Pyth deployment release raw record.
    pub pyth_release: ObservedAccount,
    /// Finalized WindowSpec raw record.
    pub window: ObservedAccount,
    /// Router-owned verified EncodedVaa.
    pub encoded_vaa: ObservedAccount,
}

/// Current deployment/account coordinates not authored by provider intent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderSubmitDeploymentV3 {
    /// Core-owned immutable infrastructure profile.
    pub infrastructure: Pubkey,
    /// Registry ProgramData.
    pub registry_programdata: Pubkey,
    /// Registry ArtifactRelease raw record.
    pub registry_artifact: Pubkey,
    /// Registry ArtifactRelease vacant staging cursor.
    pub registry_artifact_staging: Pubkey,
    /// Core ProgramData.
    pub core_programdata: Pubkey,
    /// Resolution program.
    pub resolution_program: Pubkey,
    /// Resolution ProgramData.
    pub resolution_programdata: Pubkey,
    /// Receiver Config.
    pub receiver_config: Pubkey,
    /// Router GuardianSet selected by the signed VAA.
    pub guardian_set: Pubkey,
}

/// Permissionless terminal-consumption intent. Every authority coordinate is
/// derived from same-finalized chain observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderExecuteIntentV3 {
    /// Transaction signer that executes the already-submitted update.
    pub resolver: Pubkey,
    /// Positive Source replay sequence for the terminal certificate.
    pub terminal_sequence: u64,
    /// Exact Receiver PostUpdateParams body committed at submission.
    pub post_update_body: Vec<u8>,
}

/// Same-finalized accounts needed to derive one Core provider execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderExecuteSnapshotV3 {
    /// Open canonical Market.
    pub market: ObservedAccount,
    /// Primary Source state.
    pub source_state: ObservedAccount,
    /// Submitted provider lifecycle.
    pub lifecycle: ObservedAccount,
    /// Receiver-owned provider update.
    pub update: ObservedAccount,
    /// Finalized `SourceMaterialV3`.
    pub source_material: ObservedAccount,
    /// Finalized primary SourceSpec.
    pub source_spec: ObservedAccount,
    /// Finalized Source ProviderRelease.
    pub source_provider_release: ObservedAccount,
    /// Finalized Pyth adapter configuration.
    pub adapter_config: ObservedAccount,
    /// Finalized WindowSpec.
    pub window: ObservedAccount,
    /// Finalized StatisticSpec.
    pub statistic: ObservedAccount,
    /// Finalized Pyth deployment release.
    pub pyth_release: ObservedAccount,
    /// Product Runtime V2 graph root.
    pub product: ObservedAccount,
    /// Product-selected result domain.
    pub result_domain: ObservedAccount,
    /// Product-selected portfolio.
    pub portfolio: ObservedAccount,
}

/// Current deployment accounts reauthenticated by Core and Resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderExecuteDeploymentV3 {
    /// Registry ProgramData.
    pub registry_programdata: Pubkey,
    /// Registry immutable ArtifactRelease record.
    pub registry_artifact: Pubkey,
    /// Vacant Registry ArtifactRelease staging cursor.
    pub registry_artifact_staging: Pubkey,
    /// Current Core ProgramData.
    pub core_programdata: Pubkey,
    /// Current Trading program required by the shared Resolution frame.
    pub trading_program: Pubkey,
    /// Current Trading ProgramData.
    pub trading_programdata: Pubkey,
    /// Current Resolution program.
    pub resolution_program: Pubkey,
    /// Current Resolution ProgramData.
    pub resolution_programdata: Pubkey,
    /// Receiver Config selected by the Pyth release.
    pub receiver_config: Pubkey,
}

/// Constructed unsigned provider instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderTransportReportV3 {
    /// Exact unsigned instruction.
    pub instruction: Instruction,
    /// Finalized observation from which every semantic account was derived.
    pub observation: Observation,
    /// Resolution-owned lifecycle PDA.
    pub lifecycle: Pubkey,
    /// Resolution-owned Receiver write-authority PDA.
    pub update_authority: Pubkey,
}

/// Unsigned packet-safe provider transaction and its exact signing boundary.
#[cfg(feature = "transaction-planning")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderTransportTransactionPlanV3 {
    /// Exact unsigned v0 message and packet geometry.
    pub message: VersionedMessagePlanV0,
    /// Canonical signer order required by the message.
    pub required_signers: Vec<Pubkey>,
}

/// Exact submit message plus its chain-derived lifecycle prepayment.
#[cfg(feature = "transaction-planning")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderSubmitTransactionPlanV3 {
    /// Packet-safe v0 message and exact signer boundary.
    pub transaction: ProviderTransportTransactionPlanV3,
    /// Lamports transferred to the still-vacant lifecycle immediately before
    /// the provider instruction in the same durable message.
    pub lifecycle_top_up_lamports: u64,
}

/// Refusal from a mutated provider report or unsafe transaction routing.
#[cfg(feature = "transaction-planning")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderTransportTransactionErrorV3 {
    /// The instruction no longer had the frozen discriminator, account order,
    /// privilege profile, or request-to-account joins.
    Frame,
    /// Finalized lookup-table selection or signed packet geometry refused.
    Routing(dclutch_versioned_message_operator::Error),
}

/// Compile one exact provider submission with its required lifecycle prepay.
///
/// The fresh Receiver update and Resolution lifecycle are observed at the
/// builder's identical finalized slot. Neither may already contain account
/// data, and the update must remain an entirely vacant System account. The
/// lifecycle may carry a partial prefund, but never more than the caller's
/// authenticated rent minimum. The owner constructs the System transfer and
/// provider action together so an exterior cannot reorder or substitute the
/// prepayment while retaining a superficially valid provider instruction.
#[cfg(feature = "transaction-planning")]
pub fn compile_provider_submit_with_lifecycle_prepay_v0(
    report: &ProviderTransportReportV3,
    update_account: &ObservedAccount,
    lifecycle_account: &ObservedAccount,
    lifecycle_rent_minimum: u64,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
) -> Result<ProviderSubmitTransactionPlanV3, ProviderTransportTransactionErrorV3> {
    let request = validate_submit_report(report)?;
    if update_account.observation != report.observation
        || lifecycle_account.observation != report.observation
        || update_account.key.to_bytes() != request.update_account
        || lifecycle_account.key.to_bytes() != request.lifecycle
        || update_account.owner != system_program::ID
        || lifecycle_account.owner != system_program::ID
        || update_account.executable
        || lifecycle_account.executable
        || !update_account.data.is_empty()
        || !lifecycle_account.data.is_empty()
        || update_account.lamports != 0
        || lifecycle_account.lamports > lifecycle_rent_minimum
    {
        return Err(ProviderTransportTransactionErrorV3::Frame);
    }
    let lifecycle_top_up_lamports = lifecycle_rent_minimum
        .checked_sub(lifecycle_account.lamports)
        .ok_or(ProviderTransportTransactionErrorV3::Frame)?;
    let submitter = Pubkey::new_from_array(request.provider_submitter);
    let instructions = if lifecycle_top_up_lamports == 0 {
        vec![report.instruction.clone()]
    } else {
        vec![
            transfer(
                &submitter,
                &lifecycle_account.key,
                lifecycle_top_up_lamports,
            ),
            report.instruction.clone(),
        ]
    };
    let required_signers = vec![submitter, Pubkey::new_from_array(request.update_account)];
    let transaction = compile_provider_instructions_v0(
        report,
        &instructions,
        recent_blockhash,
        lookup_tables,
        required_signers,
    )?;
    Ok(ProviderSubmitTransactionPlanV3 {
        transaction,
        lifecycle_top_up_lamports,
    })
}

/// Build one exact real-provider submission from same-snapshot chain state.
pub fn build_provider_submit_v3(
    snapshot: &ProviderSubmitSnapshotV3,
    deployment: ProviderSubmitDeploymentV3,
    intent: &ProviderSubmitIntentV3,
) -> Result<ProviderTransportReportV3, ProviderTransportOperatorErrorV3> {
    let observation = require_same_finalized_observation(&[
        &snapshot.market,
        &snapshot.source_state,
        &snapshot.source_material,
        &snapshot.source_spec,
        &snapshot.source_provider_release,
        &snapshot.pyth_release,
        &snapshot.window,
        &snapshot.encoded_vaa,
    ])?;
    PostUpdateParamsView::parse(&intent.post_update_body)
        .map_err(|_| ProviderTransportOperatorErrorV3::Intent)?;
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::State)?;
    let source = SourceResolutionStateV2::decode(&snapshot.source_state.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::State)?;
    if source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != snapshot.market.key.to_bytes()
        || source.generation() != market.identity.generation
        || source.material_id().to_bytes() != market.identity.resolution_policy.to_bytes()
        || snapshot.source_state.owner != deployment.resolution_program
    {
        return Err(ProviderTransportOperatorErrorV3::State);
    }
    let registry = Pubkey::new_from_array(market.identity.registry_program.to_bytes());
    let release_set = market.identity.selected_release_set.to_bytes();
    authenticate_raw(
        registry,
        &snapshot.source_material,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        source.material_id().to_bytes(),
    )?;
    let material = SourceMaterialV3::decode(&snapshot.source_material.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    authenticate_raw(
        registry,
        &snapshot.source_spec,
        SOURCE_SPEC_SCHEMA_ID_V1,
        material.primary_source_spec().to_bytes(),
    )?;
    let source_spec = SourceSpecV1::decode(&snapshot.source_spec.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    authenticate_raw(
        registry,
        &snapshot.source_provider_release,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        source_spec.provider_release_id().to_bytes(),
    )?;
    let provider =
        dclutch_source_contract::ProviderReleaseV1::decode(&snapshot.source_provider_release.data)
            .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    let pyth_id = provider.provider_deployment_release_id().to_bytes();
    authenticate_raw(
        registry,
        &snapshot.pyth_release,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        pyth_id,
    )?;
    let pyth = PythReleaseV1::decode(&snapshot.pyth_release.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Provider)?;
    authenticate_raw(
        registry,
        &snapshot.window,
        WINDOW_SPEC_SCHEMA_ID_V1,
        material.window_spec().to_bytes(),
    )?;
    let window = WindowSpecV1::decode(&snapshot.window.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    if window.source_spec_id() != material.primary_source_spec()
        || intent.reclaim_after_unix_seconds < window.end_unix_seconds()
        || snapshot.encoded_vaa.owner != Pubkey::new_from_array(pyth.router_program())
    {
        return Err(ProviderTransportOperatorErrorV3::Provider);
    }
    let encoded = VerifiedEncodedVaaV1::parse(&snapshot.encoded_vaa.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Provider)?;
    if encoded.write_authority() != intent.submitter.to_bytes() {
        return Err(ProviderTransportOperatorErrorV3::Provider);
    }
    let expected_guardian = Pubkey::find_program_address(
        &[b"GuardianSet", &encoded.guardian_set_index().to_be_bytes()],
        &Pubkey::new_from_array(pyth.router_program()),
    )
    .0;
    if deployment.guardian_set != expected_guardian
        || deployment.receiver_config != Pubkey::new_from_array(pyth.receiver_config())
    {
        return Err(ProviderTransportOperatorErrorV3::Address);
    }
    let fresh = derive_provider_submit_fresh_coordinates_v3(
        snapshot.market.key,
        snapshot.source_state.key,
        intent.update_account,
        deployment.resolution_program,
    );
    let lifecycle = fresh.lifecycle;
    let update_authority = fresh.update_authority;
    let request = ProviderSubmitRequestV3 {
        generation: source.generation(),
        reclaim_after_unix_seconds: intent.reclaim_after_unix_seconds,
        market: snapshot.market.key.to_bytes(),
        source_state: snapshot.source_state.key.to_bytes(),
        lifecycle: lifecycle.to_bytes(),
        source_material: source.material_id().to_bytes(),
        provider_release: pyth_id,
        update_account: intent.update_account.to_bytes(),
        provider_submitter: intent.submitter.to_bytes(),
        refund_recipient: intent.refund_recipient.to_bytes(),
        release_set,
        registry_program: registry.to_bytes(),
        encoded_vaa: snapshot.encoded_vaa.key.to_bytes(),
        post_body_digest: hash(&intent.post_update_body).to_bytes(),
    };
    let mut data = request
        .to_bytes()
        .map_err(|_| ProviderTransportOperatorErrorV3::Intent)?
        .to_vec();
    data.extend_from_slice(&intent.post_update_body);
    let activation =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &registry).0;
    let infrastructure = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &snapshot.market.owner,
    )
    .0;
    if deployment.infrastructure != infrastructure {
        return Err(ProviderTransportOperatorErrorV3::Address);
    }
    let treasury = Pubkey::find_program_address(
        &[b"treasury", &[0]],
        &Pubkey::new_from_array(pyth.receiver_program()),
    )
    .0;
    let material_staging = staging(
        registry,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        source.material_id().to_bytes(),
    );
    let source_staging = staging(
        registry,
        SOURCE_SPEC_SCHEMA_ID_V1,
        material.primary_source_spec().to_bytes(),
    );
    let provider_staging = staging(
        registry,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        source_spec.provider_release_id().to_bytes(),
    );
    let pyth_staging = staging(registry, PYTH_RELEASE_RECORD_SCHEMA_ID_V1, pyth_id);
    let window_staging = staging(
        registry,
        WINDOW_SPEC_SCHEMA_ID_V1,
        material.window_spec().to_bytes(),
    );
    let accounts = vec![
        AccountMeta::new(intent.submitter, true),
        AccountMeta::new(intent.update_account, true),
        AccountMeta::new(lifecycle, false),
        AccountMeta::new_readonly(update_authority, false),
        AccountMeta::new_readonly(intent.refund_recipient, false),
        AccountMeta::new_readonly(snapshot.market.key, false),
        AccountMeta::new_readonly(activation, false),
        AccountMeta::new_readonly(deployment.infrastructure, false),
        AccountMeta::new_readonly(registry, false),
        AccountMeta::new_readonly(deployment.registry_programdata, false),
        AccountMeta::new_readonly(deployment.registry_artifact, false),
        AccountMeta::new_readonly(deployment.registry_artifact_staging, false),
        AccountMeta::new_readonly(snapshot.market.owner, false),
        AccountMeta::new_readonly(deployment.core_programdata, false),
        AccountMeta::new_readonly(deployment.resolution_program, false),
        AccountMeta::new_readonly(deployment.resolution_programdata, false),
        AccountMeta::new_readonly(snapshot.source_state.key, false),
        AccountMeta::new_readonly(snapshot.source_material.key, false),
        AccountMeta::new_readonly(material_staging, false),
        AccountMeta::new_readonly(snapshot.source_spec.key, false),
        AccountMeta::new_readonly(source_staging, false),
        AccountMeta::new_readonly(snapshot.source_provider_release.key, false),
        AccountMeta::new_readonly(provider_staging, false),
        AccountMeta::new_readonly(snapshot.pyth_release.key, false),
        AccountMeta::new_readonly(pyth_staging, false),
        AccountMeta::new_readonly(snapshot.window.key, false),
        AccountMeta::new_readonly(window_staging, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(pyth.receiver_program()), false),
        AccountMeta::new_readonly(Pubkey::new_from_array(pyth.receiver_programdata()), false),
        AccountMeta::new_readonly(deployment.receiver_config, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(pyth.router_program()), false),
        AccountMeta::new_readonly(Pubkey::new_from_array(pyth.router_programdata()), false),
        AccountMeta::new_readonly(snapshot.encoded_vaa.key, false),
        AccountMeta::new_readonly(deployment.guardian_set, false),
        AccountMeta::new(treasury, false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    if accounts.len() != PROVIDER_SUBMIT_ACCOUNT_COUNT_V3 || !distinct(&accounts) {
        return Err(ProviderTransportOperatorErrorV3::Address);
    }
    Ok(ProviderTransportReportV3 {
        instruction: Instruction {
            program_id: deployment.resolution_program,
            accounts,
            data,
        },
        observation,
        lifecycle,
        update_authority,
    })
}

/// The exact Core parent-request bytes and caller-authority PDA one Execute
/// packet carries, derived from coordinates alone.
///
/// [`build_provider_execute_v3`] calls this, so the address is defined in one
/// place. A caller that needs the key *before* it has a report — a lookup-table
/// union planner does, because the table must be frozen before the packet that
/// names it exists — derives the identical key from the identical bytes instead
/// of maintaining a second implementation that can drift.
///
/// Every argument is a coordinate an exterior driver already pins against
/// finalized chain: `market_id` and `generation` are the Market's own identity,
/// `core_program` is the Market's owner. Supplying a coordinate the chain does
/// not agree with yields an address the runtime will refuse, not a forgery.
pub fn provider_execute_caller_authority_v3(
    release_set: [u8; 32],
    market: Pubkey,
    market_id: Identity,
    generation: u64,
    source_state: Pubkey,
    core_program: Pubkey,
) -> Result<(Vec<u8>, Pubkey), ProviderTransportOperatorErrorV3> {
    let core_request = Request::administrative(Action::ExecuteProvider, generation, market_id);
    let core_bytes = core_request
        .encode()
        .map_err(|_| ProviderTransportOperatorErrorV3::Intent)?;
    let parent_request_digest = hash(&core_bytes).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        release_set,
        market.to_bytes(),
        ExecutionRoleV1::Core,
        source_state.to_bytes(),
        parent_request_digest,
    )
    .map_err(|_| ProviderTransportOperatorErrorV3::Address)?;
    Ok((
        core_bytes.to_vec(),
        Pubkey::find_program_address(&caller_seeds.as_slices(), &core_program).0,
    ))
}

/// Build the permissionless Core-caller action that consumes one submitted provider update.
///
/// The only caller choices are resolver, positive terminal sequence, and the
/// exact provider body already committed by the lifecycle. Market, Source,
/// Product, release, provider, and caller-authority coordinates are derived
/// from same-finalized observations.
pub fn build_provider_execute_v3(
    snapshot: &ProviderExecuteSnapshotV3,
    deployment: ProviderExecuteDeploymentV3,
    intent: &ProviderExecuteIntentV3,
) -> Result<ProviderTransportReportV3, ProviderTransportOperatorErrorV3> {
    let observation = require_same_finalized_observation(&[
        &snapshot.market,
        &snapshot.source_state,
        &snapshot.lifecycle,
        &snapshot.update,
        &snapshot.source_material,
        &snapshot.source_spec,
        &snapshot.source_provider_release,
        &snapshot.adapter_config,
        &snapshot.window,
        &snapshot.statistic,
        &snapshot.pyth_release,
        &snapshot.product,
        &snapshot.result_domain,
        &snapshot.portfolio,
    ])?;
    PostUpdateParamsView::parse(&intent.post_update_body)
        .map_err(|_| ProviderTransportOperatorErrorV3::Intent)?;
    let market = CoreState::decode(&snapshot.market.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::State)?;
    let source = SourceResolutionStateV2::decode(&snapshot.source_state.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::State)?;
    let lifecycle = ProviderUpdateLifecycleV3::decode(&snapshot.lifecycle.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Lifecycle)?;
    if market.phase != CorePhase::Open
        || market.readiness != Readiness::Consumed
        || source.phase() != SourceResolutionPhaseV1::Primary
        || source.market() != snapshot.market.key.to_bytes()
        || source.generation() != market.identity.generation
        || source.material_id().to_bytes() != market.identity.resolution_policy.to_bytes()
        || snapshot.source_state.owner != deployment.resolution_program
        || lifecycle.status != ProviderUpdateStatusV3::Submitted
        || snapshot.lifecycle.owner != deployment.resolution_program
        || lifecycle.market != snapshot.market.key.to_bytes()
        || lifecycle.source_state != snapshot.source_state.key.to_bytes()
        || lifecycle.source_material != source.material_id().to_bytes()
        || lifecycle.update_account != snapshot.update.key.to_bytes()
        || lifecycle.update_digest != hash(&snapshot.update.data).to_bytes()
        || lifecycle.post_body_digest != hash(&intent.post_update_body).to_bytes()
        || lifecycle.release_set != market.identity.selected_release_set.to_bytes()
        || lifecycle.registry_program != market.identity.registry_program.to_bytes()
        || intent.terminal_sequence == 0
    {
        return Err(ProviderTransportOperatorErrorV3::State);
    }
    let registry = Pubkey::new_from_array(market.identity.registry_program.to_bytes());
    let release_set = market.identity.selected_release_set.to_bytes();
    authenticate_raw(
        registry,
        &snapshot.source_material,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        source.material_id().to_bytes(),
    )?;
    let material = SourceMaterialV3::decode(&snapshot.source_material.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    authenticate_raw(
        registry,
        &snapshot.source_spec,
        SOURCE_SPEC_SCHEMA_ID_V1,
        material.primary_source_spec().to_bytes(),
    )?;
    let source_spec = SourceSpecV1::decode(&snapshot.source_spec.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    authenticate_raw(
        registry,
        &snapshot.source_provider_release,
        PROVIDER_RELEASE_SCHEMA_ID_V1,
        source_spec.provider_release_id().to_bytes(),
    )?;
    let source_provider =
        dclutch_source_contract::ProviderReleaseV1::decode(&snapshot.source_provider_release.data)
            .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    authenticate_raw(
        registry,
        &snapshot.adapter_config,
        PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
        source_spec.adapter_config_id().to_bytes(),
    )?;
    authenticate_raw(
        registry,
        &snapshot.window,
        WINDOW_SPEC_SCHEMA_ID_V1,
        material.window_spec().to_bytes(),
    )?;
    authenticate_raw(
        registry,
        &snapshot.statistic,
        STATISTIC_SPEC_SCHEMA_ID_V1,
        material.statistic_spec().to_bytes(),
    )?;
    let pyth_id = source_provider.provider_deployment_release_id().to_bytes();
    authenticate_raw(
        registry,
        &snapshot.pyth_release,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        pyth_id,
    )?;
    let pyth = PythReleaseV1::decode(&snapshot.pyth_release.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Provider)?;
    if lifecycle.provider_release != pyth_id
        || snapshot.update.owner != Pubkey::new_from_array(pyth.receiver_program())
        || deployment.receiver_config != Pubkey::new_from_array(pyth.receiver_config())
    {
        return Err(ProviderTransportOperatorErrorV3::Provider);
    }

    authenticate_raw(
        registry,
        &snapshot.product,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        market.identity.product_record.to_bytes(),
    )?;
    let product = ProductRecordV2::decode(&snapshot.product.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Record)?;
    if product.product_id().to_bytes() != market.identity.product_id.to_bytes() {
        return Err(ProviderTransportOperatorErrorV3::Record);
    }
    authenticate_raw(
        registry,
        &snapshot.result_domain,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        product.result_domain_digest().to_bytes(),
    )?;
    authenticate_raw(
        registry,
        &snapshot.portfolio,
        PORTFOLIO_SCHEMA_ID_V2,
        product.portfolio_digest().to_bytes(),
    )?;

    let expected_lifecycle = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            snapshot.update.key.as_ref(),
        ],
        &deployment.resolution_program,
    )
    .0;
    let update_authority = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            snapshot.market.key.as_ref(),
            snapshot.source_state.key.as_ref(),
            snapshot.update.key.as_ref(),
        ],
        &deployment.resolution_program,
    )
    .0;
    if snapshot.lifecycle.key != expected_lifecycle
        || lifecycle.update_authority != update_authority.to_bytes()
    {
        return Err(ProviderTransportOperatorErrorV3::Address);
    }
    let certificate = Pubkey::find_program_address(
        &[
            RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
            snapshot.source_state.key.as_ref(),
            &[1],
            &intent.terminal_sequence.to_le_bytes(),
        ],
        &deployment.resolution_program,
    )
    .0;
    let (core_bytes, caller_authority) = provider_execute_caller_authority_v3(
        release_set,
        snapshot.market.key,
        market.identity.market_id,
        market.identity.generation,
        snapshot.source_state.key,
        snapshot.market.owner,
    )?;
    let parent_request_digest = hash(&core_bytes).to_bytes();
    let provider_request = ProviderExecutionRequestV3 {
        caller: ProviderCallerV3::Core,
        generation: market.identity.generation,
        terminal_sequence: intent.terminal_sequence,
        market: snapshot.market.key.to_bytes(),
        source_state: snapshot.source_state.key.to_bytes(),
        certificate_account: certificate.to_bytes(),
        source_material: source.material_id().to_bytes(),
        source_spec: material.primary_source_spec().to_bytes(),
        product_record: market.identity.product_record.to_bytes(),
        result_domain: product.result_domain_digest().to_bytes(),
        provider_release: pyth_id,
        update_account: snapshot.update.key.to_bytes(),
        expected_update_digest: lifecycle.update_digest,
        provider_submitter: lifecycle.provider_submitter,
        resolver: intent.resolver.to_bytes(),
        caller_program: snapshot.market.owner.to_bytes(),
        release_set,
        capability_program_set: [0; 32],
        selected_capability_program: [0; 32],
        parent_request_digest,
        post_params_body_digest: lifecycle.post_body_digest,
    };
    let mut data = core_bytes;
    data.extend_from_slice(
        &provider_request
            .to_bytes()
            .map_err(|_| ProviderTransportOperatorErrorV3::Intent)?,
    );
    data.extend_from_slice(&intent.post_update_body);

    let activation =
        Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &registry).0;
    let infrastructure = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &snapshot.market.owner,
    )
    .0;
    let accounts = vec![
        AccountMeta::new_readonly(caller_authority, false),
        AccountMeta::new_readonly(intent.resolver, true),
        AccountMeta::new(snapshot.source_state.key, false),
        AccountMeta::new(certificate, false),
        AccountMeta::new_readonly(snapshot.market.key, false),
        AccountMeta::new_readonly(activation, false),
        AccountMeta::new_readonly(infrastructure, false),
        AccountMeta::new_readonly(registry, false),
        AccountMeta::new_readonly(deployment.registry_programdata, false),
        AccountMeta::new_readonly(deployment.registry_artifact, false),
        AccountMeta::new_readonly(deployment.registry_artifact_staging, false),
        AccountMeta::new_readonly(snapshot.market.owner, false),
        AccountMeta::new_readonly(deployment.core_programdata, false),
        AccountMeta::new_readonly(deployment.trading_program, false),
        AccountMeta::new_readonly(deployment.trading_programdata, false),
        AccountMeta::new_readonly(deployment.resolution_program, false),
        AccountMeta::new_readonly(deployment.resolution_programdata, false),
        raw_meta(&snapshot.source_material),
        staging_meta(
            registry,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
            source.material_id().to_bytes(),
        ),
        raw_meta(&snapshot.source_spec),
        staging_meta(
            registry,
            SOURCE_SPEC_SCHEMA_ID_V1,
            material.primary_source_spec().to_bytes(),
        ),
        raw_meta(&snapshot.source_provider_release),
        staging_meta(
            registry,
            PROVIDER_RELEASE_SCHEMA_ID_V1,
            source_spec.provider_release_id().to_bytes(),
        ),
        raw_meta(&snapshot.adapter_config),
        staging_meta(
            registry,
            PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
            source_spec.adapter_config_id().to_bytes(),
        ),
        raw_meta(&snapshot.window),
        staging_meta(
            registry,
            WINDOW_SPEC_SCHEMA_ID_V1,
            material.window_spec().to_bytes(),
        ),
        raw_meta(&snapshot.statistic),
        staging_meta(
            registry,
            STATISTIC_SPEC_SCHEMA_ID_V1,
            material.statistic_spec().to_bytes(),
        ),
        raw_meta(&snapshot.pyth_release),
        staging_meta(registry, PYTH_RELEASE_RECORD_SCHEMA_ID_V1, pyth_id),
        raw_meta(&snapshot.product),
        staging_meta(
            registry,
            PRODUCT_RECORD_SCHEMA_ID_V2,
            market.identity.product_record.to_bytes(),
        ),
        raw_meta(&snapshot.result_domain),
        staging_meta(
            registry,
            RESULT_DOMAIN_SCHEMA_ID_V2,
            product.result_domain_digest().to_bytes(),
        ),
        raw_meta(&snapshot.portfolio),
        staging_meta(
            registry,
            PORTFOLIO_SCHEMA_ID_V2,
            product.portfolio_digest().to_bytes(),
        ),
        AccountMeta::new(snapshot.lifecycle.key, false),
        AccountMeta::new_readonly(snapshot.update.key, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(pyth.receiver_program()), false),
        AccountMeta::new_readonly(Pubkey::new_from_array(pyth.receiver_programdata()), false),
        AccountMeta::new_readonly(deployment.receiver_config, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(pyth.router_program()), false),
        AccountMeta::new_readonly(Pubkey::new_from_array(pyth.router_programdata()), false),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    if accounts.len() != PROVIDER_EXECUTE_ACCOUNT_COUNT_V3 || !distinct(&accounts) {
        return Err(ProviderTransportOperatorErrorV3::Address);
    }
    Ok(ProviderTransportReportV3 {
        instruction: Instruction {
            program_id: snapshot.market.owner,
            accounts,
            data,
        },
        observation,
        lifecycle: snapshot.lifecycle.key,
        update_authority,
    })
}

/// Current addresses required to reclaim a lifecycle-derived Receiver update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderReclaimDeploymentV3 {
    /// Permissionless resolver transaction signer.
    pub resolver: Pubkey,
    /// Current Registry ProgramData.
    pub registry_programdata: Pubkey,
    /// Resolution program.
    pub resolution_program: Pubkey,
    /// Current Resolution ProgramData.
    pub resolution_programdata: Pubkey,
}

/// Build permissionless reclaim solely from a consumed lifecycle and pinned release record.
pub fn build_provider_reclaim_v3(
    lifecycle_account: &ObservedAccount,
    pyth_release: &ObservedAccount,
    deployment: ProviderReclaimDeploymentV3,
) -> Result<ProviderTransportReportV3, ProviderTransportOperatorErrorV3> {
    let observation = require_same_finalized_observation(&[lifecycle_account, pyth_release])?;
    let lifecycle = ProviderUpdateLifecycleV3::decode(&lifecycle_account.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Lifecycle)?;
    if lifecycle.status != ProviderUpdateStatusV3::Consumed
        || lifecycle_account.owner != deployment.resolution_program
    {
        return Err(ProviderTransportOperatorErrorV3::Lifecycle);
    }
    let registry = Pubkey::new_from_array(lifecycle.registry_program);
    authenticate_raw(
        registry,
        pyth_release,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        lifecycle.provider_release,
    )?;
    let release = PythReleaseV1::decode(&pyth_release.data)
        .map_err(|_| ProviderTransportOperatorErrorV3::Provider)?;
    let request = ProviderReclaimRequestV3 {
        generation: lifecycle.generation,
        terminal_sequence: lifecycle.terminal_sequence,
        market: lifecycle.market,
        source_state: lifecycle.source_state,
        lifecycle: lifecycle_account.key.to_bytes(),
        certificate: lifecycle.certificate,
        update_account: lifecycle.update_account,
        resolver: deployment.resolver.to_bytes(),
        refund_recipient: lifecycle.refund_recipient,
        release_set: lifecycle.release_set,
    };
    let data = request
        .to_bytes()
        .map_err(|_| ProviderTransportOperatorErrorV3::Lifecycle)?
        .to_vec();
    let activation = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &lifecycle.release_set],
        &registry,
    )
    .0;
    let update_authority = Pubkey::new_from_array(lifecycle.update_authority);
    let pyth_staging = staging(
        registry,
        PYTH_RELEASE_RECORD_SCHEMA_ID_V1,
        lifecycle.provider_release,
    );
    let accounts = vec![
        AccountMeta::new_readonly(deployment.resolver, true),
        AccountMeta::new(lifecycle_account.key, false),
        AccountMeta::new(Pubkey::new_from_array(lifecycle.update_account), false),
        AccountMeta::new(update_authority, false),
        AccountMeta::new(Pubkey::new_from_array(lifecycle.refund_recipient), false),
        AccountMeta::new_readonly(Pubkey::new_from_array(lifecycle.certificate), false),
        AccountMeta::new_readonly(activation, false),
        AccountMeta::new_readonly(registry, false),
        AccountMeta::new_readonly(deployment.registry_programdata, false),
        AccountMeta::new_readonly(deployment.resolution_program, false),
        AccountMeta::new_readonly(deployment.resolution_programdata, false),
        AccountMeta::new_readonly(pyth_release.key, false),
        AccountMeta::new_readonly(pyth_staging, false),
        AccountMeta::new_readonly(Pubkey::new_from_array(release.receiver_program()), false),
        AccountMeta::new_readonly(
            Pubkey::new_from_array(release.receiver_programdata()),
            false,
        ),
        AccountMeta::new_readonly(sysvar::clock::ID, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
    ];
    if accounts.len() != PROVIDER_RECLAIM_ACCOUNT_COUNT_V3 || !distinct(&accounts) {
        return Err(ProviderTransportOperatorErrorV3::Address);
    }
    Ok(ProviderTransportReportV3 {
        instruction: Instruction {
            program_id: deployment.resolution_program,
            accounts,
            data,
        },
        observation,
        lifecycle: lifecycle_account.key,
        update_authority,
    })
}

/// Compile one exact provider submission into an unsigned v0 message.
///
/// The provider submitter is the fee payer and the Receiver update account is
/// the second required signer. No key is generated, signed, or submitted here.
/// A lookup table is optional at the API boundary, but the 38-account route
/// will normally refuse the 1,232-byte packet limit until a finalized active
/// table contributes addresses.
#[cfg(feature = "transaction-planning")]
pub fn compile_provider_submit_v0(
    report: &ProviderTransportReportV3,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
) -> Result<ProviderTransportTransactionPlanV3, ProviderTransportTransactionErrorV3> {
    let request = validate_submit_report(report)?;
    let required_signers = vec![
        Pubkey::new_from_array(request.provider_submitter),
        Pubkey::new_from_array(request.update_account),
    ];
    compile_provider_v0(report, recent_blockhash, lookup_tables, required_signers)
}

/// Compile one exact Core-caller provider execution into an unsigned v0 message.
///
/// The resolver is a **readonly** signer: Core's outer frame demands it
/// (`validate_outer_frame` writes `let writable = matches!(index, SOURCE_STATE
/// | CERTIFICATE | LIFECYCLE)`), Core forwards the message's writability into
/// the CPI, and the Resolution program demands the same thing at its own index.
/// Solana's message compiler unconditionally promotes the fee payer to a
/// *writable* signer, so the fee payer cannot be the resolver and cannot be any
/// other account the instruction names — either choice would flip a privilege
/// the frame pins and the packet would be refused on chain with
/// `CoreSbfError::AccountFrame`. `payer` is therefore explicit and separate, and
/// aliasing is refused here rather than 1,200 bytes and one cluster round-trip
/// later. The Core caller PDA becomes a signer only inside the onchain CPI.
#[cfg(feature = "transaction-planning")]
pub fn compile_provider_execute_v0(
    report: &ProviderTransportReportV3,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
    payer: Pubkey,
) -> Result<ProviderTransportTransactionPlanV3, ProviderTransportTransactionErrorV3> {
    let request = validate_execute_report(report)?;
    let required_signers = readonly_signer_frame_signers(report, payer, request.resolver)?;
    compile_provider_v0(report, recent_blockhash, lookup_tables, required_signers)
}

/// Compile one exact permissionless reclaim into an unsigned v0 message.
///
/// Reclaim's frame is Execute's shape, not Submit's:
/// `authenticate_reclaim_privileges` demands `is_signer != (index == 0)` and
/// `is_writable != matches!(index, 1..=4)`, so its sole instruction signer — the
/// resolver, at index 0 — must also be readonly. The fee payer is separate for
/// exactly the reason given on [`compile_provider_execute_v0`]. The Resolution
/// update-authority PDA signs only the Receiver CPI onchain.
#[cfg(feature = "transaction-planning")]
pub fn compile_provider_reclaim_v0(
    report: &ProviderTransportReportV3,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
    payer: Pubkey,
) -> Result<ProviderTransportTransactionPlanV3, ProviderTransportTransactionErrorV3> {
    let request = validate_reclaim_report(report)?;
    let required_signers = readonly_signer_frame_signers(report, payer, request.resolver)?;
    compile_provider_v0(report, recent_blockhash, lookup_tables, required_signers)
}

/// The signer vector for a frame whose sole instruction signer must stay readonly.
///
/// Order is load-bearing twice over: the fee payer is always the message's first
/// signature, and the readonly signer sorts after it, so `[payer, resolver]` is
/// both the compiler's own ordering and the order a caller must present keys in.
#[cfg(feature = "transaction-planning")]
fn readonly_signer_frame_signers(
    report: &ProviderTransportReportV3,
    payer: Pubkey,
    resolver: [u8; 32],
) -> Result<Vec<Pubkey>, ProviderTransportTransactionErrorV3> {
    let resolver = Pubkey::new_from_array(resolver);
    if payer == resolver
        || payer == Pubkey::default()
        || report
            .instruction
            .accounts
            .iter()
            .any(|account| account.pubkey == payer)
    {
        return Err(ProviderTransportTransactionErrorV3::Frame);
    }
    Ok(vec![payer, resolver])
}

#[cfg(feature = "transaction-planning")]
fn compile_provider_v0(
    report: &ProviderTransportReportV3,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
    required_signers: Vec<Pubkey>,
) -> Result<ProviderTransportTransactionPlanV3, ProviderTransportTransactionErrorV3> {
    compile_provider_instructions_v0(
        report,
        core::slice::from_ref(&report.instruction),
        recent_blockhash,
        lookup_tables,
        required_signers,
    )
}

#[cfg(feature = "transaction-planning")]
fn compile_provider_instructions_v0(
    report: &ProviderTransportReportV3,
    instructions: &[Instruction],
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
    required_signers: Vec<Pubkey>,
) -> Result<ProviderTransportTransactionPlanV3, ProviderTransportTransactionErrorV3> {
    let payer = *required_signers
        .first()
        .ok_or(ProviderTransportTransactionErrorV3::Frame)?;
    let message = compile_v0_message_with_optional_tables(
        payer,
        instructions,
        recent_blockhash,
        report.observation,
        lookup_tables,
    )
    .map_err(ProviderTransportTransactionErrorV3::Routing)?;
    if usize::from(message.required_signatures) != required_signers.len() {
        return Err(ProviderTransportTransactionErrorV3::Frame);
    }
    Ok(ProviderTransportTransactionPlanV3 {
        message,
        required_signers,
    })
}

#[cfg(feature = "transaction-planning")]
fn validate_submit_report(
    report: &ProviderTransportReportV3,
) -> Result<ProviderSubmitRequestV3, ProviderTransportTransactionErrorV3> {
    let accounts = &report.instruction.accounts;
    let prefix = report
        .instruction
        .data
        .get(..PROVIDER_SUBMIT_REQUEST_BYTES_V3)
        .ok_or(ProviderTransportTransactionErrorV3::Frame)?;
    let request = ProviderSubmitRequestV3::decode(prefix)
        .map_err(|_| ProviderTransportTransactionErrorV3::Frame)?;
    let body = report
        .instruction
        .data
        .get(PROVIDER_SUBMIT_REQUEST_BYTES_V3..)
        .filter(|bytes| !bytes.is_empty())
        .ok_or(ProviderTransportTransactionErrorV3::Frame)?;
    if accounts.len() != PROVIDER_SUBMIT_ACCOUNT_COUNT_V3
        || report.instruction.program_id != account_key(accounts, 14)?
        || account_key(accounts, 0)?.to_bytes() != request.provider_submitter
        || account_key(accounts, 1)?.to_bytes() != request.update_account
        || account_key(accounts, 2)?.to_bytes() != request.lifecycle
        || account_key(accounts, 3)? != report.update_authority
        || account_key(accounts, 4)?.to_bytes() != request.refund_recipient
        || account_key(accounts, 5)?.to_bytes() != request.market
        || account_key(accounts, 8)?.to_bytes() != request.registry_program
        || account_key(accounts, 16)?.to_bytes() != request.source_state
        || account_key(accounts, 32)?.to_bytes() != request.encoded_vaa
        || report.lifecycle.to_bytes() != request.lifecycle
        || hash(body).to_bytes() != request.post_body_digest
        || !exact_submit_privileges(accounts)
        || !distinct(accounts)
    {
        return Err(ProviderTransportTransactionErrorV3::Frame);
    }
    Ok(request)
}

#[cfg(feature = "transaction-planning")]
fn validate_reclaim_report(
    report: &ProviderTransportReportV3,
) -> Result<ProviderReclaimRequestV3, ProviderTransportTransactionErrorV3> {
    let accounts = &report.instruction.accounts;
    let request = ProviderReclaimRequestV3::decode(&report.instruction.data)
        .map_err(|_| ProviderTransportTransactionErrorV3::Frame)?;
    if report.instruction.data.len() != PROVIDER_RECLAIM_REQUEST_BYTES_V3
        || accounts.len() != PROVIDER_RECLAIM_ACCOUNT_COUNT_V3
        || report.instruction.program_id != account_key(accounts, 9)?
        || account_key(accounts, 0)?.to_bytes() != request.resolver
        || account_key(accounts, 1)?.to_bytes() != request.lifecycle
        || account_key(accounts, 2)?.to_bytes() != request.update_account
        || account_key(accounts, 3)? != report.update_authority
        || account_key(accounts, 4)?.to_bytes() != request.refund_recipient
        || account_key(accounts, 5)?.to_bytes() != request.certificate
        || report.lifecycle.to_bytes() != request.lifecycle
        || !exact_reclaim_privileges(accounts)
        || !distinct(accounts)
    {
        return Err(ProviderTransportTransactionErrorV3::Frame);
    }
    Ok(request)
}

#[cfg(feature = "transaction-planning")]
fn validate_execute_report(
    report: &ProviderTransportReportV3,
) -> Result<ProviderExecutionRequestV3, ProviderTransportTransactionErrorV3> {
    let accounts = &report.instruction.accounts;
    let core_end = dclutch_market_core_codec::REQUEST_BYTES;
    let provider_end = core_end
        .checked_add(PROVIDER_EXECUTION_REQUEST_BYTES_V3)
        .ok_or(ProviderTransportTransactionErrorV3::Frame)?;
    let core_bytes = report
        .instruction
        .data
        .get(..core_end)
        .ok_or(ProviderTransportTransactionErrorV3::Frame)?;
    let core_request =
        Request::decode(core_bytes).map_err(|_| ProviderTransportTransactionErrorV3::Frame)?;
    let provider_bytes = report
        .instruction
        .data
        .get(core_end..provider_end)
        .ok_or(ProviderTransportTransactionErrorV3::Frame)?;
    let request = ProviderExecutionRequestV3::decode(provider_bytes)
        .map_err(|_| ProviderTransportTransactionErrorV3::Frame)?;
    let body = report
        .instruction
        .data
        .get(provider_end..)
        .filter(|bytes| !bytes.is_empty())
        .ok_or(ProviderTransportTransactionErrorV3::Frame)?;
    if core_request.action != Action::ExecuteProvider
        || report.instruction.program_id != account_key(accounts, 11)?
        || accounts.len() != PROVIDER_EXECUTE_ACCOUNT_COUNT_V3
        || request.caller != ProviderCallerV3::Core
        || request.market != core_request.market.to_bytes()
        || request.generation != core_request.generation
        || request.parent_request_digest != hash(core_bytes).to_bytes()
        || request.caller_program != report.instruction.program_id.to_bytes()
        || account_key(accounts, 0)?
            != Pubkey::find_program_address(
                &CallerAuthoritySeedsV1::from_bytes(
                    request.release_set,
                    request.market,
                    ExecutionRoleV1::Core,
                    request.source_state,
                    request.parent_request_digest,
                )
                .map_err(|_| ProviderTransportTransactionErrorV3::Frame)?
                .as_slices(),
                &report.instruction.program_id,
            )
            .0
        || account_key(accounts, 1)?.to_bytes() != request.resolver
        || account_key(accounts, 2)?.to_bytes() != request.source_state
        || account_key(accounts, 3)?.to_bytes() != request.certificate_account
        || account_key(accounts, 4)?.to_bytes() != request.market
        || account_key(accounts, 37)? != report.lifecycle
        || account_key(accounts, 38)?.to_bytes() != request.update_account
        || hash(body).to_bytes() != request.post_params_body_digest
        || !exact_execute_privileges(accounts)
        || !distinct(accounts)
    {
        return Err(ProviderTransportTransactionErrorV3::Frame);
    }
    Ok(request)
}

#[cfg(feature = "transaction-planning")]
fn account_key(
    accounts: &[AccountMeta],
    index: usize,
) -> Result<Pubkey, ProviderTransportTransactionErrorV3> {
    accounts
        .get(index)
        .map(|account| account.pubkey)
        .ok_or(ProviderTransportTransactionErrorV3::Frame)
}

#[cfg(feature = "transaction-planning")]
fn exact_submit_privileges(accounts: &[AccountMeta]) -> bool {
    accounts.iter().enumerate().all(|(index, account)| {
        let expected_signer = matches!(index, 0 | 1);
        let expected_writable = matches!(index, 0 | 1 | 2 | 34);
        account.is_signer == expected_signer && account.is_writable == expected_writable
    })
}

#[cfg(feature = "transaction-planning")]
fn exact_reclaim_privileges(accounts: &[AccountMeta]) -> bool {
    accounts.iter().enumerate().all(|(index, account)| {
        let expected_signer = index == 0;
        let expected_writable = matches!(index, 1..=4);
        account.is_signer == expected_signer && account.is_writable == expected_writable
    })
}

#[cfg(feature = "transaction-planning")]
fn exact_execute_privileges(accounts: &[AccountMeta]) -> bool {
    accounts.iter().enumerate().all(|(index, account)| {
        let expected_signer = index == 1;
        let expected_writable = matches!(index, 2 | 3 | 37);
        account.is_signer == expected_signer && account.is_writable == expected_writable
    })
}

fn authenticate_raw(
    registry: Pubkey,
    observed: &ObservedAccount,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<(), ProviderTransportOperatorErrorV3> {
    let expected =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    if observed.key != expected
        || observed.owner != registry
        || observed.executable
        || hash(&observed.data).to_bytes() != digest
    {
        Err(ProviderTransportOperatorErrorV3::Record)
    } else {
        Ok(())
    }
}

fn staging(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0
}

fn record_pair(
    registry: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> ProviderSubmitRecordCoordinatesV3 {
    ProviderSubmitRecordCoordinatesV3 {
        raw: Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0,
        staging: staging(registry, schema, digest),
    }
}

fn raw_meta(account: &ObservedAccount) -> AccountMeta {
    AccountMeta::new_readonly(account.key, false)
}

fn staging_meta(registry: Pubkey, schema: [u8; 32], digest: [u8; 32]) -> AccountMeta {
    AccountMeta::new_readonly(staging(registry, schema, digest), false)
}

fn distinct(accounts: &[AccountMeta]) -> bool {
    accounts.iter().enumerate().all(|(index, account)| {
        accounts
            .iter()
            .skip(index + 1)
            .all(|other| other.pubkey != account.pubkey)
    })
}

fn require_same_finalized_observation(
    accounts: &[&ObservedAccount],
) -> Result<Observation, ProviderTransportOperatorErrorV3> {
    let first = accounts
        .first()
        .ok_or(ProviderTransportOperatorErrorV3::State)?
        .observation;
    if first.finality != Finality::Finalized
        || accounts.iter().any(|account| account.observation != first)
    {
        Err(ProviderTransportOperatorErrorV3::State)
    } else {
        Ok(first)
    }
}

#[cfg(all(test, feature = "transaction-planning"))]
#[allow(clippy::indexing_slicing, clippy::needless_range_loop)]
mod tests {
    use super::*;
    use solana_address_lookup_table_interface::{
        program,
        state::{AddressLookupTable, LookupTableMeta},
    };
    use std::borrow::Cow;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn observation(slot: u64) -> Observation {
        Observation {
            slot,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn account_frame(count: usize, program_index: usize) -> Vec<AccountMeta> {
        (0..count)
            .map(|index| {
                let address = if index == program_index {
                    key(200)
                } else {
                    key(u8::try_from(index + 1).expect("small frame"))
                };
                AccountMeta::new_readonly(address, false)
            })
            .collect()
    }

    fn submit_report() -> ProviderTransportReportV3 {
        let mut accounts = account_frame(PROVIDER_SUBMIT_ACCOUNT_COUNT_V3, 14);
        for index in [0, 1, 2, 34] {
            accounts[index].is_writable = true;
        }
        accounts[0].is_signer = true;
        accounts[1].is_signer = true;
        let body = vec![0xa5; 64];
        let request = ProviderSubmitRequestV3 {
            generation: 7,
            reclaim_after_unix_seconds: 1_800_000_100,
            market: accounts[5].pubkey.to_bytes(),
            source_state: accounts[16].pubkey.to_bytes(),
            lifecycle: accounts[2].pubkey.to_bytes(),
            source_material: key(101).to_bytes(),
            provider_release: key(213).to_bytes(),
            update_account: accounts[1].pubkey.to_bytes(),
            provider_submitter: accounts[0].pubkey.to_bytes(),
            refund_recipient: accounts[4].pubkey.to_bytes(),
            release_set: key(210).to_bytes(),
            registry_program: accounts[8].pubkey.to_bytes(),
            encoded_vaa: accounts[32].pubkey.to_bytes(),
            post_body_digest: hash(&body).to_bytes(),
        };
        let mut data = request.to_bytes().expect("submit request").to_vec();
        data.extend_from_slice(&body);
        ProviderTransportReportV3 {
            instruction: Instruction {
                program_id: accounts[14].pubkey,
                accounts,
                data,
            },
            observation: observation(90),
            lifecycle: Pubkey::new_from_array(request.lifecycle),
            update_authority: key(4),
        }
    }

    fn reclaim_report() -> ProviderTransportReportV3 {
        let mut accounts = account_frame(PROVIDER_RECLAIM_ACCOUNT_COUNT_V3, 9);
        for index in 1..=4 {
            accounts[index].is_writable = true;
        }
        accounts[0].is_signer = true;
        let request = ProviderReclaimRequestV3 {
            generation: 7,
            terminal_sequence: 3,
            market: key(210).to_bytes(),
            source_state: key(211).to_bytes(),
            lifecycle: accounts[1].pubkey.to_bytes(),
            certificate: accounts[5].pubkey.to_bytes(),
            update_account: accounts[2].pubkey.to_bytes(),
            resolver: accounts[0].pubkey.to_bytes(),
            refund_recipient: accounts[4].pubkey.to_bytes(),
            release_set: key(212).to_bytes(),
        };
        ProviderTransportReportV3 {
            instruction: Instruction {
                program_id: accounts[9].pubkey,
                accounts,
                data: request.to_bytes().expect("reclaim request").to_vec(),
            },
            observation: observation(90),
            lifecycle: Pubkey::new_from_array(request.lifecycle),
            update_authority: key(4),
        }
    }

    fn execute_report() -> ProviderTransportReportV3 {
        let mut accounts = account_frame(PROVIDER_EXECUTE_ACCOUNT_COUNT_V3, 11);
        for index in [2, 3, 37] {
            accounts[index].is_writable = true;
        }
        accounts[1].is_signer = true;
        let core = Request::administrative(
            Action::ExecuteProvider,
            7,
            dclutch_market_core_codec::Identity::new(accounts[4].pubkey.to_bytes())
                .expect("market identity"),
        );
        let core_bytes = core.encode().expect("Core request");
        // The captured Pyth post_update instruction is 102 bytes including its
        // 8-byte discriminator. Resolution transports the exact 94-byte body.
        let body = vec![0xa5; 94];
        let request = ProviderExecutionRequestV3 {
            caller: ProviderCallerV3::Core,
            generation: 7,
            terminal_sequence: 3,
            market: accounts[4].pubkey.to_bytes(),
            source_state: accounts[2].pubkey.to_bytes(),
            certificate_account: accounts[3].pubkey.to_bytes(),
            source_material: key(101).to_bytes(),
            source_spec: key(102).to_bytes(),
            product_record: key(103).to_bytes(),
            result_domain: key(104).to_bytes(),
            provider_release: key(105).to_bytes(),
            update_account: accounts[38].pubkey.to_bytes(),
            expected_update_digest: key(107).to_bytes(),
            provider_submitter: key(108).to_bytes(),
            resolver: accounts[1].pubkey.to_bytes(),
            caller_program: accounts[11].pubkey.to_bytes(),
            release_set: key(110).to_bytes(),
            capability_program_set: [0; 32],
            selected_capability_program: [0; 32],
            parent_request_digest: hash(&core_bytes).to_bytes(),
            post_params_body_digest: hash(&body).to_bytes(),
        };
        let authority = Pubkey::find_program_address(
            &CallerAuthoritySeedsV1::from_bytes(
                request.release_set,
                request.market,
                ExecutionRoleV1::Core,
                request.source_state,
                request.parent_request_digest,
            )
            .expect("caller seeds")
            .as_slices(),
            &accounts[11].pubkey,
        )
        .0;
        accounts[0].pubkey = authority;
        let mut data = core_bytes.to_vec();
        data.extend_from_slice(&request.to_bytes().expect("provider request"));
        data.extend_from_slice(&body);
        ProviderTransportReportV3 {
            instruction: Instruction {
                program_id: accounts[11].pubkey,
                accounts,
                data,
            },
            observation: observation(90),
            lifecycle: key(38),
            update_authority: key(111),
        }
    }

    fn lookup_table(report: &ProviderTransportReportV3) -> ObservedAccount {
        let addresses = report
            .instruction
            .accounts
            .iter()
            .filter(|account| !account.is_signer)
            .map(|account| account.pubkey)
            .collect::<Vec<_>>();
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(key(220)),
                last_extended_slot: report.observation.slot - 1,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation: report.observation,
            key: key(221),
            owner: program::id(),
            lamports: 1_000_000,
            executable: false,
            data: table.serialize_for_tests().expect("lookup table bytes"),
        }
    }

    /// A payer no instruction account names. Distinct from every fixture key.
    fn stage_payer() -> Pubkey {
        key(250)
    }

    fn vacant(key: Pubkey, lamports: u64, observation: Observation) -> ObservedAccount {
        ObservedAccount {
            observation,
            key,
            owner: system_program::ID,
            lamports,
            executable: false,
            data: Vec::new(),
        }
    }

    #[test]
    fn submit_prepay_is_same_message_exact_and_refuses_stale_or_occupied_fresh_accounts() {
        let report = submit_report();
        let update = vacant(report.instruction.accounts[1].pubkey, 0, report.observation);
        let lifecycle = vacant(report.lifecycle, 400, report.observation);
        let table = lookup_table(&report);
        let plan = compile_provider_submit_with_lifecycle_prepay_v0(
            &report,
            &update,
            &lifecycle,
            1_000,
            Hash::new_from_array([7; 32]),
            core::slice::from_ref(&table),
        )
        .expect("exact submit with prepay");
        assert_eq!(plan.lifecycle_top_up_lamports, 600);
        assert_eq!(
            plan.transaction.required_signers,
            vec![update_key(&report, 0), update.key]
        );
        assert!(
            plan.transaction.message.wire_bytes
                <= dclutch_versioned_message_operator::PACKET_DATA_BYTES
        );

        let mut stale = lifecycle.clone();
        stale.observation.slot -= 1;
        assert_eq!(
            compile_provider_submit_with_lifecycle_prepay_v0(
                &report,
                &update,
                &stale,
                1_000,
                Hash::new_from_array([7; 32]),
                core::slice::from_ref(&table),
            ),
            Err(ProviderTransportTransactionErrorV3::Frame)
        );
        let occupied = ObservedAccount {
            data: vec![1],
            ..update.clone()
        };
        assert_eq!(
            compile_provider_submit_with_lifecycle_prepay_v0(
                &report,
                &occupied,
                &lifecycle,
                1_000,
                Hash::new_from_array([7; 32]),
                core::slice::from_ref(&table),
            ),
            Err(ProviderTransportTransactionErrorV3::Frame)
        );
        let overfunded = vacant(report.lifecycle, 1_001, report.observation);
        assert_eq!(
            compile_provider_submit_with_lifecycle_prepay_v0(
                &report,
                &update,
                &overfunded,
                1_000,
                Hash::new_from_array([7; 32]),
                core::slice::from_ref(&table),
            ),
            Err(ProviderTransportTransactionErrorV3::Frame)
        );
    }

    fn update_key(report: &ProviderTransportReportV3, index: usize) -> Pubkey {
        report.instruction.accounts[index].pubkey
    }

    #[test]
    fn reclaim_fits_inline_and_keeps_its_permissionless_resolver_readonly() {
        let report = reclaim_report();
        let plan =
            compile_provider_reclaim_v0(&report, Hash::new_from_array([7; 32]), &[], stage_payer())
                .expect("inline reclaim");
        assert_eq!(
            plan.required_signers,
            vec![stage_payer(), report.instruction.accounts[0].pubkey]
        );
        assert_eq!(plan.message.required_signatures, 2);
        assert_eq!(plan.message.loaded_addresses, 0);
        assert!(plan.message.wire_bytes <= dclutch_versioned_message_operator::PACKET_DATA_BYTES);
    }

    #[test]
    fn a_fee_payer_that_aliases_the_frame_refuses_before_the_cluster_sees_it() {
        // Every one of these compiled a sendable packet before wall 10, and every
        // one of them would have been refused on chain with CoreSbfError::
        // AccountFrame — the fee payer is promoted to a writable signer by the
        // message compiler, and both frames pin these accounts' privileges.
        let execute = execute_report();
        let execute_table = lookup_table(&execute);
        // Index 1 is the resolver, 2 the Source state, 5 an ordinary readonly
        // account. Each is a different privilege the fee payer would flip.
        for alias in [1_usize, 2, 5] {
            let alias = execute.instruction.accounts[alias].pubkey;
            assert_eq!(
                compile_provider_execute_v0(
                    &execute,
                    Hash::new_from_array([7; 32]),
                    core::slice::from_ref(&execute_table),
                    alias,
                ),
                Err(ProviderTransportTransactionErrorV3::Frame),
                "an Execute fee payer at {alias} aliases the frame and must refuse locally",
            );
        }
        assert_eq!(
            compile_provider_execute_v0(
                &execute,
                Hash::new_from_array([7; 32]),
                core::slice::from_ref(&execute_table),
                Pubkey::default(),
            ),
            Err(ProviderTransportTransactionErrorV3::Frame),
        );

        let reclaim = reclaim_report();
        assert!(reclaim.instruction.accounts[0].is_signer);
        assert!(!reclaim.instruction.accounts[0].is_writable);
        for alias in [0_usize, 1, 4] {
            let alias = reclaim.instruction.accounts[alias].pubkey;
            assert_eq!(
                compile_provider_reclaim_v0(&reclaim, Hash::new_from_array([7; 32]), &[], alias),
                Err(ProviderTransportTransactionErrorV3::Frame),
                "a Reclaim fee payer at {alias} aliases the frame and must refuse locally",
            );
        }
    }

    #[test]
    fn the_execute_caller_authority_is_derived_in_exactly_one_place() {
        let report = execute_report();
        let request = ProviderExecutionRequestV3::decode(
            &report.instruction.data[dclutch_market_core_codec::REQUEST_BYTES
                ..dclutch_market_core_codec::REQUEST_BYTES + PROVIDER_EXECUTION_REQUEST_BYTES_V3],
        )
        .expect("execution request");
        let (core_bytes, caller_authority) = provider_execute_caller_authority_v3(
            request.release_set,
            Pubkey::new_from_array(request.market),
            Identity::new(request.market).expect("market identity"),
            request.generation,
            Pubkey::new_from_array(request.source_state),
            Pubkey::new_from_array(request.caller_program),
        )
        .expect("caller authority");
        assert_eq!(
            caller_authority, report.instruction.accounts[0].pubkey,
            "the shared derivation must reproduce the builder's own index-0 account",
        );
        assert_eq!(
            hash(&core_bytes).to_bytes(),
            request.parent_request_digest,
            "and the identical parent-request bytes the instruction carries",
        );
    }

    #[test]
    fn submission_requires_routing_and_reports_both_real_signers() {
        let report = submit_report();
        let request = ProviderSubmitRequestV3::decode(
            &report.instruction.data[..PROVIDER_SUBMIT_REQUEST_BYTES_V3],
        )
        .expect("submit request");
        assert_ne!(
            report.instruction.accounts[17].pubkey.to_bytes(),
            request.source_material,
            "SourceMaterial content digest must not be confused with its finalized-record PDA",
        );
        assert_ne!(
            report.instruction.accounts[23].pubkey.to_bytes(),
            request.provider_release,
            "content digest must not be confused with its finalized-record PDA",
        );
        assert_eq!(
            compile_provider_submit_v0(&report, Hash::new_from_array([7; 32]), &[]),
            Err(ProviderTransportTransactionErrorV3::Routing(
                dclutch_versioned_message_operator::Error::PacketTooLarge,
            ))
        );
        let table = lookup_table(&report);
        let plan = compile_provider_submit_v0(
            &report,
            Hash::new_from_array([7; 32]),
            core::slice::from_ref(&table),
        )
        .expect("table-routed submission");
        assert_eq!(
            plan.required_signers,
            vec![
                report.instruction.accounts[0].pubkey,
                report.instruction.accounts[1].pubkey,
            ]
        );
        assert_eq!(plan.message.required_signatures, 2);
        assert!(plan.message.loaded_addresses > 0);
        assert!(plan.message.wire_bytes <= dclutch_versioned_message_operator::PACKET_DATA_BYTES);
    }

    #[test]
    fn core_execution_requires_routing_and_a_payer_beside_its_readonly_resolver() {
        let report = execute_report();
        assert_eq!(PROVIDER_EXECUTE_ACCOUNT_COUNT_V3, 47);
        assert_eq!(
            report.instruction.accounts.len(),
            PROVIDER_EXECUTE_ACCOUNT_COUNT_V3
        );
        assert!(distinct(&report.instruction.accounts));
        assert_eq!(dclutch_market_core_codec::REQUEST_BYTES, 72);
        assert_eq!(PROVIDER_EXECUTION_REQUEST_BYTES_V3, 608);
        assert_eq!(report.instruction.data.len(), 774);
        assert_eq!(
            compile_provider_execute_v0(&report, Hash::new_from_array([7; 32]), &[], stage_payer()),
            Err(ProviderTransportTransactionErrorV3::Routing(
                dclutch_versioned_message_operator::Error::PacketTooLarge,
            ))
        );
        let table = lookup_table(&report);
        let plan = compile_provider_execute_v0(
            &report,
            Hash::new_from_array([7; 32]),
            core::slice::from_ref(&table),
            stage_payer(),
        )
        .expect("table-routed provider execution");
        assert_eq!(
            plan.required_signers,
            vec![stage_payer(), report.instruction.accounts[1].pubkey]
        );
        assert_eq!(plan.message.required_signatures, 2);
        assert_eq!(plan.message.loaded_addresses, 45);
        // A payer distinct from the resolver costs exactly 96 bytes: 64 for the
        // second signature and 32 for its static key. That is the whole price of
        // keeping the resolver readonly, and it is what §7.13 measured.
        assert_eq!(plan.message.wire_bytes, 1_072 + 96);
        assert_eq!(
            dclutch_versioned_message_operator::PACKET_DATA_BYTES - plan.message.wire_bytes,
            64,
        );
    }

    #[test]
    fn privilege_mutation_and_stale_tables_refuse() {
        let mut submit = submit_report();
        submit.instruction.accounts[1].is_signer = false;
        assert_eq!(
            compile_provider_submit_v0(&submit, Hash::new_from_array([7; 32]), &[]),
            Err(ProviderTransportTransactionErrorV3::Frame)
        );

        let reclaim = reclaim_report();
        let mut stale = lookup_table(&reclaim);
        stale.observation.slot -= 1;
        assert_eq!(
            compile_provider_reclaim_v0(
                &reclaim,
                Hash::new_from_array([7; 32]),
                core::slice::from_ref(&stale),
                stage_payer(),
            ),
            Err(ProviderTransportTransactionErrorV3::Routing(
                dclutch_versioned_message_operator::Error::ObservationMismatch,
            ))
        );

        let mut execute = execute_report();
        execute.instruction.accounts[0].is_signer = true;
        assert_eq!(
            compile_provider_execute_v0(
                &execute,
                Hash::new_from_array([7; 32]),
                &[],
                stage_payer()
            ),
            Err(ProviderTransportTransactionErrorV3::Frame)
        );

        let mut direct_resolution = execute_report();
        direct_resolution.instruction.program_id =
            direct_resolution.instruction.accounts[15].pubkey;
        assert_eq!(
            compile_provider_execute_v0(
                &direct_resolution,
                Hash::new_from_array([7; 32]),
                &[],
                stage_payer(),
            ),
            Err(ProviderTransportTransactionErrorV3::Frame),
            "the unsigned caller PDA cannot bypass the Core wrapper through a top-level Resolution instruction",
        );
    }
}
