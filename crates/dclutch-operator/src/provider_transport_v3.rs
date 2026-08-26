//! Chain-derived construction for real Pyth Receiver submission and reclaim.

use dclutch_market_core_codec::CoreState;
use dclutch_pyth_svm::{PostUpdateParamsView, PythReleaseV1, VerifiedEncodedVaaV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::ACTIVATION_PDA_DOMAIN_V1;
use dclutch_release_set_contract::PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1;
use dclutch_resolution_codec::{
    PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
    PYTH_RELEASE_RECORD_SCHEMA_ID_V1, ProviderReclaimRequestV3, ProviderSubmitRequestV3,
    ProviderUpdateLifecycleV3, ProviderUpdateStatusV3,
};
use dclutch_source_contract::{
    PROVIDER_RELEASE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SOURCE_SPEC_SCHEMA_ID_V1,
    SourceMaterialV2, SourceResolutionPhaseV1, SourceResolutionStateV2, SourceSpecV1,
    WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::ObservedAccount;

/// Resolution submission account count frozen by the physical adapter.
pub const PROVIDER_SUBMIT_ACCOUNT_COUNT_V3: usize = 38;
/// Resolution reclaim account count frozen by the physical adapter.
pub const PROVIDER_RECLAIM_ACCOUNT_COUNT_V3: usize = 18;

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
    /// Finalized SourceMaterialV2 raw record.
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

/// Constructed unsigned provider instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderTransportReportV3 {
    /// Exact unsigned instruction.
    pub instruction: Instruction,
    /// Resolution-owned lifecycle PDA.
    pub lifecycle: Pubkey,
    /// Resolution-owned Receiver write-authority PDA.
    pub update_authority: Pubkey,
}

/// Build one exact real-provider submission from same-snapshot chain state.
pub fn build_provider_submit_v3(
    snapshot: &ProviderSubmitSnapshotV3,
    deployment: ProviderSubmitDeploymentV3,
    intent: &ProviderSubmitIntentV3,
) -> Result<ProviderTransportReportV3, ProviderTransportOperatorErrorV3> {
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
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        source.material_id().to_bytes(),
    )?;
    let material = SourceMaterialV2::decode(&snapshot.source_material.data)
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
    let (lifecycle, _) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
            intent.update_account.as_ref(),
        ],
        &deployment.resolution_program,
    );
    let (update_authority, _) = Pubkey::find_program_address(
        &[
            PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3,
            snapshot.market.key.as_ref(),
            snapshot.source_state.key.as_ref(),
            intent.update_account.as_ref(),
        ],
        &deployment.resolution_program,
    );
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
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
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
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
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
        lifecycle,
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
        lifecycle: lifecycle_account.key,
        update_authority,
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

fn distinct(accounts: &[AccountMeta]) -> bool {
    accounts.iter().enumerate().all(|(index, account)| {
        accounts
            .iter()
            .skip(index + 1)
            .all(|other| other.pubkey != account.pubkey)
    })
}
