//! Chain-derived construction for real Pyth Receiver submission and reclaim.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use dclutch_market_core_codec::CoreState;
use dclutch_pyth_svm::{PostUpdateParamsView, PythReleaseV1, VerifiedEncodedVaaV1};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::ACTIVATION_PDA_DOMAIN_V1;
use dclutch_release_set_contract::PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1;
use dclutch_resolution_codec::{
    PROVIDER_RECLAIM_REQUEST_BYTES_V3, PROVIDER_SUBMIT_REQUEST_BYTES_V3,
    PROVIDER_UPDATE_AUTHORITY_PDA_DOMAIN_V3, PROVIDER_UPDATE_LIFECYCLE_PDA_DOMAIN_V3,
    PYTH_RELEASE_RECORD_SCHEMA_ID_V1, ProviderReclaimRequestV3, ProviderSubmitRequestV3,
    ProviderUpdateLifecycleV3, ProviderUpdateStatusV3,
};
use dclutch_source_contract::{
    PROVIDER_RELEASE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SOURCE_SPEC_SCHEMA_ID_V1,
    SourceMaterialV2, SourceResolutionPhaseV1, SourceResolutionStateV2, SourceSpecV1,
    WINDOW_SPEC_SCHEMA_ID_V1, WindowSpecV1,
};
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

pub use dclutch_resolution_core_v3_operator::{Finality, Observation, ObservedAccount};
use dclutch_versioned_message_operator::{
    VersionedMessagePlanV0, compile_v0_message_with_optional_tables,
};

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
    /// Finalized observation from which every semantic account was derived.
    pub observation: Observation,
    /// Resolution-owned lifecycle PDA.
    pub lifecycle: Pubkey,
    /// Resolution-owned Receiver write-authority PDA.
    pub update_authority: Pubkey,
}

/// Unsigned packet-safe provider transaction and its exact signing boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderTransportTransactionPlanV3 {
    /// Exact unsigned v0 message and packet geometry.
    pub message: VersionedMessagePlanV0,
    /// Canonical signer order required by the message.
    pub required_signers: Vec<Pubkey>,
}

/// Refusal from a mutated provider report or unsafe transaction routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderTransportTransactionErrorV3 {
    /// The instruction no longer had the frozen discriminator, account order,
    /// privilege profile, or request-to-account joins.
    Frame,
    /// Finalized lookup-table selection or signed packet geometry refused.
    Routing(dclutch_versioned_message_operator::Error),
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
        observation,
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

/// Compile one exact permissionless reclaim into an unsigned v0 message.
///
/// The permissionless resolver is both fee payer and sole transaction signer.
/// The Resolution update-authority PDA signs only the Receiver CPI onchain.
pub fn compile_provider_reclaim_v0(
    report: &ProviderTransportReportV3,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
) -> Result<ProviderTransportTransactionPlanV3, ProviderTransportTransactionErrorV3> {
    let request = validate_reclaim_report(report)?;
    let required_signers = vec![Pubkey::new_from_array(request.resolver)];
    compile_provider_v0(report, recent_blockhash, lookup_tables, required_signers)
}

fn compile_provider_v0(
    report: &ProviderTransportReportV3,
    recent_blockhash: Hash,
    lookup_tables: &[ObservedAccount],
    required_signers: Vec<Pubkey>,
) -> Result<ProviderTransportTransactionPlanV3, ProviderTransportTransactionErrorV3> {
    let payer = *required_signers
        .first()
        .ok_or(ProviderTransportTransactionErrorV3::Frame)?;
    let message = compile_v0_message_with_optional_tables(
        payer,
        core::slice::from_ref(&report.instruction),
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
        || account_key(accounts, 17)?.to_bytes() != request.source_material
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

fn account_key(
    accounts: &[AccountMeta],
    index: usize,
) -> Result<Pubkey, ProviderTransportTransactionErrorV3> {
    accounts
        .get(index)
        .map(|account| account.pubkey)
        .ok_or(ProviderTransportTransactionErrorV3::Frame)
}

fn exact_submit_privileges(accounts: &[AccountMeta]) -> bool {
    accounts.iter().enumerate().all(|(index, account)| {
        let expected_signer = matches!(index, 0 | 1);
        let expected_writable = matches!(index, 0 | 1 | 2 | 34);
        account.is_signer == expected_signer && account.is_writable == expected_writable
    })
}

fn exact_reclaim_privileges(accounts: &[AccountMeta]) -> bool {
    accounts.iter().enumerate().all(|(index, account)| {
        let expected_signer = index == 0;
        let expected_writable = matches!(index, 1..=4);
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

#[cfg(test)]
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
            source_material: accounts[17].pubkey.to_bytes(),
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

    #[test]
    fn reclaim_fits_inline_and_reports_only_permissionless_resolver() {
        let report = reclaim_report();
        let plan = compile_provider_reclaim_v0(&report, Hash::new_from_array([7; 32]), &[])
            .expect("inline reclaim");
        assert_eq!(
            plan.required_signers,
            vec![report.instruction.accounts[0].pubkey]
        );
        assert_eq!(plan.message.required_signatures, 1);
        assert_eq!(plan.message.loaded_addresses, 0);
        assert!(plan.message.wire_bytes <= dclutch_versioned_message_operator::PACKET_DATA_BYTES);
    }

    #[test]
    fn submission_requires_routing_and_reports_both_real_signers() {
        let report = submit_report();
        let request = ProviderSubmitRequestV3::decode(
            &report.instruction.data[..PROVIDER_SUBMIT_REQUEST_BYTES_V3],
        )
        .expect("submit request");
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
            ),
            Err(ProviderTransportTransactionErrorV3::Routing(
                dclutch_versioned_message_operator::Error::ObservationMismatch,
            ))
        );
    }
}
