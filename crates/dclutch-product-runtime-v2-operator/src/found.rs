//! Successor-only chain-derived Core Found construction.
//!
//! This builder accepts one finalized account snapshot, independently
//! authenticates every immutable record coordinate and cross-record join, and
//! emits the exact unsigned 31-account Core Found instruction. It performs no
//! RPC, signing, submission, funding, or account mutation.

use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_market_core_codec::{
    Action, Identity, MarketCoreStateSeedsV2, MarketIdentity, REQUEST_BYTES, Request, STATE_BYTES,
};
use dclutch_product_runtime_v2_admission::{
    AdmissionProjectionV2, AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2, admit_authenticated_records_v2,
};
use dclutch_realm_contract::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
};
use dclutch_registry_svm::{ProgramDataV3View, ProgramV3View};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ExecutionRoleV1, PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1, ProtocolInfrastructureProfileV1,
};
use dclutch_rent_contract::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleRentCreditV2,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SourceMaterialV2,
};
use solana_program::{
    account_info::AccountInfo,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use crate::{
    AccountObservationV2, Error, FinalizedRecordObservationV2, Result, coordinate, digest,
};

/// Exact number of accounts in the Runtime V2 Core Found frame.
pub const FOUND_ACCOUNT_COUNT_V2: usize = 31;

/// One non-Product finalized raw/staging record observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedReferenceObservationV2<'a> {
    /// Exact schema/validator identity selecting the Registry PDA domain.
    pub schema_id: [u8; 32],
    /// Registry-owned raw and System-owned vacant staging observations.
    pub record: FinalizedRecordObservationV2<'a>,
}

/// One finalized pre-credit snapshot sufficient to derive the sole Market and
/// lifecycle-credit coordinates for Runtime V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundProjectionStateV2<'a> {
    /// System-owned signing payer and Market rent sponsor.
    pub payer: AccountObservationV2<'a>,
    /// System-owned empty exact Market PDA destination.
    pub market: AccountObservationV2<'a>,
    /// Exact executable Rent program selected by the immutable profile.
    pub rent_program: AccountObservationV2<'a>,
    /// Finalized Realm raw/staging pair.
    pub realm: FinalizedReferenceObservationV2<'a>,
    /// Finalized Runtime V2 Product raw/staging pair.
    pub product: FinalizedRecordObservationV2<'a>,
    /// Finalized Runtime V2 result-domain raw/staging pair.
    pub result_domain: FinalizedRecordObservationV2<'a>,
    /// Finalized Runtime V2 portfolio raw/staging pair.
    pub portfolio: FinalizedRecordObservationV2<'a>,
    /// Finalized SourceMaterialV2 raw/staging pair.
    pub source_material: FinalizedReferenceObservationV2<'a>,
    /// Finalized capability-manifest raw/staging pair.
    pub capability_manifest: FinalizedReferenceObservationV2<'a>,
    /// Finalized execution-release-set raw/staging pair.
    pub execution_release_set: FinalizedReferenceObservationV2<'a>,
    /// Registry-owned activated release-set cache.
    pub activation_cache: AccountObservationV2<'a>,
    /// Exact executable Core program selected for Found.
    pub core_program: AccountObservationV2<'a>,
    /// Current Core ProgramData account named by the activated release.
    pub core_programdata: AccountObservationV2<'a>,
    /// Exact executable Registry program selected by the immutable profile.
    pub registry_program: AccountObservationV2<'a>,
    /// Canonical Rent sysvar observation.
    pub rent: AccountObservationV2<'a>,
    /// Canonical executable System Program observation.
    pub system_program: AccountObservationV2<'a>,
    /// Immutable per-Core Registry/Rent selection PDA.
    pub infrastructure_profile: AccountObservationV2<'a>,
    /// Finalized immutable Registry artifact release.
    pub registry_artifact: FinalizedRecordObservationV2<'a>,
    /// Current Registry ProgramData observation.
    pub registry_programdata: AccountObservationV2<'a>,
    /// Finalized immutable Rent artifact release.
    pub rent_artifact: FinalizedRecordObservationV2<'a>,
    /// Current Rent ProgramData observation.
    pub rent_programdata: AccountObservationV2<'a>,
}

/// One finalized post-credit snapshot sufficient to construct Core Found for Runtime V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundStateV2<'a> {
    /// System-owned signing payer and Market rent sponsor.
    pub payer: AccountObservationV2<'a>,
    /// System-owned empty exact Market PDA destination.
    pub market: AccountObservationV2<'a>,
    /// Existing lifecycle RentCredit bound to this Market generation.
    pub rent_credit: AccountObservationV2<'a>,
    /// Exact executable Rent program selected by the immutable profile.
    pub rent_program: AccountObservationV2<'a>,
    /// Finalized Realm raw/staging pair.
    pub realm: FinalizedReferenceObservationV2<'a>,
    /// Finalized Runtime V2 Product raw/staging pair.
    pub product: FinalizedRecordObservationV2<'a>,
    /// Finalized Runtime V2 result-domain raw/staging pair.
    pub result_domain: FinalizedRecordObservationV2<'a>,
    /// Finalized Runtime V2 portfolio raw/staging pair.
    pub portfolio: FinalizedRecordObservationV2<'a>,
    /// Finalized SourceMaterialV2 raw/staging pair.
    pub source_material: FinalizedReferenceObservationV2<'a>,
    /// Finalized capability-manifest raw/staging pair.
    pub capability_manifest: FinalizedReferenceObservationV2<'a>,
    /// Finalized execution-release-set raw/staging pair.
    pub execution_release_set: FinalizedReferenceObservationV2<'a>,
    /// Registry-owned activated release-set cache.
    pub activation_cache: AccountObservationV2<'a>,
    /// Exact executable Core program selected for Found.
    pub core_program: AccountObservationV2<'a>,
    /// Current Core ProgramData account named by the activated release.
    pub core_programdata: AccountObservationV2<'a>,
    /// Exact executable Registry program selected by the immutable profile.
    pub registry_program: AccountObservationV2<'a>,
    /// Canonical Rent sysvar observation.
    pub rent: AccountObservationV2<'a>,
    /// Canonical executable System Program observation.
    pub system_program: AccountObservationV2<'a>,
    /// Immutable per-Core Registry/Rent selection PDA.
    pub infrastructure_profile: AccountObservationV2<'a>,
    /// Finalized immutable Registry artifact release.
    pub registry_artifact: FinalizedRecordObservationV2<'a>,
    /// Current Registry ProgramData observation.
    pub registry_programdata: AccountObservationV2<'a>,
    /// Finalized immutable Rent artifact release.
    pub rent_artifact: FinalizedRecordObservationV2<'a>,
    /// Current Rent ProgramData observation.
    pub rent_programdata: AccountObservationV2<'a>,
}

impl<'a> FoundStateV2<'a> {
    /// Remove only the lifecycle-credit observation, retaining the complete
    /// same-slot authority needed to derive its exact pre-creation coordinate.
    pub fn projection_state(self) -> FoundProjectionStateV2<'a> {
        FoundProjectionStateV2 {
            payer: self.payer,
            market: self.market,
            rent_program: self.rent_program,
            realm: self.realm,
            product: self.product,
            result_domain: self.result_domain,
            portfolio: self.portfolio,
            source_material: self.source_material,
            capability_manifest: self.capability_manifest,
            execution_release_set: self.execution_release_set,
            activation_cache: self.activation_cache,
            core_program: self.core_program,
            core_programdata: self.core_programdata,
            registry_program: self.registry_program,
            rent: self.rent,
            system_program: self.system_program,
            infrastructure_profile: self.infrastructure_profile,
            registry_artifact: self.registry_artifact,
            registry_programdata: self.registry_programdata,
            rent_artifact: self.rent_artifact,
            rent_programdata: self.rent_programdata,
        }
    }
}

/// Authenticated pre-credit Found projection. This is not an executable Found
/// instruction: it exists solely to select the exact Market, generation,
/// release set, and lifecycle-credit PDA before the credit account exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundProjectionV2 {
    /// Sole Market address derived from the authenticated snapshot.
    pub market_address: Pubkey,
    /// Market identity reconstructed from the exact selected Registry.
    pub market_identity: MarketIdentity,
    /// Product graph projection authenticated under the selected Registry.
    pub product: AdmissionProjectionV2,
    /// Runtime native outcome width, including explicit failure.
    pub outcome_count: u32,
    /// Shared finalized observation slot.
    pub observation_slot: u64,
    /// Exact Market rent top-up required from the payer.
    pub market_rent_top_up: u64,
}

/// Exact chain-derived Runtime V2 Core Found plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundInstructionPlanV2 {
    /// Exact unsigned 31-account instruction.
    pub instruction: Instruction,
    /// Sole Market address derived from the authenticated snapshot.
    pub market_address: Pubkey,
    /// Market identity reconstructed from the exact selected Registry.
    pub market_identity: MarketIdentity,
    /// Product graph projection authenticated under the selected Registry.
    pub product: AdmissionProjectionV2,
    /// Runtime native outcome width, including explicit failure.
    pub outcome_count: u32,
    /// Shared finalized observation slot.
    pub observation_slot: u64,
    /// Exact Market rent top-up required from the payer.
    pub market_rent_top_up: u64,
}

/// Reauthenticate one finalized Runtime V2 snapshot and construct Core Found.
pub fn build_found_instruction_v2(
    generation: u64,
    state: FoundStateV2<'_>,
) -> Result<FoundInstructionPlanV2> {
    let projection = project_found_v2(generation, state.projection_state())?;
    if state.rent_credit.slot != projection.observation_slot {
        return Err(Error::ObservationMismatch);
    }
    if projection_accounts(state.projection_state())
        .iter()
        .any(|account| account.key == state.rent_credit.key)
    {
        return Err(Error::AccountAuthority);
    }
    authenticate_rent_credit(
        state.rent_program,
        state.rent_credit,
        projection.market_address,
        generation,
        projection.market_identity.selected_release_set.to_bytes(),
    )?;
    let request = Request::administrative(
        Action::Found,
        generation,
        projection.market_identity.market_id,
    )
    .encode()
    .map_err(|_| Error::InvalidRecord)?;
    if request.len() != REQUEST_BYTES {
        return Err(Error::InvalidRecord);
    }
    let accounts = found_metas(state);
    if accounts.len() != FOUND_ACCOUNT_COUNT_V2 {
        return Err(Error::AccountAuthority);
    }
    Ok(FoundInstructionPlanV2 {
        instruction: Instruction {
            program_id: state.core_program.key,
            accounts,
            data: request.to_vec(),
        },
        market_address: projection.market_address,
        market_identity: projection.market_identity,
        outcome_count: projection.outcome_count,
        product: projection.product,
        observation_slot: projection.observation_slot,
        market_rent_top_up: projection.market_rent_top_up,
    })
}

/// Authenticate the complete immutable Found authority and derive the exact
/// pre-credit Market projection. The projection cannot be submitted as Found;
/// callers must first create and reacquire its lifecycle credit, then call
/// [`build_found_instruction_v2`] with the real post-create observation.
pub fn project_found_v2(
    generation: u64,
    state: FoundProjectionStateV2<'_>,
) -> Result<FoundProjectionV2> {
    let slot = require_one_slot(state)?;
    authenticate_runtime_accounts(state)?;
    let rent = decode_rent(state.rent)?;
    authenticate_record_rent_minima(state, &rent)?;
    let realm_digest = authenticate_reference(
        state.registry_program.key,
        state.realm,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    let realm = RealmV1::decode(state.realm.record.raw.data).map_err(|_| Error::InvalidRecord)?;
    if realm.to_bytes().as_slice() != state.realm.record.raw.data {
        return Err(Error::InvalidRecord);
    }

    let product_coordinate = authenticate_product_record(
        state.registry_program.key,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        state.product,
    )?;
    let domain_coordinate = authenticate_product_record(
        state.registry_program.key,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        state.result_domain,
    )?;
    let portfolio_coordinate = authenticate_product_record(
        state.registry_program.key,
        PORTFOLIO_SCHEMA_ID_V2,
        state.portfolio,
    )?;
    let receipt = AdmissionReceiptV2 {
        product: product_coordinate,
        result_domain: domain_coordinate,
        portfolio: portfolio_coordinate,
    };
    let product = admit_authenticated_records_v2(
        receipt,
        state.product.raw.data,
        state.result_domain.raw.data,
        state.portfolio.raw.data,
    )
    .map_err(|_| Error::InvalidRecord)?;

    let source_digest = authenticate_reference(
        state.registry_program.key,
        state.source_material,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
    )?;
    SourceMaterialV2::decode(state.source_material.record.raw.data)
        .and_then(|material| {
            material.authenticate_product_record(SourceContentId::new(
                product.product_record_digest.to_bytes(),
            )?)
        })
        .map_err(|_| Error::CrossRecordMismatch)?;

    let manifest_digest = authenticate_reference(
        state.registry_program.key,
        state.capability_manifest,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    CapabilityManifestV1::decode(state.capability_manifest.record.raw.data)
        .map_err(|_| Error::InvalidRecord)?;

    let release_set_digest = authenticate_reference(
        state.registry_program.key,
        state.execution_release_set,
        EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
    )?;
    let release_set = ExecutionReleaseSetV1::decode(state.execution_release_set.record.raw.data)
        .map_err(|_| Error::InvalidRecord)?;
    if release_set.to_bytes().as_slice() != state.execution_release_set.record.raw.data {
        return Err(Error::InvalidRecord);
    }
    authenticate_activation(state, release_set_digest.to_bytes(), release_set)?;
    authenticate_infrastructure(state, &rent)?;

    let mut market_identity = MarketIdentity {
        market_id: identity(state.market.key.to_bytes())?,
        realm_id: identity(realm_digest.to_bytes())?,
        product_record: identity(product.product_record_digest.to_bytes())?,
        product_id: identity(product.join.product_id.to_bytes())?,
        resolution_policy: identity(source_digest.to_bytes())?,
        capability_manifest: identity(manifest_digest.to_bytes())?,
        selected_release_set: identity(release_set_digest.to_bytes())?,
        registry_program: identity(state.registry_program.key.to_bytes())?,
        generation,
    };
    let market_address = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
        &state.core_program.key,
    )
    .0;
    if market_address != state.market.key {
        return Err(Error::AccountAuthority);
    }
    market_identity.market_id = identity(market_address.to_bytes())?;
    let market_rent_minimum = rent.minimum_balance(STATE_BYTES);
    let market_rent_top_up = market_rent_minimum.saturating_sub(state.market.lamports);
    if state.payer.lamports < market_rent_top_up {
        return Err(Error::InsufficientPayer);
    }
    Ok(FoundProjectionV2 {
        market_address,
        market_identity,
        outcome_count: product.join.outcome_count,
        product,
        observation_slot: slot,
        market_rent_top_up,
    })
}

fn require_one_slot(state: FoundProjectionStateV2<'_>) -> Result<u64> {
    let slot = state.registry_program.slot;
    if projection_accounts(state)
        .iter()
        .any(|account| account.slot != slot)
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(slot)
}

fn authenticate_runtime_accounts(state: FoundProjectionStateV2<'_>) -> Result<()> {
    let keys: Vec<Pubkey> = projection_accounts(state)
        .iter()
        .map(|account| account.key)
        .collect();
    for (index, key) in keys.iter().enumerate() {
        if keys
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other == key)
        {
            return Err(Error::AccountAuthority);
        }
    }
    if state.payer.owner != system_program::ID
        || state.payer.executable
        || !state.payer.data.is_empty()
        || state.market.owner != system_program::ID
        || state.market.executable
        || !state.market.data.is_empty()
        || !state.rent_program.executable
        || !state.registry_program.executable
        || !state.core_program.executable
        || state.core_programdata.executable
        || state.activation_cache.executable
        || state.infrastructure_profile.owner != state.core_program.key
        || state.infrastructure_profile.executable
        || state.infrastructure_profile.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
        || state.registry_programdata.executable
        || state.rent_programdata.executable
        || state.system_program.key != system_program::ID
        || state.system_program.owner != native_loader::ID
        || !state.system_program.executable
        || !state.system_program.data.is_empty()
    {
        return Err(Error::AccountAuthority);
    }
    Ok(())
}

fn authenticate_record_rent_minima(state: FoundProjectionStateV2<'_>, rent: &Rent) -> Result<()> {
    let records = [
        state.realm.record,
        state.product,
        state.result_domain,
        state.portfolio,
        state.source_material.record,
        state.capability_manifest.record,
        state.execution_release_set.record,
        state.registry_artifact,
        state.rent_artifact,
    ];
    if records
        .iter()
        .any(|record| record.raw_rent_minimum != rent.minimum_balance(record.raw.data.len()))
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(())
}

fn authenticate_product_record(
    registry: Pubkey,
    schema: [u8; 32],
    observation: FinalizedRecordObservationV2<'_>,
) -> Result<FinalizedRecordCoordinateV2> {
    let content_digest = digest(observation.raw.data)?;
    let coordinate = coordinate(registry, schema, content_digest)?;
    super::validate_record(registry, coordinate, observation)?;
    Ok(coordinate)
}

fn authenticate_reference(
    registry: Pubkey,
    reference: FinalizedReferenceObservationV2<'_>,
    expected_schema: [u8; 32],
) -> Result<dclutch_product_runtime_v2::ContentId> {
    if reference.schema_id != expected_schema {
        return Err(Error::AccountAuthority);
    }
    let coordinate = authenticate_product_record(registry, expected_schema, reference.record)?;
    Ok(coordinate.content_digest)
}

fn authenticate_activation(
    state: FoundProjectionStateV2<'_>,
    release_set_digest: [u8; 32],
    release_set: ExecutionReleaseSetV1,
) -> Result<()> {
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set_digest],
        &state.registry_program.key,
    )
    .0;
    if state.activation_cache.key != expected_cache
        || state.activation_cache.owner != state.registry_program.key
    {
        return Err(Error::AccountAuthority);
    }
    let activated = ActivatedExecutionReleaseSetViewV1::decode(state.activation_cache.data)
        .map_err(|_| Error::InvalidRecord)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| Error::InvalidRecord)?
        .to_bytes()
        != release_set_digest
        || activated
            .release_set_projection()
            .map_err(|_| Error::InvalidRecord)?
            != release_set
    {
        return Err(Error::CrossRecordMismatch);
    }
    let core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|_| Error::InvalidRecord)?;
    let release = core.release();
    let binding = release_set.binding(ExecutionRoleV1::Core);
    if core.artifact_release_id() != binding.artifact_release()
        || release.program().to_bytes() != state.core_program.key.to_bytes()
        || binding.program().to_bytes() != state.core_program.key.to_bytes()
        || release.programdata() != state.core_programdata.key.to_bytes()
        || release.loader_program().to_bytes() != state.core_program.owner.to_bytes()
        || state.core_programdata.owner != state.core_program.owner
    {
        return Err(Error::CrossRecordMismatch);
    }
    authenticate_current_deployment(state.core_program, state.core_programdata, release, true)?;
    Ok(())
}

fn authenticate_infrastructure(state: FoundProjectionStateV2<'_>, rent: &Rent) -> Result<()> {
    let expected_profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V1],
        &state.core_program.key,
    )
    .0;
    if state.infrastructure_profile.key != expected_profile
        || state.infrastructure_profile.lamports
            < rent.minimum_balance(PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1)
    {
        return Err(Error::AccountAuthority);
    }
    let profile = ProtocolInfrastructureProfileV1::decode(state.infrastructure_profile.data)
        .map_err(|_| Error::InvalidRecord)?;
    if profile.registry().program().to_bytes() != state.registry_program.key.to_bytes()
        || profile.rent().program().to_bytes() != state.rent_program.key.to_bytes()
    {
        return Err(Error::CrossRecordMismatch);
    }
    let registry = authenticate_artifact(
        state.registry_program.key,
        state.registry_artifact,
        state.registry_program,
        state.registry_programdata,
    )?;
    let rent_program = authenticate_artifact(
        state.registry_program.key,
        state.rent_artifact,
        state.rent_program,
        state.rent_programdata,
    )?;
    if registry != profile.registry() || rent_program != profile.rent() {
        return Err(Error::CrossRecordMismatch);
    }
    Ok(())
}

fn authenticate_artifact(
    registry: Pubkey,
    observation: FinalizedRecordObservationV2<'_>,
    program: AccountObservationV2<'_>,
    programdata: AccountObservationV2<'_>,
) -> Result<ExecutionRoleBindingV1> {
    let coordinate =
        authenticate_product_record(registry, ARTIFACT_RELEASE_SCHEMA_ID_V1, observation)?;
    let release =
        ArtifactReleaseV1::decode(observation.raw.data).map_err(|_| Error::InvalidRecord)?;
    if release.program().to_bytes() != program.key.to_bytes()
        || release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable
    {
        return Err(Error::CrossRecordMismatch);
    }
    authenticate_current_deployment(program, programdata, release, true)?;
    let artifact_release = ArtifactReleaseIdV1::new(coordinate.content_digest.to_bytes())
        .map_err(|_| Error::InvalidRecord)?;
    Ok(ExecutionRoleBindingV1::new(
        release.program(),
        artifact_release,
    ))
}

fn authenticate_current_deployment(
    program: AccountObservationV2<'_>,
    programdata: AccountObservationV2<'_>,
    release: ArtifactReleaseV1,
    require_immutable: bool,
) -> Result<()> {
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.program().to_bytes() != program.key.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
        || (require_immutable && release.upgrade_policy() != ArtifactUpgradePolicyV1::Immutable)
    {
        return Err(Error::AccountAuthority);
    }
    let program_view = ProgramV3View::parse(program.data).map_err(|_| Error::InvalidRecord)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != release.programdata()
        || programdata.key != expected_programdata
    {
        return Err(Error::CrossRecordMismatch);
    }
    let programdata_view =
        ProgramDataV3View::parse(programdata.data).map_err(|_| Error::InvalidRecord)?;
    let deployment = DeploymentObservationV1::new(
        program.key.to_bytes(),
        program.owner.to_bytes(),
        program.executable,
        programdata.key.to_bytes(),
        programdata.owner.to_bytes(),
        programdata.executable,
        program_view.programdata(),
        bpf_loader_upgradeable::ID.to_bytes(),
        programdata_view.deployment_slot(),
        hash(programdata_view.elf()).to_bytes(),
        programdata_view.upgrade_authority(),
    )
    .map_err(|_| Error::InvalidRecord)?;
    release
        .authenticate_deployment(deployment)
        .map_err(|_| Error::CrossRecordMismatch)
}

fn authenticate_rent_credit(
    rent_program: AccountObservationV2<'_>,
    rent_credit: AccountObservationV2<'_>,
    market_address: Pubkey,
    generation: u64,
    release_set: [u8; 32],
) -> Result<()> {
    if rent_credit.owner != rent_program.key || rent_credit.executable {
        return Err(Error::AccountAuthority);
    }
    let credit =
        LifecycleRentCreditV2::decode(rent_credit.data).map_err(|_| Error::InvalidRecord)?;
    if credit.market().to_bytes() != market_address.to_bytes()
        || credit.release_set().to_bytes() != release_set
        || credit.generation() != generation
    {
        return Err(Error::CrossRecordMismatch);
    }
    let market = credit.market().to_bytes();
    let generation_bytes = credit.generation().to_le_bytes();
    let bump = [credit.pda_bump()];
    let expected = Pubkey::create_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            &market,
            &generation_bytes,
            &bump,
        ],
        &rent_program.key,
    )
    .map_err(|_| Error::AccountAuthority)?;
    if expected != rent_credit.key {
        return Err(Error::AccountAuthority);
    }
    Ok(())
}

fn decode_rent(account: AccountObservationV2<'_>) -> Result<Rent> {
    if account.key != sysvar::rent::ID
        || account.owner != sysvar::ID
        || account.executable
        || account.data.len() != Rent::size_of()
    {
        return Err(Error::AccountAuthority);
    }
    let mut lamports = account.lamports;
    let mut data = account.data.to_vec();
    let info = AccountInfo::new(
        &account.key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        false,
    );
    Rent::from_account_info(&info).map_err(|_| Error::AccountAuthority)
}

fn found_metas(state: FoundStateV2<'_>) -> Vec<AccountMeta> {
    vec![
        AccountMeta::new(state.payer.key, true),
        AccountMeta::new(state.market.key, false),
        AccountMeta::new_readonly(state.rent_credit.key, false),
        AccountMeta::new_readonly(state.rent_program.key, false),
        AccountMeta::new_readonly(state.realm.record.raw.key, false),
        AccountMeta::new_readonly(state.realm.record.staging.key, false),
        AccountMeta::new_readonly(state.product.raw.key, false),
        AccountMeta::new_readonly(state.product.staging.key, false),
        AccountMeta::new_readonly(state.result_domain.raw.key, false),
        AccountMeta::new_readonly(state.result_domain.staging.key, false),
        AccountMeta::new_readonly(state.portfolio.raw.key, false),
        AccountMeta::new_readonly(state.portfolio.staging.key, false),
        AccountMeta::new_readonly(state.source_material.record.raw.key, false),
        AccountMeta::new_readonly(state.source_material.record.staging.key, false),
        AccountMeta::new_readonly(state.capability_manifest.record.raw.key, false),
        AccountMeta::new_readonly(state.capability_manifest.record.staging.key, false),
        AccountMeta::new_readonly(state.execution_release_set.record.raw.key, false),
        AccountMeta::new_readonly(state.execution_release_set.record.staging.key, false),
        AccountMeta::new_readonly(state.activation_cache.key, false),
        AccountMeta::new_readonly(state.core_program.key, false),
        AccountMeta::new_readonly(state.core_programdata.key, false),
        AccountMeta::new_readonly(state.registry_program.key, false),
        AccountMeta::new_readonly(state.rent.key, false),
        AccountMeta::new_readonly(state.system_program.key, false),
        AccountMeta::new_readonly(state.infrastructure_profile.key, false),
        AccountMeta::new_readonly(state.registry_artifact.raw.key, false),
        AccountMeta::new_readonly(state.registry_artifact.staging.key, false),
        AccountMeta::new_readonly(state.registry_programdata.key, false),
        AccountMeta::new_readonly(state.rent_artifact.raw.key, false),
        AccountMeta::new_readonly(state.rent_artifact.staging.key, false),
        AccountMeta::new_readonly(state.rent_programdata.key, false),
    ]
}

fn projection_accounts(
    state: FoundProjectionStateV2<'_>,
) -> [AccountObservationV2<'_>; FOUND_ACCOUNT_COUNT_V2 - 1] {
    [
        state.payer,
        state.market,
        state.rent_program,
        state.realm.record.raw,
        state.realm.record.staging,
        state.product.raw,
        state.product.staging,
        state.result_domain.raw,
        state.result_domain.staging,
        state.portfolio.raw,
        state.portfolio.staging,
        state.source_material.record.raw,
        state.source_material.record.staging,
        state.capability_manifest.record.raw,
        state.capability_manifest.record.staging,
        state.execution_release_set.record.raw,
        state.execution_release_set.record.staging,
        state.activation_cache,
        state.core_program,
        state.core_programdata,
        state.registry_program,
        state.rent,
        state.system_program,
        state.infrastructure_profile,
        state.registry_artifact.raw,
        state.registry_artifact.staging,
        state.registry_programdata,
        state.rent_artifact.raw,
        state.rent_artifact.staging,
        state.rent_programdata,
    ]
}

fn identity(bytes: [u8; 32]) -> Result<Identity> {
    Identity::new(bytes).map_err(|_| Error::InvalidRecord)
}
