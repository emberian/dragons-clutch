//! Successor-only chain-derived Core Found construction.
//!
//! This builder accepts one finalized account snapshot, independently
//! authenticates every immutable record coordinate and cross-record join, and
//! emits the exact unsigned 31-account Core Found instruction. It performs no
//! RPC, signing, submission, funding, or account mutation.

use dclutch_market::capability_manifest::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1,
    funding::funded_rent_persists_v1,
};
use dclutch_market::{
    Action, FOUND_ACCOUNT_ROLES_V3, FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3,
    FOUND_PRICE_GATE_RAW_INDEX_V3, Identity, MarketCoreStateSeedsV2, MarketIdentity, REQUEST_BYTES,
    Request, STATE_BYTES,
};
use dclutch_product::payoff::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3, semantic_basis_preimage_v3},
};
use dclutch_product::ResultDomainV2;
use dclutch_product::admission::{
    AdmissionProjectionV2, AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2, admit_authenticated_records_v2,
};
use dclutch_market::realm::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::{
    ACTIVATION_PDA_DOMAIN_V1, ARTIFACT_RELEASE_SCHEMA_ID_V1, ActivatedExecutionReleaseSetViewV1,
    ArtifactReleaseV1, DeploymentObservationV1, require_slot_pinned_release_v1,
};
use dclutch_registry::svm::{ProgramDataV3View, ProgramV3View};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, ExecutionRoleBindingV1, ExecutionRoleV1,
    PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2, PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2,
    ProtocolInfrastructureProfileV2,
};
use dclutch_market::rent::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleRentCreditV2,
};
use dclutch_source::{
    ContentId as SourceContentId, MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, ManipulationFloorV1,
    SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_SPEC_SCHEMA_ID_V1, SourceCapacityProfileV1, SourceMaterialV3, SourcePrincipalPolicyV1,
    SourceSpecV1,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use crate::{
    AccountObservationV2, Error, FinalizedRecordObservationV2, Result, coordinate, digest,
};

/// Exact number of accounts in the Runtime V2 ordinary Core Found V3 frame.
pub use dclutch_market::{FOUND_ACCOUNT_COUNT_V3, FOUND_PRICE_GATE_ACCOUNT_COUNT_V3};

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
    /// Finalized Product-linked ProductBasisV3 raw/staging pair.
    pub linked_basis: FinalizedRecordObservationV2<'a>,
    /// Finalized SourceMaterialV3 raw/staging pair.
    pub source_material: FinalizedReferenceObservationV2<'a>,
    /// Finalized SourceSpecV1 raw/staging pair selected by SourceMaterialV3.
    pub source_spec: FinalizedReferenceObservationV2<'a>,
    /// Finalized SourceCapacityProfileV1 pair selected by SourceSpecV1.
    pub capacity_profile: FinalizedReferenceObservationV2<'a>,
    /// Selected finalized ManipulationFloorV1 pair, or the canonical absent pair.
    pub manipulation_floor: FinalizedReferenceObservationV2<'a>,
    /// Finalized capability-manifest raw/staging pair.
    pub capability_manifest: FinalizedReferenceObservationV2<'a>,
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
    /// Finalized Product-linked ProductBasisV3 raw/staging pair.
    pub linked_basis: FinalizedRecordObservationV2<'a>,
    /// Finalized SourceMaterialV3 raw/staging pair.
    pub source_material: FinalizedReferenceObservationV2<'a>,
    /// Finalized SourceSpecV1 raw/staging pair selected by SourceMaterialV3.
    pub source_spec: FinalizedReferenceObservationV2<'a>,
    /// Finalized SourceCapacityProfileV1 pair selected by SourceSpecV1.
    pub capacity_profile: FinalizedReferenceObservationV2<'a>,
    /// Selected finalized ManipulationFloorV1 pair, or the canonical absent pair.
    pub manipulation_floor: FinalizedReferenceObservationV2<'a>,
    /// Finalized capability-manifest raw/staging pair.
    pub capability_manifest: FinalizedReferenceObservationV2<'a>,
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
    /// Finalized `DCLTPGT1` no-arbitrage certificate, when the basis needs one.
    ///
    /// `None` is the ordinary case and produces the canonical 37-account
    /// frame, byte-for-byte as before. A basis declaring degree >= 2 requires
    /// one, and supplying it appends the pair at the end of the frame -- so
    /// nothing an existing caller builds moves.
    pub price_gate: Option<FinalizedRecordObservationV2<'a>>,
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
            linked_basis: self.linked_basis,
            source_material: self.source_material,
            source_spec: self.source_spec,
            capacity_profile: self.capacity_profile,
            manipulation_floor: self.manipulation_floor,
            capability_manifest: self.capability_manifest,
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
    /// Source-policy principal cap projected through the authenticated basis scale.
    pub principal_cap_sets: u64,
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
    /// Source-policy principal cap projected through the authenticated basis scale.
    pub principal_cap_sets: u64,
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
    .map_err(Error::MarketCore)?;
    if request.len() != REQUEST_BYTES {
        return Err(Error::InvalidRecord);
    }
    let accounts = found_metas(state);
    let expected = if state.price_gate.is_some() {
        FOUND_PRICE_GATE_ACCOUNT_COUNT_V3
    } else {
        FOUND_ACCOUNT_COUNT_V3
    };
    if accounts.len() != expected {
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
        principal_cap_sets: projection.principal_cap_sets,
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
    let realm = RealmV1::decode(state.realm.record.raw.data).map_err(Error::Realm)?;
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
    .map_err(Error::ProductRuntimeAdmission)?;

    let linked_basis_digest = authenticate_product_record(
        state.registry_program.key,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        state.linked_basis,
    )?;
    let basis = ProductBasisV3::decode(state.linked_basis.raw.data).map_err(Error::ProductBasis)?;
    let domain =
        ResultDomainV2::decode(state.result_domain.raw.data).map_err(Error::ProductRuntime)?;
    let semantic =
        semantic_basis_preimage_v3(state.linked_basis.raw.data).map_err(Error::ProductBasis)?;
    let semantic_basis_id = hashv(&[
        SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        semantic.prefix(),
        semantic.suffix(),
    ])
    .to_bytes();
    if linked_basis_digest.content_digest.to_bytes() != hash(state.linked_basis.raw.data).to_bytes()
        || semantic_basis_id != product.join.liability_basis_id.to_bytes()
        || basis.product_id() != product.join.product_id.to_bytes()
        || basis.result_domain_id() != product.join.result_domain_id.to_bytes()
        || basis.coordinate_domain_id() != domain.coordinate_domain_id().to_bytes()
        || basis.result_unit_id() != domain.result_unit_id().to_bytes()
        || basis.payout_scale() == 0
    {
        return Err(Error::CrossRecordMismatch);
    }

    let source_digest = authenticate_reference(
        state.registry_program.key,
        state.source_material,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    )?;
    let material =
        SourceMaterialV3::decode(state.source_material.record.raw.data).map_err(Error::Source)?;
    material
        .authenticate_product_record(
            SourceContentId::new(product.product_record_digest.to_bytes())
                .map_err(|_| Error::InvalidRecord)?,
        )
        .map_err(|_| Error::CrossRecordMismatch)?;
    let source_spec_digest = authenticate_reference(
        state.registry_program.key,
        state.source_spec,
        SOURCE_SPEC_SCHEMA_ID_V1,
    )?;
    let source_spec =
        SourceSpecV1::decode(state.source_spec.record.raw.data).map_err(Error::Source)?;
    let capacity_profile_digest = authenticate_reference(
        state.registry_program.key,
        state.capacity_profile,
        SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
    )?;
    let capacity_profile = SourceCapacityProfileV1::decode(state.capacity_profile.record.raw.data)
        .map_err(Error::Source)?;
    let manipulation_floor = match material.principal_policy() {
        SourcePrincipalPolicyV1::ExplicitlyUnbounded => {
            authenticate_absent_optional_reference(
                state.registry_program.key,
                state.manipulation_floor,
                MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            )?;
            None
        }
        SourcePrincipalPolicyV1::BoundedByFloor(_) => {
            let floor_digest = authenticate_reference(
                state.registry_program.key,
                state.manipulation_floor,
                MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            )?;
            let floor = ManipulationFloorV1::decode(state.manipulation_floor.record.raw.data)
                .map_err(Error::Source)?;
            Some((
                SourceContentId::new(floor_digest.to_bytes()).map_err(|_| Error::InvalidRecord)?,
                floor,
            ))
        }
    };
    let principal_cap_sets = material
        .derive_principal_cap_sets(
            SourceContentId::new(source_spec_digest.to_bytes())
                .map_err(|_| Error::InvalidRecord)?,
            source_spec,
            SourceContentId::new(capacity_profile_digest.to_bytes())
                .map_err(|_| Error::InvalidRecord)?,
            capacity_profile,
            manipulation_floor,
            SourceContentId::new(*realm.collateral_mint()).map_err(|_| Error::InvalidRecord)?,
            basis.payout_scale(),
        )
        .map_err(|_| Error::CrossRecordMismatch)?
        .to_sets();
    if principal_cap_sets == 0 {
        return Err(Error::CrossRecordMismatch);
    }

    let manifest_digest = authenticate_reference(
        state.registry_program.key,
        state.capability_manifest,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    )?;
    CapabilityManifestV1::decode(state.capability_manifest.record.raw.data)
        .map_err(Error::Capability)?;

    let release_set_digest = authenticate_activation(state)?;
    authenticate_infrastructure(state)?;

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
        principal_cap_sets,
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
        || state.infrastructure_profile.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
        || state.registry_programdata.executable
        || state.rent_programdata.executable
        || state.system_program.key != system_program::ID
        || state.system_program.owner != native_loader::ID
        || !state.system_program.executable
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
        state.linked_basis,
        state.source_material.record,
        state.source_spec.record,
        state.capacity_profile.record,
        state.capability_manifest.record,
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
) -> Result<dclutch_product::ContentId> {
    if reference.schema_id != expected_schema {
        return Err(Error::AccountAuthority);
    }
    let coordinate = authenticate_product_record(registry, expected_schema, reference.record)?;
    Ok(coordinate.content_digest)
}

fn authenticate_absent_optional_reference(
    registry: Pubkey,
    reference: FinalizedReferenceObservationV2<'_>,
    expected_schema: [u8; 32],
) -> Result<()> {
    if reference.schema_id != expected_schema {
        return Err(Error::AccountAuthority);
    }
    let absent = [0_u8; 32];
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &expected_schema, &absent],
        &registry,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &expected_schema, &absent],
        &registry,
    )
    .0;
    for (observation, expected) in [
        (reference.record.raw, expected_raw),
        (reference.record.staging, expected_staging),
    ] {
        if observation.key != expected
            || observation.owner != system_program::ID
            || observation.executable
            || !observation.data.is_empty()
        {
            return Err(Error::AccountAuthority);
        }
    }
    Ok(())
}

fn authenticate_activation(
    state: FoundProjectionStateV2<'_>,
) -> Result<dclutch_product::ContentId> {
    let activated = ActivatedExecutionReleaseSetViewV1::decode(state.activation_cache.data)
        .map_err(Error::Registry)?;
    let release_set_digest = activated
        .execution_release_set_id()
        .map_err(Error::Registry)?
        .to_bytes();
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
    let release_set = activated
        .release_set_projection()
        .map_err(Error::Registry)?;
    let core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(Error::Registry)?;
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
    authenticate_current_deployment(state.core_program, state.core_programdata, release)?;
    dclutch_product::ContentId::new(release_set_digest).map_err(|_| Error::InvalidRecord)
}

fn authenticate_infrastructure(state: FoundProjectionStateV2<'_>) -> Result<()> {
    let expected_profile = Pubkey::find_program_address(
        &[PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2],
        &state.core_program.key,
    )
    .0;
    if state.infrastructure_profile.key != expected_profile
        || !funded_rent_persists_v1(state.infrastructure_profile.lamports)
    {
        return Err(Error::AccountAuthority);
    }
    let profile = ProtocolInfrastructureProfileV2::decode(state.infrastructure_profile.data)
        .map_err(Error::ReleaseSet)?;
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
    let release = ArtifactReleaseV1::decode(observation.raw.data).map_err(Error::Registry)?;
    if release.program().to_bytes() != program.key.to_bytes() {
        return Err(Error::CrossRecordMismatch);
    }
    authenticate_current_deployment(program, programdata, release)?;
    let artifact_release = ArtifactReleaseIdV1::new(coordinate.content_digest.to_bytes())
        .map_err(Error::ReleaseSet)?;
    Ok(ExecutionRoleBindingV1::new(
        release.program(),
        artifact_release,
    ))
}

/// Mirror the on-chain founding admission for one Loader V3 deployment.
///
/// This host path always hashes the observed ELF, so its soundness never rested
/// on immutability; what the release's upgrade policy decides is only WHICH
/// deployments are admissible at all. Decision 0012 made that the two canonical
/// pinned shapes, and `authenticate_deployment` still holds the deployment to
/// the exact slot and exact authority the release bound.
fn authenticate_current_deployment(
    program: AccountObservationV2<'_>,
    programdata: AccountObservationV2<'_>,
    release: ArtifactReleaseV1,
) -> Result<()> {
    require_slot_pinned_release_v1(release).map_err(Error::Registry)?;
    if release.loader_program().to_bytes() != bpf_loader_upgradeable::ID.to_bytes()
        || release.program().to_bytes() != program.key.to_bytes()
        || release.programdata() != programdata.key.to_bytes()
        || program.owner != bpf_loader_upgradeable::ID
        || programdata.owner != bpf_loader_upgradeable::ID
        || !program.executable
        || programdata.executable
    {
        return Err(Error::AccountAuthority);
    }
    let program_view = ProgramV3View::parse(program.data).map_err(Error::RegistrySvm)?;
    let expected_programdata =
        Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0;
    if program_view.programdata() != release.programdata()
        || programdata.key != expected_programdata
    {
        return Err(Error::CrossRecordMismatch);
    }
    let programdata_view =
        ProgramDataV3View::parse(programdata.data).map_err(Error::RegistrySvm)?;
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
    .map_err(Error::Registry)?;
    release
        .authenticate_deployment(deployment)
        .map_err(Error::Registry)
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
    let credit = LifecycleRentCreditV2::decode(rent_credit.data).map_err(Error::LifecycleRent)?;
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
    // **The canonical projection below is one literal and stays one.** The SDK
    // and web ABI generators read `found_metas` by regex and derive the 37
    // account labels from exactly this `vec![...]`; splitting the extension into
    // its own step is what keeps the canonical frame something a machine can
    // still recognise, and what keeps a TypeScript client that never founds
    // curvature working with no change at all.
    //
    // It carries the account keys and nothing else. Each slot's writable and
    // signer privilege comes from `FOUND_ACCOUNT_ROLES_V3`, which Lean emits
    // from the same frame this order belongs to, so the order and the
    // privileges can no longer be edited apart -- which is exactly what
    // thirty-seven hand-written `new`/`new_readonly` choices, one per line,
    // allowed.
    let keys = vec![
        state.payer.key,
        state.market.key,
        state.rent_credit.key,
        state.rent_program.key,
        state.realm.record.raw.key,
        state.realm.record.staging.key,
        state.product.raw.key,
        state.product.staging.key,
        state.result_domain.raw.key,
        state.result_domain.staging.key,
        state.portfolio.raw.key,
        state.portfolio.staging.key,
        state.linked_basis.raw.key,
        state.linked_basis.staging.key,
        state.source_material.record.raw.key,
        state.source_material.record.staging.key,
        state.source_spec.record.raw.key,
        state.source_spec.record.staging.key,
        state.capacity_profile.record.raw.key,
        state.capacity_profile.record.staging.key,
        state.manipulation_floor.record.raw.key,
        state.manipulation_floor.record.staging.key,
        state.capability_manifest.record.raw.key,
        state.capability_manifest.record.staging.key,
        state.activation_cache.key,
        state.core_program.key,
        state.core_programdata.key,
        state.registry_program.key,
        state.rent.key,
        state.system_program.key,
        state.infrastructure_profile.key,
        state.registry_artifact.raw.key,
        state.registry_artifact.staging.key,
        state.registry_programdata.key,
        state.rent_artifact.raw.key,
        state.rent_artifact.staging.key,
        state.rent_programdata.key,
    ];
    let accounts = keys
        .into_iter()
        .zip(FOUND_ACCOUNT_ROLES_V3)
        .map(|(key, (writable, signer))| {
            if writable {
                AccountMeta::new(key, signer)
            } else {
                AccountMeta::new_readonly(key, signer)
            }
        })
        .collect();
    let accounts = extend_with_price_gate(accounts, state.price_gate);
    debug_assert_eq!(
        accounts
            .get(FOUND_CAPABILITY_MANIFEST_RAW_INDEX_V3)
            .map(|meta| meta.pubkey),
        Some(state.capability_manifest.record.raw.key),
    );
    accounts
}

fn extend_with_price_gate(
    mut accounts: Vec<AccountMeta>,
    certificate: Option<FinalizedRecordObservationV2<'_>>,
) -> Vec<AccountMeta> {
    if let Some(certificate) = certificate {
        // The appended pair takes the last two entries of the same emitted
        // table, so the extension cannot acquire a privilege the frame does not
        // declare for it.
        for (key, (writable, signer)) in [certificate.raw.key, certificate.staging.key]
            .into_iter()
            .zip(
                FOUND_ACCOUNT_ROLES_V3[FOUND_PRICE_GATE_RAW_INDEX_V3..]
                    .iter()
                    .copied(),
            )
        {
            accounts.push(if writable {
                AccountMeta::new(key, signer)
            } else {
                AccountMeta::new_readonly(key, signer)
            });
        }
    }
    accounts
}

fn projection_accounts(
    state: FoundProjectionStateV2<'_>,
) -> [AccountObservationV2<'_>; FOUND_ACCOUNT_COUNT_V3 - 1] {
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
        state.linked_basis.raw,
        state.linked_basis.staging,
        state.source_material.record.raw,
        state.source_material.record.staging,
        state.source_spec.record.raw,
        state.source_spec.record.staging,
        state.capacity_profile.record.raw,
        state.capacity_profile.record.staging,
        state.manipulation_floor.record.raw,
        state.manipulation_floor.record.staging,
        state.capability_manifest.record.raw,
        state.capability_manifest.record.staging,
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
    Identity::new(bytes).map_err(Error::MarketCore)
}
