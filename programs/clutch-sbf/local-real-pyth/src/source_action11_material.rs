//! Chain-derived unsigned material for current Source action 11.
//!
//! The operator accepts one bounded finalized snapshot and reconstructs the
//! complete typed reopen request, closed lineage, target postimage, immutable
//! work receipt, custody-ledger postimage, account order, and v0 transaction.
//! It never accepts target bytes, semantic IDs, rent amounts, signer material,
//! or a browser-authored transaction request.

use crate::rpc_index::{
    CanonicalFamily, CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount,
    ObservedRpcAccountRemoval, RpcAccountRemovalKind, RpcCommitment,
    RpcObservationProvenance,
};
use crate::transaction_builder::{
    ConstructionError, ExactEquation, IntegerUnit, OwnedInstructionDraft,
    ProtocolTransactionBuilder, SemanticOwner, TransactionTransport, UnsignedProtocolTransaction,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::registry::{ExtensionFamily, SourceSeriesAction};
use clutch_solana_layout::source_series::{
    account_contract_v2, validate_account_metas_v2, ObservedSourceAccountMetaV2,
    ReopenGenerationIntentV2, SourceMutableFamilyV2, REOPEN_GENERATION_PAYLOAD_BYTES_V2,
};
use clutch_source_plane_v3::{
    ContentId, FixedCodec, OpenRawPageV3, SourceHeadV3, StatisticResultV3, WindowWorkV3,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{
    account_data_id, authenticate_reopen_lineage_account,
    authenticate_source_release_account, authenticate_source_reopen_generation_request,
    authenticate_source_route, authenticate_source_work_receipt_account, authorize_reopen,
    encode_runtime_account, open_lineage_generation, plan_source_account_creation,
    AuthenticatedReopenLineageV1, AuthenticatedSourceRouteV1, LineageAccessV1, LineageFamilyV1,
    ReopenLineageV1, RentExemptionQuoteV1, RuntimeAccountBodyV1, RuntimeAccountHeaderV1,
    RuntimeAccountViewV1, RuntimeDerivedPdaV1, RuntimeKey, SourceFundingCustodyLedgerV1,
    SourceReopenFamilyV1, SourceReopenGenerationRequestV1, SourceReopenTargetV1,
    SourceWorkAuthorizationV1, SourceWorkKindV1, SourceWorkReceiptAccessV1,
    SourceWorkReceiptAccountV1, SourceWorkScheduleBindingV1, RUNTIME_ACCOUNT_HEADER_BYTES,
    SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES, SOURCE_WORK_RECEIPT_ACCOUNT_BYTES,
    SOURCE_WORK_SCHEDULE_BYTES,
};
use sha2::{Digest, Sha256};
use solana_address::Address;
use solana_instruction::AccountMeta;
use solana_rent::Rent;

pub const SOURCE_ACTION11_VALIDITY_SLOTS_V1: u64 = 32;
pub const SOURCE_ACTION11_FAMILY_V1: ExtensionFamily = ExtensionFamily::SourceSeries;
pub const SOURCE_ACTION11_LOCAL_ACTION_V1: u8 = 11;

const OWNER_SCHEMA_V1: &str = "dragons-clutch/operator/source-action11-material/v1";
const OWNER_PACKAGE_V1: &str = "clutch-source-plane-v3-runtime";
const SEED_PRODUCT_ARTIFACT_V1: &[u8] = b"dc:product-artifact:v1";
const SEED_SOURCE_FUNDING_CUSTODY_V1: &[u8] = b"dc:source-funding:v1";
const SEED_SOURCE_REOPEN_REQUEST_V1: &[u8] = b"dc-sp3-reopen-request";
const SOURCE_FUNDING_CUSTODY_AUTH_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/authenticated-source-funding-custody/v1";
const SOURCE_FUNDING_CUSTODY_PHYSICAL_TRANSITION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/sbf/source-funding-custody-physical-transition/v1";
const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);
const RENT_SYSVAR_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218,
    238, 8, 155, 161, 253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);
const SYSVAR_OWNER_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 117, 247, 41, 199, 61, 147, 64, 143, 33, 97, 32, 6, 126,
    216, 140, 118, 224, 140, 40, 127, 193, 148, 96, 0, 0, 0, 0,
]);
const RENT_SYSVAR_BYTES_V1: usize = 17;

pub type SourceAction11MaterialResult<T> =
    core::result::Result<T, SourceAction11MaterialError>;
type Result<T> = SourceAction11MaterialResult<T>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceAction11MaterialError {
    CheckedRelease,
    ChainSnapshot,
    ChainAuthority,
    AccountOccupancy,
    Funding,
    Arithmetic,
    Construction,
}

impl core::fmt::Display for SourceAction11MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not admit Source action 11",
            Self::ChainSnapshot => "Source reopen accounts are not one finalized snapshot",
            Self::ChainAuthority => "hostile Source reopen authority failed authentication",
            Self::AccountOccupancy => "Source reopen target or receipt slot is occupied",
            Self::Funding => "Source custody cannot fund exact reopen rent and work",
            Self::Arithmetic => "Source action-11 exact arithmetic overflowed",
            Self::Construction => "canonical Source action-11 construction refused",
        })
    }
}

impl std::error::Error for SourceAction11MaterialError {}

/// A finalized target slot may be a system-owned prefund or a positive proof
/// that the account was closed. Both cases carry the same snapshot provenance.
#[derive(Clone, Copy, Debug)]
pub enum ObservedSourceReopenSlotV1<'a> {
    Present(&'a ObservedRpcAccount),
    Removed(&'a ObservedRpcAccountRemoval),
}

impl ObservedSourceReopenSlotV1<'_> {
    fn address(self) -> Address {
        match self {
            Self::Present(account) => account.address,
            Self::Removed(account) => account.address,
        }
    }

    fn provenance(self) -> &RpcObservationProvenance {
        match self {
            Self::Present(account) => &account.provenance,
            Self::Removed(account) => &account.provenance,
        }
    }

    fn validate(self, expected: Address) -> Result<u64> {
        if self.address() != expected {
            return Err(SourceAction11MaterialError::AccountOccupancy);
        }
        match self {
            Self::Present(account) => {
                if account.owner != SYSTEM_PROGRAM_ID
                    || account.executable
                    || !account.data.is_empty()
                {
                    return Err(SourceAction11MaterialError::AccountOccupancy);
                }
                Ok(account.lamports)
            }
            Self::Removed(account) => {
                if account.kind != RpcAccountRemovalKind::Closed
                    || account.observed_owner != SYSTEM_PROGRAM_ID
                    || account.observed_lamports != 0
                    || account.observed_executable
                    || account.observed_data_bytes != 0
                {
                    return Err(SourceAction11MaterialError::AccountOccupancy);
                }
                Ok(0)
            }
        }
    }
}

/// Complete raw finalized snapshot for one deterministic reopen.
#[derive(Clone, Copy, Debug)]
pub struct SourceAction11ChainSnapshotV1<'a> {
    pub source_release: &'a ObservedRpcAccount,
    pub adapter_program: &'a ObservedRpcAccount,
    pub adapter_program_data: &'a ObservedRpcAccount,
    pub parser_program: &'a ObservedRpcAccount,
    pub parser_program_data: &'a ObservedRpcAccount,
    pub parser_config: &'a ObservedRpcAccount,
    pub source_spec: &'a ObservedRpcAccount,
    pub source_work_schedule: &'a ObservedRpcAccount,
    pub generation_authority: &'a ObservedRpcAccount,
    pub generation_target: ObservedSourceReopenSlotV1<'a>,
    pub generation_lineage: &'a ObservedRpcAccount,
    pub source_work_receipt: ObservedSourceReopenSlotV1<'a>,
    pub keeper: &'a ObservedRpcAccount,
    pub source_funding_custody: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
}

/// Opaque chain-derived action-11 instruction and accounting projection.
#[derive(Clone, Debug)]
pub struct ChainDerivedSourceAction11MaterialV1 {
    checked_release_key: String,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    program_id: Address,
    program_data: Address,
    family: SourceMutableFamilyV2,
    call_ordinal: u32,
    source_release_manifest_id: [u8; 32],
    lineage_state_before_id: [u8; 32],
    semantic_binding_id: [u8; 32],
    target_body_id: [u8; 32],
    target_post_data_id: [u8; 32],
    lineage_state_after_id: [u8; 32],
    work_receipt_id: [u8; 32],
    custody_post_data_id: [u8; 32],
    target_rent_debit_lamports: u64,
    receipt_rent_debit_lamports: u64,
    keeper_payment_lamports: u64,
    custody_balance_before_lamports: u64,
    custody_balance_after_lamports: u64,
    custody_principal_before_lamports: u64,
    custody_principal_after_lamports: u64,
    keeper_balance_before_lamports: u64,
    keeper_balance_after_lamports: u64,
    valid_before_slot: u64,
    keeper: Address,
    ordered_accounts: Vec<AccountMeta>,
}

impl ChainDerivedSourceAction11MaterialV1 {
    pub const fn family(&self) -> SourceMutableFamilyV2 { self.family }
    pub const fn call_ordinal(&self) -> u32 { self.call_ordinal }
    pub const fn target_post_data_id(&self) -> [u8; 32] { self.target_post_data_id }
    pub const fn lineage_state_after_id(&self) -> [u8; 32] { self.lineage_state_after_id }
    pub const fn work_receipt_id(&self) -> [u8; 32] { self.work_receipt_id }
    pub const fn custody_post_data_id(&self) -> [u8; 32] { self.custody_post_data_id }

    pub fn unsigned_instruction(&self, release: &IndexedProgramRelease) -> Result<OwnedInstructionDraft> {
        let coordinate = CanonicalIntentCoordinate {
            family_tag: SOURCE_ACTION11_FAMILY_V1.tag(),
            family_version: SOURCE_ACTION11_FAMILY_V1.version(),
            local_action: SOURCE_ACTION11_LOCAL_ACTION_V1,
        };
        if release.key() != self.checked_release_key
            || release.program_id != self.program_id
            || release.program_data != self.program_data
            || release.release_manifest_sha256 != self.release_manifest_sha256
            || release.capability_profile_id != self.capability_profile_id
            || release.enabled_intents.binary_search(&coordinate).is_err()
        {
            return Err(SourceAction11MaterialError::CheckedRelease);
        }
        let intent = ReopenGenerationIntentV2 {
            family: self.family,
            source_release_manifest_id: self.source_release_manifest_id,
            expected_lineage_state_id: self.lineage_state_before_id,
            semantic_binding_id: self.semantic_binding_id,
            target_body_id: self.target_body_id,
            valid_before_slot: self.valid_before_slot,
        };
        let mut payload = [0_u8; REOPEN_GENERATION_PAYLOAD_BYTES_V2];
        intent.encode(&mut payload).map_err(|_| SourceAction11MaterialError::Construction)?;
        let total_debit = self.target_rent_debit_lamports
            .checked_add(self.receipt_rent_debit_lamports)
            .and_then(|value| value.checked_add(self.keeper_payment_lamports))
            .ok_or(SourceAction11MaterialError::Arithmetic)?;
        OwnedInstructionDraft::checked_release_source_request_v2(
            release,
            "reopen-source-generation-v2",
            SemanticOwner {
                package: OWNER_PACKAGE_V1.into(),
                schema: OWNER_SCHEMA_V1.into(),
                release_sha256: self.release_manifest_sha256,
            },
            self.ordered_accounts.clone(),
            vec![self.keeper],
            vec![
                ExactEquation {
                    name: "Source lifecycle custody funds only target rent, receipt rent, and terminal work".into(),
                    unit: IntegerUnit::Lamports,
                    left: u128::from(self.custody_balance_before_lamports),
                    right: u128::from(self.custody_balance_after_lamports) + u128::from(total_debit),
                },
                ExactEquation {
                    name: "Source custody semantic principal bears the exact reopen debit".into(),
                    unit: IntegerUnit::Lamports,
                    left: u128::from(self.custody_principal_before_lamports),
                    right: u128::from(self.custody_principal_after_lamports) + u128::from(total_debit),
                },
                ExactEquation {
                    name: "Source postterminal keeper receives the release-selected ceiling".into(),
                    unit: IntegerUnit::Lamports,
                    left: u128::from(self.keeper_balance_before_lamports)
                        + u128::from(self.keeper_payment_lamports),
                    right: u128::from(self.keeper_balance_after_lamports),
                },
            ],
            SourceSeriesAction::ReopenGeneration,
            self.call_ordinal,
            &payload,
        ).map_err(map_construction)
    }

    /// Compile one exact unsigned v0 transaction. The already-authenticated
    /// writable keeper is the transport fee payer; custody never signs.
    pub fn unsigned_transaction(
        &self,
        release: &IndexedProgramRelease,
        transport: TransactionTransport,
    ) -> Result<UnsignedProtocolTransaction> {
        let draft = self.unsigned_instruction(release)?;
        ProtocolTransactionBuilder::new(
            self.keeper, self.program_id, self.release_manifest_sha256, transport,
        ).and_then(|builder| builder.build_source_v0(draft)).map_err(map_construction)
    }
}

/// Derive action 11 without caller-authored semantic fields or account metas.
pub fn derive_source_action11_material_v1(
    release: &IndexedProgramRelease,
    snapshot: SourceAction11ChainSnapshotV1<'_>,
) -> Result<ChainDerivedSourceAction11MaterialV1> {
    authenticate_release_shape(release)?;
    authenticate_snapshot_provenance(release, snapshot)?;
    let program_id = release.program_id;
    let program_key = runtime_key(program_id);

    let manifest = clutch_source_plane_v3_runtime::SourceReleaseManifestV2::decode(
        &snapshot.source_release.data,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let release_recipe = PdaRecipeV3::source_release(
        manifest.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let authenticated_release = authenticate_source_release_account(
        program_key,
        account_view(snapshot.source_release, false, false),
        derive_recipe(program_id, release_recipe)?,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    if snapshot.adapter_program.address != release.program_id
        || snapshot.adapter_program_data.address != release.program_data
    {
        return Err(SourceAction11MaterialError::CheckedRelease);
    }
    let route = authenticate_source_route(
        authenticated_release,
        account_view(snapshot.adapter_program, false, false),
        account_view(snapshot.adapter_program_data, false, false),
        account_view(snapshot.parser_program, false, false),
        account_view(snapshot.parser_program_data, false, false),
        account_view(snapshot.parser_config, false, false),
        account_view(snapshot.source_spec, false, false),
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let schedule = authenticate_schedule(program_id, route, snapshot.source_work_schedule)?;
    let custody = authenticate_custody(program_id, route, schedule, snapshot.source_funding_custody)?;
    let initial_custody_data_id = account_data_id(
        runtime_key(snapshot.source_funding_custody.address), &snapshot.source_funding_custody.data,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let custody_authentication_id = custody_authentication_id(
        route, schedule, snapshot.source_funding_custody, initial_custody_data_id, custody,
    )?;

    let lineage_body = ReopenLineageV1::decode(&snapshot.generation_lineage.data)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let lineage_recipe = PdaRecipeV3::reopen_lineage(
        lineage_body.recipe_id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let lineage = authenticate_reopen_lineage_account(
        route,
        account_view(snapshot.generation_lineage, true, false),
        derive_recipe(program_id, lineage_recipe)?,
        LineageAccessV1::Mutable,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    if lineage.lineage().is_open || lineage.lineage().latest_generation == 0 {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }

    let request = SourceReopenGenerationRequestV1::decode(&snapshot.generation_authority.data)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let request_id = request.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let generation_authority_program = address(route.generation_authority_program());
    let (request_address, request_bump) = Address::find_program_address(
        &[SEED_SOURCE_REOPEN_REQUEST_V1, &request_id.bytes()], &generation_authority_program,
    );
    let authorization = authenticate_source_reopen_generation_request(
        route,
        account_view(snapshot.generation_authority, false, false),
        RuntimeDerivedPdaV1 {
            program_id: route.generation_authority_program(),
            recipe_id: request_id,
            address: runtime_key(request_address),
            bump: request_bump,
        },
        lineage,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    if snapshot.generation_authority.address != request_address {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }

    let target = *authorization.target();
    let target_recipe = target.recipe(route).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let target_derived = derive_recipe(program_id, target_recipe)?;
    let target_address = address(target_derived.address);
    let target_prefund = snapshot.generation_target.validate(target_address)?;
    let rent = authenticate_rent(snapshot.rent_sysvar)?;
    let rent_data_id = account_data_id(runtime_key(RENT_SYSVAR_ID), &snapshot.rent_sysvar.data)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let projection = match target {
        SourceReopenTargetV1::SourceHead(body) => project_target(
            route, lineage, LineageFamilyV1::SourceHead, &target_recipe, target_derived,
            target_address, target_prefund, rent, rent_data_id, snapshot.source_funding_custody.address, &body,
        ),
        SourceReopenTargetV1::OpenRawPage(body) => project_target(
            route, lineage, LineageFamilyV1::OpenRawPage, &target_recipe, target_derived,
            target_address, target_prefund, rent, rent_data_id, snapshot.source_funding_custody.address, &body,
        ),
        SourceReopenTargetV1::WindowWork(body) => project_target(
            route, lineage, LineageFamilyV1::WindowWork, &target_recipe, target_derived,
            target_address, target_prefund, rent, rent_data_id, snapshot.source_funding_custody.address, &body,
        ),
        SourceReopenTargetV1::StatisticResult(body) => project_target(
            route, lineage, LineageFamilyV1::StatisticResult, &target_recipe, target_derived,
            target_address, target_prefund, rent, rent_data_id, snapshot.source_funding_custody.address, &body,
        ),
    }?;
    if authorization.generation_policy_id() != lineage.lineage().last_close_receipt_id {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }
    let family = mutable_family(target.family());
    let call_ordinal = u32::try_from(projection.header.generation)
        .map_err(|_| SourceAction11MaterialError::Arithmetic)?;
    if call_ordinal == 0 || call_ordinal > schedule.maximum_calls() {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }
    let lineage_after_bytes = projection.lineage_after.encode()
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let lineage_state_after_id = account_data_id(
        runtime_key(snapshot.generation_lineage.address), &lineage_after_bytes,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let semantic_receipt_id = ContentId::from_bytes(hashv(&[
        b"dragons-clutch/source-reopen-action/v1",
        &route.route_id().bytes(),
        &authorization.id().bytes(),
        &projection.account_data_id.bytes(),
        &lineage_state_after_id.bytes(),
    ]));
    if semantic_receipt_id.is_zero() { return Err(SourceAction11MaterialError::ChainAuthority); }

    let kind = SourceWorkKindV1::TerminalLifecycle;
    let ceiling = schedule.ceiling_for(kind);
    if ceiling == 0 || snapshot.keeper.address == Address::default()
        || snapshot.keeper.executable || snapshot.keeper.address == snapshot.source_funding_custody.address
    {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }
    let receipt_slot_id = SourceWorkAuthorizationV1::receipt_slot_id(
        route, schedule, kind, call_ordinal, semantic_receipt_id,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let receipt_recipe = PdaRecipeV3::source_work_receipt(receipt_slot_id)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let receipt_derived = derive_recipe(program_id, receipt_recipe)?;
    let receipt_address = address(receipt_derived.address);
    let receipt_prefund = snapshot.source_work_receipt.validate(receipt_address)?;
    let work_authorization = SourceWorkAuthorizationV1::new(
        route, schedule, kind, receipt_derived.address, call_ordinal, ceiling, semantic_receipt_id,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let receipt = SourceWorkReceiptAccountV1::from_work(route, work_authorization)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let receipt_bytes = receipt.encode().map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let receipt_rent_debit_lamports = rent.minimum_balance(SOURCE_WORK_RECEIPT_ACCOUNT_BYTES)
        .saturating_sub(receipt_prefund);
    let receipt_balance_after = receipt_prefund.checked_add(receipt_rent_debit_lamports)
        .ok_or(SourceAction11MaterialError::Arithmetic)?;
    let authenticated_receipt = authenticate_source_work_receipt_account(
        route,
        schedule,
        RuntimeAccountViewV1 {
            key: receipt_derived.address,
            owner: program_key,
            lamports: receipt_balance_after,
            executable: false,
            writable: true,
            signer: false,
            data: &receipt_bytes,
        },
        receipt_derived,
        SourceWorkReceiptAccessV1::CreatedMutable,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;

    let total_debit = projection.rent_debit_lamports.checked_add(receipt_rent_debit_lamports)
        .and_then(|value| value.checked_add(ceiling))
        .ok_or(SourceAction11MaterialError::Arithmetic)?;
    if custody.remaining_principal_lamports < total_debit
        || snapshot.source_funding_custody.lamports < total_debit
    {
        return Err(SourceAction11MaterialError::Funding);
    }
    let mut custody_after = custody;
    let mut physical_after = snapshot.source_funding_custody.lamports;
    custody_after = apply_custody_debit(
        custody_after, &mut physical_after, projection.rent_debit_lamports,
        custody_authentication_id, initial_custody_data_id, runtime_key(target_address),
        target_recipe.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?,
    )?;
    custody_after = apply_custody_debit(
        custody_after, &mut physical_after, receipt_rent_debit_lamports,
        custody_authentication_id, initial_custody_data_id, receipt_derived.address,
        receipt_recipe.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?,
    )?;
    let direct_payment_id = ContentId::from_bytes(hashv(&[
        b"dragons-clutch/sbf/source-custody-direct-payment/v1",
        &snapshot.keeper.address.to_bytes(),
        &ceiling.to_le_bytes(),
    ]));
    custody_after = apply_custody_debit(
        custody_after, &mut physical_after, ceiling, custody_authentication_id,
        initial_custody_data_id, runtime_key(snapshot.keeper.address), direct_payment_id,
    )?;
    let keeper_balance_after_lamports = snapshot.keeper.lamports.checked_add(ceiling)
        .ok_or(SourceAction11MaterialError::Arithmetic)?;
    let custody_after_bytes = custody_after.encode().map_err(|_| SourceAction11MaterialError::Funding)?;
    let custody_post_data_id = account_data_id(
        runtime_key(snapshot.source_funding_custody.address), &custody_after_bytes,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    if authenticated_receipt.receipt().id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?
        != receipt.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?
    {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }
    let valid_before_slot = snapshot.source_release.provenance.slot
        .checked_add(SOURCE_ACTION11_VALIDITY_SLOTS_V1)
        .ok_or(SourceAction11MaterialError::Arithmetic)?;
    let ordered_accounts = ordered_action11_accounts([
        snapshot.source_release.address,
        snapshot.adapter_program.address,
        snapshot.adapter_program_data.address,
        snapshot.parser_program.address,
        snapshot.parser_program_data.address,
        snapshot.parser_config.address,
        snapshot.source_spec.address,
        snapshot.source_work_schedule.address,
        snapshot.generation_authority.address,
        target_address,
        snapshot.generation_lineage.address,
        receipt_address,
        snapshot.keeper.address,
        snapshot.source_funding_custody.address,
        SYSTEM_PROGRAM_ID,
        RENT_SYSVAR_ID,
    ])?;
    Ok(ChainDerivedSourceAction11MaterialV1 {
        checked_release_key: release.key(),
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        program_id,
        program_data: release.program_data,
        family,
        call_ordinal,
        source_release_manifest_id: route.release_manifest_id().bytes(),
        lineage_state_before_id: lineage.account_data_id().bytes(),
        semantic_binding_id: target_recipe.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?.bytes(),
        target_body_id: target.body_id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?.bytes(),
        target_post_data_id: projection.account_data_id.bytes(),
        lineage_state_after_id: lineage_state_after_id.bytes(),
        work_receipt_id: receipt.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?.bytes(),
        custody_post_data_id: custody_post_data_id.bytes(),
        target_rent_debit_lamports: projection.rent_debit_lamports,
        receipt_rent_debit_lamports,
        keeper_payment_lamports: ceiling,
        custody_balance_before_lamports: snapshot.source_funding_custody.lamports,
        custody_balance_after_lamports: physical_after,
        custody_principal_before_lamports: custody.remaining_principal_lamports,
        custody_principal_after_lamports: custody_after.remaining_principal_lamports,
        keeper_balance_before_lamports: snapshot.keeper.lamports,
        keeper_balance_after_lamports,
        valid_before_slot,
        keeper: snapshot.keeper.address,
        ordered_accounts,
    })
}

#[derive(Clone, Debug)]
struct TargetProjectionV1 {
    header: RuntimeAccountHeaderV1,
    account_data_id: ContentId,
    lineage_after: ReopenLineageV1,
    rent_debit_lamports: u64,
}

#[allow(clippy::too_many_arguments)]
fn project_target<T: RuntimeAccountBodyV1>(
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    family: LineageFamilyV1,
    recipe: &PdaRecipeV3,
    derived: RuntimeDerivedPdaV1,
    target: Address,
    balance_before: u64,
    rent: Rent,
    rent_data_id: ContentId,
    custody: Address,
    body: &T,
) -> Result<TargetProjectionV1> {
    let recipe_id = recipe.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let reopen = authorize_reopen(
        route, lineage, family, recipe_id, recipe_id, runtime_key(target), derived,
    ).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let space = RUNTIME_ACCOUNT_HEADER_BYTES.checked_add(T::ENCODED_LEN)
        .ok_or(SourceAction11MaterialError::Arithmetic)?;
    let minimum = rent.minimum_balance(space);
    let debit = minimum.saturating_sub(balance_before);
    let balance_after = balance_before.checked_add(debit)
        .ok_or(SourceAction11MaterialError::Arithmetic)?;
    let funding = plan_source_account_creation(
        route,
        reopen,
        RentExemptionQuoteV1 {
            rent_sysvar_id: rent_data_id,
            account: runtime_key(target),
            data_len: u32::try_from(space).map_err(|_| SourceAction11MaterialError::Arithmetic)?,
            minimum_balance_lamports: minimum,
        },
        runtime_key(custody),
        balance_before,
        debit,
        balance_after,
    ).map_err(|_| SourceAction11MaterialError::Funding)?;
    let header = RuntimeAccountHeaderV1 {
        family: T::FAMILY,
        bump: derived.bump,
        principal_recipient: funding.ledger.principal_recipient,
        payer_principal_lamports: funding.ledger.payer_principal_lamports,
        donation_floor_lamports: funding.ledger.donation_lamports,
        generation: funding.ledger.generation,
    };
    let mut postimage = vec![0_u8; space];
    encode_runtime_account(header, body, route.neutral_sink(), &mut postimage)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let data_id = account_data_id(runtime_key(target), &postimage)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let lineage_after = open_lineage_generation(lineage.lineage(), reopen, data_id)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    Ok(TargetProjectionV1 { header, account_data_id: data_id, lineage_after, rent_debit_lamports: debit })
}

fn apply_custody_debit(
    ledger: SourceFundingCustodyLedgerV1,
    physical_balance: &mut u64,
    debit: u64,
    custody_authentication_id: ContentId,
    initial_custody_data_id: ContentId,
    counterparty: RuntimeKey,
    semantic_id: ContentId,
) -> Result<SourceFundingCustodyLedgerV1> {
    if debit == 0 { return Ok(ledger); }
    *physical_balance = physical_balance.checked_sub(debit)
        .ok_or(SourceAction11MaterialError::Funding)?;
    let transition_id = ContentId::from_bytes(hashv(&[
        SOURCE_FUNDING_CUSTODY_PHYSICAL_TRANSITION_DOMAIN_V1,
        &custody_authentication_id.bytes(),
        &initial_custody_data_id.bytes(),
        &counterparty.bytes(),
        &debit.to_le_bytes(),
        &0_u64.to_le_bytes(),
        &semantic_id.bytes(),
    ]));
    ledger.transition(debit, 0, *physical_balance, transition_id)
        .map_err(|_| SourceAction11MaterialError::Funding)
}

fn custody_authentication_id(
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    account: &ObservedRpcAccount,
    data_id: ContentId,
    ledger: SourceFundingCustodyLedgerV1,
) -> Result<ContentId> {
    let ledger_id = ledger.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let id = ContentId::from_bytes(hashv(&[
        SOURCE_FUNDING_CUSTODY_AUTH_DOMAIN_V1,
        &route.route_id().bytes(),
        &schedule.source_work_schedule_id().bytes(),
        &schedule.lifecycle_id().bytes(),
        &account.address.to_bytes(),
        &data_id.bytes(),
        &ledger_id.bytes(),
        &account.lamports.to_le_bytes(),
    ]));
    if id.is_zero() { Err(SourceAction11MaterialError::ChainAuthority) } else { Ok(id) }
}

fn authenticate_schedule(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    account: &ObservedRpcAccount,
) -> Result<SourceWorkScheduleBindingV1> {
    if account.owner != program_id || account.executable || account.data.len() != SOURCE_WORK_SCHEDULE_BYTES {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }
    let schedule = SourceWorkScheduleBindingV1::decode(&account.data)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    schedule.validate_against(route).map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let id = schedule.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let (expected, _) = Address::find_program_address(
        &[SEED_PRODUCT_ARTIFACT_V1, &[ArtifactKind::SourceWorkScheduleV1.byte()], &id.bytes()],
        &program_id,
    );
    if account.address != expected { return Err(SourceAction11MaterialError::ChainAuthority); }
    Ok(schedule)
}

fn authenticate_custody(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    account: &ObservedRpcAccount,
) -> Result<SourceFundingCustodyLedgerV1> {
    let (expected, _) = Address::find_program_address(
        &[SEED_SOURCE_FUNDING_CUSTODY_V1, &schedule.lifecycle_id().bytes()], &program_id,
    );
    if account.address != expected || account.owner != program_id || account.executable
        || account.data.len() != SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
    {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }
    let ledger = SourceFundingCustodyLedgerV1::decode(&account.data)
        .map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let explained = ledger.remaining_principal_lamports.checked_add(ledger.donation_lamports)
        .ok_or(SourceAction11MaterialError::Arithmetic)?;
    if !ledger.is_live() || schedule.payer().bytes() != account.address.to_bytes()
        || ledger.adapter_program.bytes() != program_id.to_bytes()
        || ledger.release_manifest_id != route.release_manifest_id()
        || ledger.route_id != route.route_id()
        || ledger.source_work_schedule_id != schedule.source_work_schedule_id()
        || ledger.lifecycle_id != schedule.lifecycle_id()
        || ledger.custody_account.bytes() != account.address.to_bytes()
        || ledger.neutral_sink != route.neutral_sink() || account.lamports < explained
    {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }
    Ok(ledger)
}

fn authenticate_rent(account: &ObservedRpcAccount) -> Result<Rent> {
    if account.address != RENT_SYSVAR_ID || account.owner != SYSVAR_OWNER_ID
        || account.executable || account.data.len() != RENT_SYSVAR_BYTES_V1
    {
        return Err(SourceAction11MaterialError::ChainAuthority);
    }
    bincode::deserialize(&account.data).map_err(|_| SourceAction11MaterialError::ChainAuthority)
}

fn authenticate_release_shape(release: &IndexedProgramRelease) -> Result<()> {
    release.validate().map_err(|_| SourceAction11MaterialError::CheckedRelease)?;
    if !release.families.contains(&CanonicalFamily::Source) {
        return Err(SourceAction11MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_snapshot_provenance(
    release: &IndexedProgramRelease,
    snapshot: SourceAction11ChainSnapshotV1<'_>,
) -> Result<()> {
    let first = &snapshot.source_release.provenance;
    let release_key = release.key();
    if first.slot == 0 || first.commitment != RpcCommitment::Finalized
        || first.release_key.as_str() != release_key.as_str()
    {
        return Err(SourceAction11MaterialError::ChainSnapshot);
    }
    let accounts = [
        snapshot.source_release, snapshot.adapter_program, snapshot.adapter_program_data,
        snapshot.parser_program, snapshot.parser_program_data, snapshot.parser_config,
        snapshot.source_spec, snapshot.source_work_schedule, snapshot.generation_authority,
        snapshot.generation_lineage, snapshot.keeper, snapshot.source_funding_custody,
        snapshot.rent_sysvar,
    ];
    if accounts.iter().any(|account| !same_provenance(first, &account.provenance))
        || !same_provenance(first, snapshot.generation_target.provenance())
        || !same_provenance(first, snapshot.source_work_receipt.provenance())
    {
        return Err(SourceAction11MaterialError::ChainSnapshot);
    }
    Ok(())
}

fn same_provenance(first: &RpcObservationProvenance, other: &RpcObservationProvenance) -> bool {
    other.commitment == RpcCommitment::Finalized && other.slot == first.slot
        && other.cluster_key == first.cluster_key && other.release_key == first.release_key
}

fn ordered_action11_accounts(addresses: [Address; 16]) -> Result<Vec<AccountMeta>> {
    let action = SourceSeriesAction::ReopenGeneration;
    let contract = account_contract_v2(action);
    if contract.len() != addresses.len() { return Err(SourceAction11MaterialError::Construction); }
    let mut accounts = Vec::with_capacity(addresses.len());
    let mut observed = Vec::with_capacity(addresses.len());
    for (index, pubkey) in addresses.into_iter().enumerate() {
        let expected = contract.meta(index).ok_or(SourceAction11MaterialError::Construction)?;
        accounts.push(AccountMeta { pubkey, is_signer: expected.signer, is_writable: expected.writable });
        observed.push(ObservedSourceAccountMetaV2 { signer: expected.signer, writable: expected.writable });
    }
    validate_account_metas_v2(action, &observed).map_err(|_| SourceAction11MaterialError::Construction)?;
    Ok(accounts)
}

const fn mutable_family(family: SourceReopenFamilyV1) -> SourceMutableFamilyV2 {
    match family {
        SourceReopenFamilyV1::SourceHead => SourceMutableFamilyV2::SourceHead,
        SourceReopenFamilyV1::OpenRawPage => SourceMutableFamilyV2::OpenRawPage,
        SourceReopenFamilyV1::WindowWork => SourceMutableFamilyV2::WindowWork,
        SourceReopenFamilyV1::StatisticResult => SourceMutableFamilyV2::StatisticResult,
    }
}

fn account_view<'a>(account: &'a ObservedRpcAccount, writable: bool, signer: bool) -> RuntimeAccountViewV1<'a> {
    RuntimeAccountViewV1 {
        key: runtime_key(account.address), owner: runtime_key(account.owner), lamports: account.lamports,
        executable: account.executable, writable, signer, data: &account.data,
    }
}

fn derive_recipe(program_id: Address, recipe: PdaRecipeV3) -> Result<RuntimeDerivedPdaV1> {
    recipe.validate().map_err(|_| SourceAction11MaterialError::ChainAuthority)?;
    let mut seeds = Vec::with_capacity(usize::from(recipe.seed_count()));
    for index in 0..usize::from(recipe.seed_count()) {
        seeds.push(recipe.seed(index).map_err(|_| SourceAction11MaterialError::ChainAuthority)?);
    }
    let (derived, bump) = Address::find_program_address(&seeds, &program_id);
    Ok(RuntimeDerivedPdaV1 {
        program_id: runtime_key(program_id),
        recipe_id: recipe.id().map_err(|_| SourceAction11MaterialError::ChainAuthority)?,
        address: runtime_key(derived),
        bump,
    })
}

fn hashv(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    for part in parts { hasher.update(part); }
    hasher.finalize().into()
}

fn runtime_key(address: Address) -> RuntimeKey { RuntimeKey::from_bytes(address.to_bytes()) }
fn address(key: RuntimeKey) -> Address { Address::new_from_array(key.bytes()) }

fn map_construction(_: ConstructionError) -> SourceAction11MaterialError {
    SourceAction11MaterialError::Construction
}
