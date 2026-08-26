//! Successor-only chain-derived Core Found construction.
//!
//! This builder accepts one finalized account snapshot, independently
//! authenticates every immutable record coordinate and cross-record join, and
//! emits the exact unsigned 24-account Core Found instruction. It performs no
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
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_release_set_contract::{
    EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1, ExecutionReleaseSetV1, ExecutionRoleV1,
};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RentCreditV1};
use dclutch_source_contract::{
    ContentId as SourceContentId, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SourceMaterialV2,
};
use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{native_loader, system_program, sysvar};

use crate::{
    AccountObservationV2, Error, FinalizedRecordObservationV2, Result, coordinate, digest,
};

/// Exact number of accounts in the Runtime V2 Core Found frame.
pub const FOUND_ACCOUNT_COUNT_V2: usize = 24;

/// One non-Product finalized raw/staging record observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FinalizedReferenceObservationV2<'a> {
    /// Exact schema/validator identity selecting the Registry PDA domain.
    pub schema_id: [u8; 32],
    /// Registry-owned raw and System-owned vacant staging observations.
    pub record: FinalizedRecordObservationV2<'a>,
}

/// One finalized snapshot sufficient to construct Core Found for Runtime V2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundStateV2<'a> {
    /// System-owned signing payer and Market rent sponsor.
    pub payer: AccountObservationV2<'a>,
    /// System-owned empty exact Market PDA destination.
    pub market: AccountObservationV2<'a>,
    /// Existing permanent RentCredit account.
    pub rent_credit: AccountObservationV2<'a>,
    /// Executable program owning the RentCredit.
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
    /// Exact executable Registry program owning all immutable records.
    pub registry_program: AccountObservationV2<'a>,
    /// Canonical Rent sysvar observation.
    pub rent: AccountObservationV2<'a>,
    /// Canonical executable System Program observation.
    pub system_program: AccountObservationV2<'a>,
}

/// Exact chain-derived Runtime V2 Core Found plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundInstructionPlanV2 {
    /// Exact unsigned 24-account instruction.
    pub instruction: Instruction,
    /// Sole derived Market address.
    pub market_address: Pubkey,
    /// Immutable Market identity reconstructed from authenticated records.
    pub market_identity: MarketIdentity,
    /// Ephemeral Product graph projection reconstructed from raw truth.
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
    authenticate_rent_credit(state)?;

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
    let request = Request::administrative(Action::Found, generation, market_identity.market_id)
        .encode()
        .map_err(|_| Error::InvalidRecord)?;
    if request.len() != REQUEST_BYTES {
        return Err(Error::InvalidRecord);
    }
    let accounts = found_metas(state);
    if accounts.len() != FOUND_ACCOUNT_COUNT_V2 {
        return Err(Error::AccountAuthority);
    }
    authenticate_selected_rent_program_v2(state)?;
    Ok(FoundInstructionPlanV2 {
        instruction: Instruction {
            program_id: state.core_program.key,
            accounts,
            data: request.to_vec(),
        },
        market_address,
        market_identity,
        outcome_count: product.join.outcome_count,
        product,
        observation_slot: slot,
        market_rent_top_up,
    })
}

fn authenticate_selected_rent_program_v2(_state: FoundStateV2<'_>) -> Result<()> {
    // Core Found currently proves only that the supplied executable program
    // owns a byte-valid RentCredit PDA. No Registry record, release set, Realm,
    // or Market coordinate selects that program. Until the onchain ABI adds
    // such an authority, exporting an instruction would turn a founder-chosen
    // program into client-side truth.
    Err(Error::UnselectedRentProgram)
}

fn require_one_slot(state: FoundStateV2<'_>) -> Result<u64> {
    let slot = state.registry_program.slot;
    if all_accounts(state)
        .iter()
        .any(|account| account.slot != slot)
    {
        return Err(Error::ObservationMismatch);
    }
    Ok(slot)
}

fn authenticate_runtime_accounts(state: FoundStateV2<'_>) -> Result<()> {
    let keys: Vec<Pubkey> = all_accounts(state)
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
        || state.system_program.key != system_program::ID
        || state.system_program.owner != native_loader::ID
        || !state.system_program.executable
        || !state.system_program.data.is_empty()
    {
        return Err(Error::AccountAuthority);
    }
    Ok(())
}

fn authenticate_record_rent_minima(state: FoundStateV2<'_>, rent: &Rent) -> Result<()> {
    let records = [
        state.realm.record,
        state.product,
        state.result_domain,
        state.portfolio,
        state.source_material.record,
        state.capability_manifest.record,
        state.execution_release_set.record,
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
    state: FoundStateV2<'_>,
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
    Ok(())
}

fn authenticate_rent_credit(state: FoundStateV2<'_>) -> Result<()> {
    if state.rent_credit.owner != state.rent_program.key || state.rent_credit.executable {
        return Err(Error::AccountAuthority);
    }
    let credit = RentCreditV1::decode(state.rent_credit.data).map_err(|_| Error::InvalidRecord)?;
    let authority = credit.refund_authority().to_bytes();
    let bump = [credit.pda_bump()];
    let expected = Pubkey::create_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, &authority, &bump],
        &state.rent_program.key,
    )
    .map_err(|_| Error::AccountAuthority)?;
    if expected != state.rent_credit.key {
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
    ]
}

fn all_accounts(state: FoundStateV2<'_>) -> [AccountObservationV2<'_>; FOUND_ACCOUNT_COUNT_V2] {
    [
        state.payer,
        state.market,
        state.rent_credit,
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
    ]
}

fn identity(bytes: [u8; 32]) -> Result<Identity> {
    Identity::new(bytes).map_err(|_| Error::InvalidRecord)
}
