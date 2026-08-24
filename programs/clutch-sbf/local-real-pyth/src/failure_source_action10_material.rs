//! Chain-derived unsigned material for current Source action 10.
//!
//! This module has no browser request type. It consumes complete hostile RPC
//! account bodies plus one checked release and reconstructs the exact Failure
//! handoff, work-receipt postimage, liveness debit, and ordered account frame.
//! Construction remains unavailable until that release explicitly contains
//! the `(77, 2, 10)` tuple.

use crate::rpc_index::{
    CanonicalFamily, CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount,
    ObservedRpcAccountRemoval, RpcAccountRemovalKind, RpcCommitment,
};
use crate::transaction_builder::{
    ConstructionError, ExactEquation, IntegerUnit, OwnedInstructionDraft, SemanticOwner,
};
use clutch_failure_policy_runtime::market_policy_v1::FailureMarketAdmissionStateV1;
use clutch_liveness::runtime_adapter_v1::{
    plan_runtime_transition_v1, RuntimePersistedAccountViewV1, RuntimeReceiptKindV1,
    RuntimeReceiptObservationV1, RuntimeTransitionActionV1, RuntimeTransitionIntentV1,
};
use clutch_liveness::runtime_v1::{
    RuntimeCompartmentKindV1, RuntimeCompartmentPhaseV1, RuntimeCompartmentV1,
    RuntimeLivenessPolicyV1, RUNTIME_LIVENESS_ACCOUNT_BYTES_V1,
    RUNTIME_LIVENESS_POLICY_BYTES_V1,
};
use clutch_liveness::Id as LivenessId;
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::failure_recovery::{
    FailureMarketRootAccountV2, FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2,
};
use clutch_solana_layout::registry::{ExtensionFamily, SourceSeriesAction};
use clutch_solana_layout::source_series::{
    account_contract_v2, validate_account_metas_v2, EmitFailureHandoffIntentV2,
    ObservedSourceAccountMetaV2, SourceHandoffKindV2, EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2,
};
use clutch_solana_layout::Intent;
use clutch_source_plane_v3::{
    ContentId, FixedCodec, StatisticKeyV3, StatisticResultStatusV3, WindowSpecV3,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{
    authenticate_persisted_source_policy_handoff,
    authenticate_persisted_statistic_result_account, authenticate_persisted_window_evidence,
    authenticate_reopen_lineage_account, authenticate_source_release_account,
    authenticate_source_route, authenticate_source_work_receipt_account,
    authenticate_statistic_result_absence, authenticate_window_seal_account,
    join_source_occurrence, source_occurrence_record_id, source_runtime_liveness_policy_id_v1,
    AuthenticatedClockBucketV1, AuthenticatedSourceRouteV1, ClockSnapshotV1,
    FailurePolicySourceHandoffV1, LineageAccessV1, OccurrenceDispositionV1,
    ReopenLineageV1, RuntimeAccountViewV1, RuntimeDerivedPdaV1, RuntimeKey,
    SourceFundingCustodyLedgerV1, SourcePolicyHandoffAccessV1, SourcePolicyHandoffAccountV1,
    SourcePolicyHandoffJoinV1, SourceWorkAuthorizationV1, SourceWorkKindV1,
    SourceWorkReceiptAccessV1, SourceWorkReceiptAccountV1, SourceWorkScheduleBindingV1,
    SuccessfulEvaluationHandoffV1, SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES,
    SOURCE_POLICY_HANDOFF_ACCOUNT_BYTES, SOURCE_WORK_RECEIPT_ACCOUNT_BYTES,
    SOURCE_WORK_SCHEDULE_BYTES,
};
use solana_address::Address;
use solana_instruction::AccountMeta;
use solana_rent::Rent;

/// Exact operator-side validity horizon. It is a replay-safety bound, not a
/// protocol timing promise and not a browser-selected expiry.
pub const FAILURE_SOURCE_ACTION10_VALIDITY_SLOTS_V1: u64 = 32;
/// Exact Source family selected by the current action-10 handler.
pub const FAILURE_SOURCE_ACTION10_FAMILY_V1: ExtensionFamily = ExtensionFamily::SourceSeries;
/// Exact current Source local action.
pub const FAILURE_SOURCE_ACTION10_LOCAL_ACTION_V1: u8 = 10;

const SOURCE_ACTION10_OWNER_SCHEMA_V1: &str =
    "dragons-clutch/operator/failure-source-action10-material/v1";
const SOURCE_ACTION10_OWNER_PACKAGE_V1: &str =
    "clutch-source-plane-v3-runtime+clutch-failure-policy-runtime";

const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);
const CLOCK_SYSVAR_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184,
    163, 155, 75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
const RENT_SYSVAR_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238,
    8, 155, 161, 253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
]);
const SYSVAR_OWNER_ID: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 117, 247, 41, 199, 61, 147, 64, 143, 33, 97, 32, 6, 126, 216,
    140, 118, 224, 140, 40, 127, 193, 148, 96, 0, 0, 0, 0,
]);

const SEED_PRODUCT_ARTIFACT_V1: &[u8] = b"dc:product-artifact:v1";
const SEED_SOURCE_OCCURRENCE_V1: &[u8] = b"dc:source-occurrence:v1";
const SEED_SOURCE_LIVENESS_POLICY_V1: &[u8] = b"dc:source-live-policy:v1";
const SEED_SOURCE_FUNDING_CUSTODY_V1: &[u8] = b"dc:source-funding:v1";
const SEED_SOURCE_COMPARTMENT_V1: &[u8] = b"dc:source-compartment:v1";
const SEED_FAILURE_MARKET_ROOT_V2: &[u8] = b"dc:failure-market-root:v2";

const CLOCK_SYSVAR_BYTES_V1: usize = 40;
const CLOCK_UNIX_TIMESTAMP_OFFSET_V1: usize = 32;
const RENT_SYSVAR_BYTES_V1: usize = 17;

/// Result returned by the current Source action-10 material boundary.
pub type FailureSourceAction10MaterialResult<T> =
    core::result::Result<T, FailureSourceAction10MaterialError>;

type Result<T> = FailureSourceAction10MaterialResult<T>;

/// Fail-closed operator refusal. The onchain adapter independently repeats all
/// owner/PDA/body/privilege checks when the unsigned instruction is executed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureSourceAction10MaterialError {
    /// Checked release identity or exact capability tuple is absent.
    CheckedRelease,
    /// RPC accounts do not come from one finalized chain snapshot.
    ChainSnapshot,
    /// One hostile account body, owner, PDA, or semantic join refused.
    ChainAuthority,
    /// A predictable account is neither vacant nor an exact permitted postimage.
    AccountOccupancy,
    /// Current liveness or prepaid custody cannot fund the complete call.
    Funding,
    /// Exact arithmetic overflowed.
    Arithmetic,
    /// Canonical layout or outer construction refused.
    Construction,
}

impl core::fmt::Display for FailureSourceAction10MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not admit Source action 10",
            Self::ChainSnapshot => "accounts are not one finalized chain snapshot",
            Self::ChainAuthority => "hostile chain authority failed authentication",
            Self::AccountOccupancy => "predictable account has a noncanonical occupant",
            Self::Funding => "prepaid Source funding cannot cover action 10",
            Self::Arithmetic => "action-10 material arithmetic overflowed",
            Self::Construction => "canonical action-10 construction refused",
        })
    }
}

impl std::error::Error for FailureSourceAction10MaterialError {}

/// Raw current observation of one predictable Source slot. Existing evidence
/// slots may be program-owned; callers which require allocation invoke the
/// private vacant-shape validator below.
#[derive(Clone, Copy, Debug)]
pub enum ObservedSourceSlotV1<'a> {
    /// Present account bytes from the chain snapshot.
    Present(&'a ObservedRpcAccount),
    /// Finalized closed observation.
    Removed(&'a ObservedRpcAccountRemoval),
}

impl ObservedSourceSlotV1<'_> {
    fn address(self) -> Address {
        match self {
            Self::Present(account) => account.address,
            Self::Removed(account) => account.address,
        }
    }

    fn lamports(self) -> u64 {
        match self {
            Self::Present(account) => account.lamports,
            Self::Removed(_) => 0,
        }
    }

    fn provenance(self) -> &crate::rpc_index::RpcObservationProvenance {
        match self {
            Self::Present(account) => &account.provenance,
            Self::Removed(account) => &account.provenance,
        }
    }

    fn validate(self, expected: Address, require_zero_lamports: bool) -> Result<u64> {
        if self.address() != expected {
            return Err(FailureSourceAction10MaterialError::AccountOccupancy);
        }
        match self {
            Self::Present(account) => {
                if account.owner != SYSTEM_PROGRAM_ID
                    || account.executable
                    || !account.data.is_empty()
                    || require_zero_lamports && account.lamports != 0
                {
                    return Err(FailureSourceAction10MaterialError::AccountOccupancy);
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
                    return Err(FailureSourceAction10MaterialError::AccountOccupancy);
                }
                Ok(0)
            }
        }
    }
}

/// Complete raw chain snapshot needed to derive action 10. Every semantic ID
/// is recomputed from these bodies; none is an operator or browser argument.
#[derive(Clone, Copy, Debug)]
pub struct FailureSourceAction10ChainSnapshotV1<'a> {
    pub source_release: &'a ObservedRpcAccount,
    pub adapter_program: &'a ObservedRpcAccount,
    pub adapter_program_data: &'a ObservedRpcAccount,
    pub parser_program: &'a ObservedRpcAccount,
    pub parser_program_data: &'a ObservedRpcAccount,
    pub parser_config: &'a ObservedRpcAccount,
    pub source_spec: &'a ObservedRpcAccount,
    pub source_work_schedule: &'a ObservedRpcAccount,
    pub clock_sysvar: &'a ObservedRpcAccount,
    pub source_occurrence: &'a ObservedRpcAccount,
    pub window_spec: &'a ObservedRpcAccount,
    pub statistic_key: &'a ObservedRpcAccount,
    pub window_seal: ObservedSourceSlotV1<'a>,
    pub statistic_result: ObservedSourceSlotV1<'a>,
    pub result_lineage: &'a ObservedRpcAccount,
    pub source_work_receipt: ObservedSourceSlotV1<'a>,
    pub failure_market_root: &'a ObservedRpcAccount,
    pub source_handoff_receipt: ObservedSourceSlotV1<'a>,
    pub liveness_policy: &'a ObservedRpcAccount,
    pub source_compartment: &'a ObservedRpcAccount,
    pub source_funding_custody: &'a ObservedRpcAccount,
    pub rent_sysvar: &'a ObservedRpcAccount,
}

/// Opaque, chain-derived action material. Like every static client projection,
/// it remains untrusted by the program. It contains no signer and cannot
/// become an instruction until a checked release admits the exact tuple.
#[derive(Clone, Debug)]
pub struct ChainDerivedFailureSourceAction10MaterialV1 {
    checked_release_key: String,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    program_id: Address,
    program_data: Address,
    call_ordinal: u32,
    kind: SourceHandoffKindV2,
    handoff_id: [u8; 32],
    source_work_receipt_authentication_id: [u8; 32],
    source_work_receipt_account: Address,
    source_handoff_receipt_account: Address,
    source_handoff_post_data_id: [u8; 32],
    valid_before_slot: u64,
    keeper_payment_lamports: u64,
    liveness_work_before_lamports: u64,
    liveness_work_after_lamports: u64,
    custody_principal_before_lamports: u64,
    custody_principal_after_lamports: u64,
    custody_rent_debit_lamports: u64,
    ordered_accounts: Vec<AccountMeta>,
    keeper: Address,
}

impl ChainDerivedFailureSourceAction10MaterialV1 {
    /// Canonical branch selected from exact WindowSeal/StatisticResult state.
    pub const fn kind(&self) -> SourceHandoffKindV2 {
        self.kind
    }

    /// Exact monotone Source work-call ordinal derived from liveness state.
    pub const fn call_ordinal(&self) -> u32 {
        self.call_ordinal
    }

    /// Complete source-only semantic handoff identity.
    pub const fn handoff_id(&self) -> [u8; 32] {
        self.handoff_id
    }

    /// Exact created work-receipt account.
    pub const fn source_work_receipt_account(&self) -> Address {
        self.source_work_receipt_account
    }

    /// Exact created durable handoff account.
    pub const fn source_handoff_receipt_account(&self) -> Address {
        self.source_handoff_receipt_account
    }

    /// Digest of the exact durable handoff postimage.
    pub const fn source_handoff_post_data_id(&self) -> [u8; 32] {
        self.source_handoff_post_data_id
    }

    /// Construct one unsigned action only after the checked release admits
    /// `(77, 2, 10)`. No signature or submission facility is exposed here.
    pub fn unsigned_instruction(
        &self,
        release: &IndexedProgramRelease,
    ) -> Result<OwnedInstructionDraft> {
        let coordinate = CanonicalIntentCoordinate {
            family_tag: FAILURE_SOURCE_ACTION10_FAMILY_V1.tag(),
            family_version: FAILURE_SOURCE_ACTION10_FAMILY_V1.version(),
            local_action: FAILURE_SOURCE_ACTION10_LOCAL_ACTION_V1,
        };
        if release.key() != self.checked_release_key
            || release.program_id != self.program_id
            || release.program_data != self.program_data
            || release.release_manifest_sha256 != self.release_manifest_sha256
            || release.capability_profile_id != self.capability_profile_id
            || release.enabled_intents.binary_search(&coordinate).is_err()
        {
            return Err(FailureSourceAction10MaterialError::CheckedRelease);
        }
        let intent = EmitFailureHandoffIntentV2 {
            kind: self.kind,
            handoff_id: self.handoff_id,
            source_work_receipt_id: self.source_work_receipt_authentication_id,
            valid_before_slot: self.valid_before_slot,
        };
        let mut payload = [0_u8; EMIT_FAILURE_HANDOFF_PAYLOAD_BYTES_V2];
        intent
            .encode(&mut payload)
            .map_err(|_| FailureSourceAction10MaterialError::Construction)?;
        let semantic_owner = SemanticOwner {
            package: SOURCE_ACTION10_OWNER_PACKAGE_V1.into(),
            schema: SOURCE_ACTION10_OWNER_SCHEMA_V1.into(),
            release_sha256: self.release_manifest_sha256,
        };
        OwnedInstructionDraft::checked_release_source_handoff(
            release,
            "emit-failure-source-handoff-v2",
            semantic_owner,
            self.ordered_accounts.clone(),
            vec![self.keeper],
            vec![
                ExactEquation {
                    name: "Source work capital funds the exact keeper payment".into(),
                    unit: IntegerUnit::Lamports,
                    left: u128::from(self.liveness_work_before_lamports),
                    right: u128::from(self.liveness_work_after_lamports)
                        + u128::from(self.keeper_payment_lamports),
                },
                ExactEquation {
                    name: "Source custody funds both immutable rent shortfalls".into(),
                    unit: IntegerUnit::Lamports,
                    left: u128::from(self.custody_principal_before_lamports),
                    right: u128::from(self.custody_principal_after_lamports)
                        + u128::from(self.custody_rent_debit_lamports),
                },
            ],
            self.call_ordinal,
            &payload,
        )
        .map_err(map_construction)
    }
}

/// Reconstruct exact action-10 bytes and metas from one finalized snapshot.
/// `keeper` is local operator configuration, not browser input; it is retained
/// only as the unsigned required-signer identity.
pub fn derive_failure_source_action10_material_v1(
    release: &IndexedProgramRelease,
    snapshot: FailureSourceAction10ChainSnapshotV1<'_>,
    keeper: Address,
) -> Result<ChainDerivedFailureSourceAction10MaterialV1> {
    authenticate_release_shape(release, keeper)?;
    authenticate_snapshot_provenance(release, snapshot)?;
    let program_id = release.program_id;
    let program_key = runtime_key(program_id);

    let manifest = clutch_source_plane_v3_runtime::SourceReleaseManifestV2::decode(
        &snapshot.source_release.data,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let release_recipe = PdaRecipeV3::source_release(
        manifest
            .id()
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let release_pda = derive_recipe(program_id, release_recipe)?;
    let authenticated_release = authenticate_source_release_account(
        program_key,
        account_view(snapshot.source_release, false, false),
        release_pda,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    if snapshot.adapter_program.address != release.program_id
        || snapshot.adapter_program_data.address != release.program_data
    {
        return Err(FailureSourceAction10MaterialError::CheckedRelease);
    }
    let route = authenticate_source_route(
        authenticated_release,
        account_view(snapshot.adapter_program, false, false),
        account_view(snapshot.adapter_program_data, false, false),
        account_view(snapshot.parser_program, false, false),
        account_view(snapshot.parser_program_data, false, false),
        account_view(snapshot.parser_config, false, false),
        account_view(snapshot.source_spec, false, false),
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;

    let schedule = authenticate_schedule(program_id, route, snapshot.source_work_schedule)?;
    let clock = authenticate_clock(route, snapshot.clock_sysvar)?;
    let (failure_state, failure_facts) =
        authenticate_failure_root(program_id, route, snapshot.failure_market_root)?;
    let window = authenticate_window_input(program_id, route, snapshot.window_spec)?;
    let key = authenticate_statistic_key_input(
        program_id,
        route,
        snapshot.statistic_key,
        &window,
        ContentId::from_bytes(failure_facts.summary_program_id.bytes()),
    )?;
    if failure_facts.primary_window_id.bytes()
        != window
            .id()
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?
            .bytes()
        || failure_facts.statistic_key_id.bytes()
            != key
                .id()
                .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?
                .bytes()
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    let occurrence_id = source_occurrence_record_id(&snapshot.source_occurrence.data)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let (occurrence_address, occurrence_bump) = Address::find_program_address(
        &[SEED_SOURCE_OCCURRENCE_V1, &occurrence_id.bytes()],
        &program_id,
    );
    let occurrence = join_source_occurrence(
        route,
        account_view(snapshot.source_occurrence, false, false),
        RuntimeDerivedPdaV1 {
            program_id: program_key,
            recipe_id: occurrence_id,
            address: runtime_key(occurrence_address),
            bump: occurrence_bump,
        },
        OccurrenceDispositionV1::ExactExisting,
        &window,
        &key,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    if occurrence.market_instance_id().bytes() != failure_facts.market_instance_id.bytes() {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }

    let lineage_body = ReopenLineageV1::decode(&snapshot.result_lineage.data)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let lineage_recipe = PdaRecipeV3::reopen_lineage(
        lineage_body
            .recipe_id()
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let lineage = authenticate_reopen_lineage_account(
        route,
        account_view(snapshot.result_lineage, false, false),
        derive_recipe(program_id, lineage_recipe)?,
        LineageAccessV1::ReadOnly,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;

    let failure_policy_binding_id = ContentId::from_bytes(failure_state.binding().id().bytes());
    let (kind, handoff_id, source_fact) = authenticate_source_fact(
        program_id,
        route,
        clock,
        failure_policy_binding_id,
        occurrence,
        &window,
        &key,
        ContentId::from_bytes(failure_facts.summary_program_id.bytes()),
        lineage,
        snapshot.window_seal,
        snapshot.statistic_result,
    )?;

    let (policy, compartment) = authenticate_source_liveness(
        program_id,
        route,
        schedule,
        snapshot.liveness_policy,
        snapshot.source_compartment,
    )?;
    let custody = authenticate_source_custody(
        program_id,
        route,
        schedule,
        snapshot.source_funding_custody,
    )?;
    let call_ordinal = compartment
        .completed_calls
        .checked_add(1)
        .ok_or(FailureSourceAction10MaterialError::Arithmetic)?;
    let ceiling = schedule.ceiling_for(SourceWorkKindV1::FailureHandoff);
    let receipt_slot = SourceWorkAuthorizationV1::receipt_slot_id(
        route,
        schedule,
        SourceWorkKindV1::FailureHandoff,
        call_ordinal,
        handoff_id,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let receipt_recipe = PdaRecipeV3::source_work_receipt(receipt_slot)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let receipt_pda = derive_recipe(program_id, receipt_recipe)?;
    let receipt_address = address(receipt_pda.address);
    let receipt_prefund = snapshot
        .source_work_receipt
        .validate(receipt_address, false)?;
    let work_authorization = SourceWorkAuthorizationV1::new(
        route,
        schedule,
        SourceWorkKindV1::FailureHandoff,
        receipt_pda.address,
        call_ordinal,
        ceiling,
        handoff_id,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let work_receipt = SourceWorkReceiptAccountV1::from_work(route, work_authorization)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let work_receipt_bytes = work_receipt
        .encode()
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let authenticated_work = authenticate_source_work_receipt_account(
        route,
        schedule,
        RuntimeAccountViewV1 {
            key: receipt_pda.address,
            owner: program_key,
            lamports: receipt_prefund,
            executable: false,
            writable: true,
            signer: false,
            data: &work_receipt_bytes,
        },
        receipt_pda,
        SourceWorkReceiptAccessV1::CreatedMutable,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let join = match source_fact {
        AuthenticatedAction10SourceFactV1::FailureAbsence(handoff, absence) => {
            SourcePolicyHandoffJoinV1::failure_absence(route, handoff, absence, authenticated_work)
        }
        AuthenticatedAction10SourceFactV1::FailureResult(handoff, result) => {
            SourcePolicyHandoffJoinV1::failure_result(route, handoff, result, authenticated_work)
        }
        AuthenticatedAction10SourceFactV1::Successful(handoff, result) => {
            SourcePolicyHandoffJoinV1::successful_evaluation(
                route,
                handoff,
                result,
                authenticated_work,
            )
        }
    }
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let handoff_recipe = PdaRecipeV3::source_policy_handoff(join.id())
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let handoff_pda = derive_recipe(program_id, handoff_recipe)?;
    let handoff_address = address(handoff_pda.address);
    let handoff_prefund = snapshot
        .source_handoff_receipt
        .validate(handoff_address, false)?;
    let handoff_body = SourcePolicyHandoffAccountV1::from_join(join)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let mut handoff_bytes = [0_u8; SOURCE_POLICY_HANDOFF_ACCOUNT_BYTES];
    handoff_body
        .encode_into(&mut handoff_bytes)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let authenticated_handoff = authenticate_persisted_source_policy_handoff(
        route,
        join,
        RuntimeAccountViewV1 {
            key: handoff_pda.address,
            owner: program_key,
            lamports: handoff_prefund,
            executable: false,
            writable: true,
            signer: false,
            data: &handoff_bytes,
        },
        handoff_pda,
        SourcePolicyHandoffAccessV1::CreatedMutable,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;

    let rent = authenticate_rent(snapshot.rent_sysvar)?;
    let work_rent = rent
        .minimum_balance(SOURCE_WORK_RECEIPT_ACCOUNT_BYTES)
        .saturating_sub(receipt_prefund);
    let handoff_rent = rent
        .minimum_balance(SOURCE_POLICY_HANDOFF_ACCOUNT_BYTES)
        .saturating_sub(handoff_prefund);
    let custody_rent_debit_lamports = work_rent
        .checked_add(handoff_rent)
        .ok_or(FailureSourceAction10MaterialError::Arithmetic)?;
    if custody.remaining_principal_lamports < custody_rent_debit_lamports {
        return Err(FailureSourceAction10MaterialError::Funding);
    }
    let custody_principal_after_lamports = custody
        .remaining_principal_lamports
        .checked_sub(custody_rent_debit_lamports)
        .ok_or(FailureSourceAction10MaterialError::Funding)?;
    let (liveness_work_before_lamports, liveness_work_after_lamports) =
        authenticate_liveness_transition(
            program_id,
            schedule,
            policy,
            compartment,
            snapshot.liveness_policy,
            snapshot.source_compartment,
            authenticated_work,
            keeper,
            ceiling,
        )?;

    let valid_before_slot = clock
        .snapshot()
        .slot
        .checked_add(FAILURE_SOURCE_ACTION10_VALIDITY_SLOTS_V1)
        .ok_or(FailureSourceAction10MaterialError::Arithmetic)?;
    let ordered_addresses = [
        snapshot.source_release.address,
        snapshot.adapter_program.address,
        snapshot.adapter_program_data.address,
        snapshot.parser_program.address,
        snapshot.parser_program_data.address,
        snapshot.parser_config.address,
        snapshot.source_spec.address,
        snapshot.source_work_schedule.address,
        CLOCK_SYSVAR_ID,
        snapshot.source_occurrence.address,
        snapshot.window_spec.address,
        snapshot.statistic_key.address,
        snapshot.window_seal.address(),
        snapshot.statistic_result.address(),
        snapshot.result_lineage.address,
        receipt_address,
        snapshot.failure_market_root.address,
        handoff_address,
        snapshot.liveness_policy.address,
        snapshot.source_compartment.address,
        keeper,
        snapshot.source_funding_custody.address,
        SYSTEM_PROGRAM_ID,
        RENT_SYSVAR_ID,
    ];
    let ordered_accounts = ordered_action10_accounts(ordered_addresses)?;
    Ok(ChainDerivedFailureSourceAction10MaterialV1 {
        checked_release_key: release.key(),
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        program_id,
        program_data: release.program_data,
        call_ordinal,
        kind,
        handoff_id: handoff_id.bytes(),
        source_work_receipt_authentication_id: authenticated_work.id().bytes(),
        source_work_receipt_account: receipt_address,
        source_handoff_receipt_account: handoff_address,
        source_handoff_post_data_id: authenticated_handoff.account_data_id().bytes(),
        valid_before_slot,
        keeper_payment_lamports: ceiling,
        liveness_work_before_lamports,
        liveness_work_after_lamports,
        custody_principal_before_lamports: custody.remaining_principal_lamports,
        custody_principal_after_lamports,
        custody_rent_debit_lamports,
        ordered_accounts,
        keeper,
    })
}

#[derive(Clone, Copy)]
enum AuthenticatedAction10SourceFactV1 {
    FailureAbsence(
        FailurePolicySourceHandoffV1,
        clutch_source_plane_v3_runtime::AuthenticatedStatisticResultAbsenceV1,
    ),
    FailureResult(
        FailurePolicySourceHandoffV1,
        clutch_source_plane_v3_runtime::AuthenticatedStatisticResultAccountV1,
    ),
    Successful(
        SuccessfulEvaluationHandoffV1,
        clutch_source_plane_v3_runtime::AuthenticatedStatisticResultAccountV1,
    ),
}

#[allow(clippy::too_many_arguments)]
fn authenticate_source_fact(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    clock: AuthenticatedClockBucketV1,
    failure_policy_binding_id: ContentId,
    occurrence: clutch_source_plane_v3_runtime::OccurrenceSourceReceiptV1,
    window: &WindowSpecV3,
    key: &StatisticKeyV3,
    summary_program_id: ContentId,
    lineage: clutch_source_plane_v3_runtime::AuthenticatedReopenLineageV1,
    window_seal: ObservedSourceSlotV1<'_>,
    statistic_result: ObservedSourceSlotV1<'_>,
) -> Result<(SourceHandoffKindV2, ContentId, AuthenticatedAction10SourceFactV1)> {
    let seal_recipe = PdaRecipeV3::window_seal(
        window
            .id()
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let seal_pda = derive_recipe(program_id, seal_recipe)?;
    let result_recipe = PdaRecipeV3::statistic_result(
        key.id()
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?,
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let result_pda = derive_recipe(program_id, result_recipe)?;
    match (window_seal, statistic_result) {
        (ObservedSourceSlotV1::Present(seal), ObservedSourceSlotV1::Present(result))
            if seal.owner == program_id && result.owner == program_id =>
        {
            if seal.address != address(seal_pda.address) || result.address != address(result_pda.address)
            {
                return Err(FailureSourceAction10MaterialError::ChainAuthority);
            }
            let seal = authenticate_window_seal_account(
                route,
                account_view(seal, false, false),
                seal_pda,
                window,
            )
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
            let evidence = authenticate_persisted_window_evidence(
                route,
                &route.source_plane(),
                &route.clock_policy(),
                clock.snapshot(),
                window,
                seal,
            )
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
            let result = authenticate_persisted_statistic_result_account(
                route,
                account_view(result, false, false),
                result_pda,
                window,
                key,
                summary_program_id,
                evidence,
                lineage,
            )
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
            match result.result().status() {
                StatisticResultStatusV3::Refused => {
                    let handoff = FailurePolicySourceHandoffV1::source_evaluation_refused(
                        failure_policy_binding_id,
                        occurrence,
                        &route.clock_policy(),
                        clock.snapshot(),
                        window,
                        evidence,
                        result,
                    )
                    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
                    Ok((
                        SourceHandoffKindV2::FailureResult,
                        handoff.id(),
                        AuthenticatedAction10SourceFactV1::FailureResult(handoff, result),
                    ))
                }
                StatisticResultStatusV3::Success => {
                    let handoff = SuccessfulEvaluationHandoffV1::at_maturity(
                        failure_policy_binding_id,
                        occurrence,
                        &route.clock_policy(),
                        clock.snapshot(),
                        window,
                        evidence,
                        result,
                    )
                    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
                    Ok((
                        SourceHandoffKindV2::SuccessfulEvaluation,
                        handoff.id(),
                        AuthenticatedAction10SourceFactV1::Successful(handoff, result),
                    ))
                }
            }
        }
        (seal_slot, result_slot) => {
            seal_slot.validate(address(seal_pda.address), false)?;
            result_slot.validate(address(result_pda.address), true)?;
            let absence = authenticate_statistic_result_absence(
                route,
                key,
                RuntimeAccountViewV1 {
                    key: result_pda.address,
                    owner: runtime_key(SYSTEM_PROGRAM_ID),
                    lamports: 0,
                    executable: false,
                    writable: false,
                    signer: false,
                    data: &[],
                },
                result_pda,
                lineage,
            )
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
            let handoff = FailurePolicySourceHandoffV1::primary_maturity_without_resolution(
                failure_policy_binding_id,
                occurrence,
                &route.clock_policy(),
                clock.snapshot(),
                window,
                absence,
            )
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
            Ok((
                SourceHandoffKindV2::FailureAbsence,
                handoff.id(),
                AuthenticatedAction10SourceFactV1::FailureAbsence(handoff, absence),
            ))
        }
    }
}

fn authenticate_release_shape(release: &IndexedProgramRelease, keeper: Address) -> Result<()> {
    release
        .validate()
        .map_err(|_| FailureSourceAction10MaterialError::CheckedRelease)?;
    if keeper == Address::default()
        || !release.families.contains(&CanonicalFamily::Source)
        || !release.families.contains(&CanonicalFamily::Failure)
    {
        return Err(FailureSourceAction10MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_snapshot_provenance(
    release: &IndexedProgramRelease,
    snapshot: FailureSourceAction10ChainSnapshotV1<'_>,
) -> Result<()> {
    let first = &snapshot.source_release.provenance;
    let release_key = release.key();
    if first.commitment != RpcCommitment::Finalized
        || first.release_key.as_str() != release_key.as_str()
    {
        return Err(FailureSourceAction10MaterialError::ChainSnapshot);
    }
    let mut provenances = vec![
        &snapshot.source_release.provenance,
        &snapshot.adapter_program.provenance,
        &snapshot.adapter_program_data.provenance,
        &snapshot.parser_program.provenance,
        &snapshot.parser_program_data.provenance,
        &snapshot.parser_config.provenance,
        &snapshot.source_spec.provenance,
        &snapshot.source_work_schedule.provenance,
        &snapshot.clock_sysvar.provenance,
        &snapshot.source_occurrence.provenance,
        &snapshot.window_spec.provenance,
        &snapshot.statistic_key.provenance,
        snapshot.window_seal.provenance(),
        snapshot.statistic_result.provenance(),
        &snapshot.result_lineage.provenance,
        snapshot.source_work_receipt.provenance(),
        &snapshot.failure_market_root.provenance,
        snapshot.source_handoff_receipt.provenance(),
        &snapshot.liveness_policy.provenance,
        &snapshot.source_compartment.provenance,
        &snapshot.source_funding_custody.provenance,
        &snapshot.rent_sysvar.provenance,
    ];
    if provenances.iter().any(|provenance| {
        provenance.cluster_key != first.cluster_key
            || provenance.slot != first.slot
            || provenance.commitment != RpcCommitment::Finalized
            || provenance.release_key.as_str() != release_key.as_str()
    }) {
        return Err(FailureSourceAction10MaterialError::ChainSnapshot);
    }
    for account in [
        snapshot.source_release,
        snapshot.source_work_schedule,
        snapshot.source_occurrence,
        snapshot.window_spec,
        snapshot.statistic_key,
        snapshot.result_lineage,
        snapshot.failure_market_root,
        snapshot.liveness_policy,
        snapshot.source_compartment,
        snapshot.source_funding_custody,
    ] {
        if account.owner != release.program_id
            || account.provenance.release_key.as_str() != release_key.as_str()
        {
            return Err(FailureSourceAction10MaterialError::ChainSnapshot);
        }
    }
    provenances.clear();
    Ok(())
}

fn authenticate_schedule(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    account: &ObservedRpcAccount,
) -> Result<SourceWorkScheduleBindingV1> {
    if account.owner != program_id
        || account.executable
        || account.data.len() != SOURCE_WORK_SCHEDULE_BYTES
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    let schedule = SourceWorkScheduleBindingV1::decode(&account.data)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let schedule_id = schedule
        .id()
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let (expected, _) = Address::find_program_address(
        &[
            SEED_PRODUCT_ARTIFACT_V1,
            &[ArtifactKind::SourceWorkScheduleV1.byte()],
            &schedule_id.bytes(),
        ],
        &program_id,
    );
    if account.address != expected
        || schedule_id != route.source_work_schedule_id()
        || schedule.liveness_policy_id() != route.liveness_policy_id()
        || schedule.source_compartment_account() != route.source_compartment_account()
        || schedule.source_compartment_owner() != route.source_compartment_owner()
        || schedule.receipt_account_owner_program() != route.adapter_program()
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    Ok(schedule)
}

fn authenticate_clock(
    route: AuthenticatedSourceRouteV1,
    account: &ObservedRpcAccount,
) -> Result<AuthenticatedClockBucketV1> {
    if account.address != CLOCK_SYSVAR_ID
        || account.owner != SYSVAR_OWNER_ID
        || account.executable
        || account.data.len() != CLOCK_SYSVAR_BYTES_V1
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    let slot = le_u64(&account.data[..8]);
    let unix_signed = i64::from_le_bytes(
        account.data[CLOCK_UNIX_TIMESTAMP_OFFSET_V1..CLOCK_UNIX_TIMESTAMP_OFFSET_V1 + 8]
            .try_into()
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?,
    );
    let unix_timestamp = u64::try_from(unix_signed)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    AuthenticatedClockBucketV1::from_snapshot(
        &route.clock_policy(),
        ClockSnapshotV1 {
            slot,
            unix_timestamp,
        },
    )
    .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)
}

fn authenticate_failure_root(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    account: &ObservedRpcAccount,
) -> Result<(
    FailureMarketAdmissionStateV1,
    clutch_failure_policy_runtime::market_policy_v1::FailureMarketPolicyFactsV1,
)> {
    if account.owner != program_id || account.executable {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    let bytes: &[u8; FAILURE_MARKET_ROOT_ACCOUNT_BYTES_V2] = account
        .data
        .as_slice()
        .try_into()
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let root = FailureMarketRootAccountV2::decode(bytes)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let state = FailureMarketAdmissionStateV1::decode(&root.admission_body)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let facts = state.binding().facts();
    let root_funding = state.root_funding().facts();
    let (expected, bump) = Address::find_program_address(
        &[
            SEED_FAILURE_MARKET_ROOT_V2,
            &facts.market_instance_id.bytes(),
            &facts.generation.to_le_bytes(),
        ],
        &program_id,
    );
    if account.address != expected
        || root.bump != bump
        || root_funding.root_account_id.bytes() != account.address.to_bytes()
        || account.lamports < root_funding.observed_balance_lamports
        || facts.recovery_receipt_program_id.bytes() != program_id.to_bytes()
        || facts.source_release_manifest_id.bytes() != route.release_manifest_id().bytes()
        || facts.source_release_authentication_id.bytes()
            != route.release_authentication_id().bytes()
        || facts.source_release_account_id.bytes() != route.release_account().bytes()
        || facts.source_plane_contract_id.bytes() != route.source_plane_contract_id().bytes()
        || facts.source_spec_id.bytes() != route.source_spec_id().bytes()
        || facts.clock_policy_id.bytes() != route.clock_policy_id().bytes()
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    Ok((state, facts))
}

fn authenticate_window_input(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    account: &ObservedRpcAccount,
) -> Result<WindowSpecV3> {
    if account.owner != program_id || account.executable {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    let value = WindowSpecV3::decode(&account.data)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let id = value
        .id()
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let derived = derive_recipe(
        program_id,
        PdaRecipeV3::window_spec(id)
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?,
    )?;
    if account.address != address(derived.address)
        || value.source_spec_id != route.source_spec_id()
        || value.source_plane_program_id != route.source_plane_contract_id()
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    Ok(value)
}

fn authenticate_statistic_key_input(
    program_id: Address,
    _route: AuthenticatedSourceRouteV1,
    account: &ObservedRpcAccount,
    window: &WindowSpecV3,
    summary_program_id: ContentId,
) -> Result<StatisticKeyV3> {
    if account.owner != program_id || account.executable || summary_program_id.is_zero() {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    let value = StatisticKeyV3::decode(&account.data)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let id = value
        .id()
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let derived = derive_recipe(
        program_id,
        PdaRecipeV3::statistic_key(id)
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?,
    )?;
    if account.address != address(derived.address)
        || value.window_id
            != window
                .id()
                .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?
        || value.summary_program_id != summary_program_id
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    Ok(value)
}

fn authenticate_source_liveness(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    policy_account: &ObservedRpcAccount,
    compartment_account: &ObservedRpcAccount,
) -> Result<(RuntimeLivenessPolicyV1, RuntimeCompartmentV1)> {
    let (expected_policy, _) = Address::find_program_address(
        &[
            SEED_SOURCE_LIVENESS_POLICY_V1,
            &schedule.liveness_policy_id().bytes(),
        ],
        &program_id,
    );
    let (expected_compartment, _) = Address::find_program_address(
        &[
            SEED_SOURCE_COMPARTMENT_V1,
            &schedule.lifecycle_id().bytes(),
        ],
        &program_id,
    );
    if policy_account.address != expected_policy
        || compartment_account.address != expected_compartment
        || policy_account.owner != program_id
        || compartment_account.owner != program_id
        || policy_account.executable
        || compartment_account.executable
        || policy_account.data.len() != RUNTIME_LIVENESS_POLICY_BYTES_V1
        || compartment_account.data.len() != RUNTIME_LIVENESS_ACCOUNT_BYTES_V1
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    let policy = RuntimeLivenessPolicyV1::decode(&policy_account.data)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let compartment = RuntimeCompartmentV1::decode(&compartment_account.data)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let policy_id = source_runtime_liveness_policy_id_v1(policy)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    compartment
        .validate_against_policy(policy)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    if policy_id != route.liveness_policy_id()
        || compartment.kind != RuntimeCompartmentKindV1::Source
        || compartment.phase != RuntimeCompartmentPhaseV1::Active
        || compartment.identity.account_id.bytes() != compartment_account.address.to_bytes()
        || compartment.identity.owner.bytes() != program_id.to_bytes()
        || compartment.identity.lifecycle_id.bytes() != schedule.lifecycle_id().bytes()
        || compartment.identity.generation != schedule.generation()
        || compartment.quote_schedule_id.bytes() != schedule.source_work_schedule_id().bytes()
        || compartment.receipt_program_id.bytes() != program_id.to_bytes()
        || compartment.maximum_calls != schedule.maximum_calls()
        || compartment.maximum_lamports_per_call != schedule.maximum_lamports_per_call()
        || compartment.capitalized_work_lamports != schedule.work_capital_lamports()
        || compartment.rent_principal_lamports != schedule.rent_principal_lamports()
        || compartment_account.lamports
            < compartment
                .expected_account_balance_lamports()
                .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    Ok((policy, compartment))
}

fn authenticate_source_custody(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    account: &ObservedRpcAccount,
) -> Result<SourceFundingCustodyLedgerV1> {
    let (expected, _) = Address::find_program_address(
        &[
            SEED_SOURCE_FUNDING_CUSTODY_V1,
            &schedule.lifecycle_id().bytes(),
        ],
        &program_id,
    );
    if account.address != expected
        || account.owner != program_id
        || account.executable
        || account.data.len() != SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    let ledger = SourceFundingCustodyLedgerV1::decode(&account.data)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let explained = ledger
        .remaining_principal_lamports
        .checked_add(ledger.donation_lamports)
        .ok_or(FailureSourceAction10MaterialError::Arithmetic)?;
    if schedule.payer().bytes() != account.address.to_bytes()
        || ledger.adapter_program.bytes() != program_id.to_bytes()
        || ledger.release_manifest_id != route.release_manifest_id()
        || ledger.route_id != route.route_id()
        || ledger.source_work_schedule_id != schedule.source_work_schedule_id()
        || ledger.lifecycle_id != schedule.lifecycle_id()
        || ledger.custody_account.bytes() != account.address.to_bytes()
        || ledger.neutral_sink != route.neutral_sink()
        || account.lamports < explained
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    Ok(ledger)
}

#[allow(clippy::too_many_arguments)]
fn authenticate_liveness_transition(
    program_id: Address,
    schedule: SourceWorkScheduleBindingV1,
    _policy: RuntimeLivenessPolicyV1,
    compartment: RuntimeCompartmentV1,
    policy_account: &ObservedRpcAccount,
    compartment_account: &ObservedRpcAccount,
    work: clutch_source_plane_v3_runtime::AuthenticatedSourceWorkReceiptV1,
    keeper: Address,
    ceiling: u64,
) -> Result<(u64, u64)> {
    let receipt = work.receipt();
    let intent = RuntimeTransitionIntentV1 {
        action: RuntimeTransitionActionV1::SpendWork,
        kind: RuntimeCompartmentKindV1::Source,
        policy_id: LivenessId::from_bytes(schedule.liveness_policy_id().bytes()),
        lifecycle_id: LivenessId::from_bytes(schedule.lifecycle_id().bytes()),
        account_id: LivenessId::from_bytes(schedule.source_compartment_account().bytes()),
        semantic_owner: LivenessId::from_bytes(schedule.source_compartment_owner().bytes()),
        quote_schedule_id: LivenessId::from_bytes(schedule.source_work_schedule_id().bytes()),
        receipt_id: LivenessId::from_bytes(receipt.receipt_id().bytes()),
        keeper: LivenessId::from_bytes(keeper.to_bytes()),
        generation: schedule.generation(),
        call_ordinal: receipt.call_ordinal(),
        call_ceiling_lamports: ceiling,
        keeper_payment_lamports: ceiling,
        flags: 0,
    };
    let observation = RuntimeReceiptObservationV1 {
        receipt_account_id: LivenessId::from_bytes(work.account().bytes()),
        receipt_account_owner_program_id: LivenessId::from_bytes(program_id.to_bytes()),
        receipt_id: LivenessId::from_bytes(receipt.receipt_id().bytes()),
        receipt_kind: RuntimeReceiptKindV1::WorkCompleted,
        compartment_kind: RuntimeCompartmentKindV1::Source,
        semantic_owner: LivenessId::from_bytes(schedule.source_compartment_owner().bytes()),
        lifecycle_id: LivenessId::from_bytes(schedule.lifecycle_id().bytes()),
        quote_schedule_id: LivenessId::from_bytes(schedule.source_work_schedule_id().bytes()),
        generation: schedule.generation(),
        call_ordinal: receipt.call_ordinal(),
        call_ceiling_lamports: ceiling,
    };
    let balance_after = compartment_account
        .lamports
        .checked_sub(ceiling)
        .ok_or(FailureSourceAction10MaterialError::Funding)?;
    let transition = plan_runtime_transition_v1(
        LivenessId::from_bytes(program_id.to_bytes()),
        LivenessId::from_bytes(policy_account.address.to_bytes()),
        RuntimePersistedAccountViewV1 {
            account_id: LivenessId::from_bytes(policy_account.address.to_bytes()),
            owner_program_id: LivenessId::from_bytes(policy_account.owner.to_bytes()),
            lamports: policy_account.lamports,
            data: &policy_account.data,
            writable: false,
        },
        RuntimePersistedAccountViewV1 {
            account_id: LivenessId::from_bytes(compartment_account.address.to_bytes()),
            owner_program_id: LivenessId::from_bytes(compartment_account.owner.to_bytes()),
            lamports: compartment_account.lamports,
            data: &compartment_account.data,
            writable: true,
        },
        intent,
        Some(observation),
        balance_after,
    )
    .map_err(|_| FailureSourceAction10MaterialError::Funding)?;
    if compartment.completed_calls.checked_add(1) != Some(receipt.call_ordinal()) {
        return Err(FailureSourceAction10MaterialError::Funding);
    }
    Ok((
        transition.state_before.remaining_work_lamports,
        transition.state_after.remaining_work_lamports,
    ))
}

fn authenticate_rent(account: &ObservedRpcAccount) -> Result<Rent> {
    if account.address != RENT_SYSVAR_ID
        || account.owner != SYSVAR_OWNER_ID
        || account.executable
        || account.data.len() != RENT_SYSVAR_BYTES_V1
    {
        return Err(FailureSourceAction10MaterialError::ChainAuthority);
    }
    bincode::deserialize(&account.data)
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)
}

fn ordered_action10_accounts(addresses: [Address; 24]) -> Result<Vec<AccountMeta>> {
    let action = SourceSeriesAction::EmitFailureHandoff;
    let contract = account_contract_v2(action);
    if contract.len() != addresses.len() {
        return Err(FailureSourceAction10MaterialError::Construction);
    }
    let mut accounts = Vec::with_capacity(addresses.len());
    let mut observed = Vec::with_capacity(addresses.len());
    for (index, pubkey) in addresses.into_iter().enumerate() {
        let expected = contract
            .meta(index)
            .ok_or(FailureSourceAction10MaterialError::Construction)?;
        accounts.push(AccountMeta {
            pubkey,
            is_signer: expected.signer,
            is_writable: expected.writable,
        });
        observed.push(ObservedSourceAccountMetaV2 {
            key: pubkey.to_bytes(),
            writable: expected.writable,
            signer: expected.signer,
        });
    }
    validate_account_metas_v2(action, &observed)
        .map_err(|_| FailureSourceAction10MaterialError::Construction)?;
    Ok(accounts)
}

fn account_view<'a>(
    account: &'a ObservedRpcAccount,
    writable: bool,
    signer: bool,
) -> RuntimeAccountViewV1<'a> {
    RuntimeAccountViewV1 {
        key: runtime_key(account.address),
        owner: runtime_key(account.owner),
        lamports: account.lamports,
        executable: account.executable,
        writable,
        signer,
        data: &account.data,
    }
}

fn derive_recipe(program_id: Address, recipe: PdaRecipeV3) -> Result<RuntimeDerivedPdaV1> {
    recipe
        .validate()
        .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?;
    let mut seeds = Vec::with_capacity(usize::from(recipe.seed_count()));
    let mut index = 0_usize;
    while index < usize::from(recipe.seed_count()) {
        seeds.push(
            recipe
                .seed(index)
                .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?,
        );
        index += 1;
    }
    let (derived, bump) = Address::find_program_address(&seeds, &program_id);
    Ok(RuntimeDerivedPdaV1 {
        program_id: runtime_key(program_id),
        recipe_id: recipe
            .id()
            .map_err(|_| FailureSourceAction10MaterialError::ChainAuthority)?,
        address: runtime_key(derived),
        bump,
    })
}

fn runtime_key(address: Address) -> RuntimeKey {
    RuntimeKey::from_bytes(address.to_bytes())
}

fn address(key: RuntimeKey) -> Address {
    Address::new_from_array(key.bytes())
}

fn le_u64(bytes: &[u8]) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(bytes);
    u64::from_le_bytes(value)
}

fn map_construction(_error: ConstructionError) -> FailureSourceAction10MaterialError {
    FailureSourceAction10MaterialError::Construction
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;
    use crate::rpc_index::{RpcObservationProvenance, RpcObservationSource};

    fn provenance(slot: u64) -> RpcObservationProvenance {
        RpcObservationProvenance {
            cluster_key: "local-validator:genesis".into(),
            release_key: "checked-release".into(),
            slot,
            commitment: RpcCommitment::Finalized,
            source: RpcObservationSource::FinalizedScan,
            receive_sequence: 1,
        }
    }

    #[test]
    fn vacant_source_pda_refuses_owner_and_data_substitution() {
        let expected = Address::new_from_array([9; 32]);
        let mut account = ObservedRpcAccount {
            address: expected,
            owner: SYSTEM_PROGRAM_ID,
            lamports: 7,
            executable: false,
            rent_epoch: 0,
            data: vec![],
            provenance: provenance(12),
        };
        assert_eq!(
            ObservedSourceSlotV1::Present(&account).validate(expected, false),
            Ok(7)
        );
        account.owner = Address::new_from_array([8; 32]);
        assert_eq!(
            ObservedSourceSlotV1::Present(&account).validate(expected, false),
            Err(FailureSourceAction10MaterialError::AccountOccupancy)
        );
        account.owner = SYSTEM_PROGRAM_ID;
        account.data.push(1);
        assert_eq!(
            ObservedSourceSlotV1::Present(&account).validate(expected, false),
            Err(FailureSourceAction10MaterialError::AccountOccupancy)
        );
    }

    #[test]
    fn result_absence_refuses_prefund_even_when_creation_targets_accept_it() {
        let expected = Address::new_from_array([7; 32]);
        let account = ObservedRpcAccount {
            address: expected,
            owner: SYSTEM_PROGRAM_ID,
            lamports: 1,
            executable: false,
            rent_epoch: 0,
            data: vec![],
            provenance: provenance(14),
        };
        assert_eq!(
            ObservedSourceSlotV1::Present(&account).validate(expected, true),
            Err(FailureSourceAction10MaterialError::AccountOccupancy)
        );
    }

    #[test]
    fn action10_account_frame_refuses_aliases() {
        let mut addresses = [Address::default(); 24];
        for (index, address) in addresses.iter_mut().enumerate() {
            let mut bytes = [0_u8; 32];
            bytes[0] = u8::try_from(index + 1).unwrap();
            *address = Address::new_from_array(bytes);
        }
        addresses[23] = addresses[22];
        assert_eq!(
            ordered_action10_accounts(addresses),
            Err(FailureSourceAction10MaterialError::Construction)
        );
    }

    fn checked_release(enabled: bool) -> IndexedProgramRelease {
        IndexedProgramRelease {
            program_id: Address::new_from_array([0x41; 32]),
            program_data: Address::new_from_array([0x42; 32]),
            elf_sha256: [0x43; 32],
            deployment_slot: 1,
            release_manifest_sha256: [0x44; 32],
            capability_profile_id: [0x45; 32],
            source_commit: "0123456789abcdef0123456789abcdef01234567".into(),
            enabled_intents: enabled
                .then_some(CanonicalIntentCoordinate {
                    family_tag: FAILURE_SOURCE_ACTION10_FAMILY_V1.tag(),
                    family_version: FAILURE_SOURCE_ACTION10_FAMILY_V1.version(),
                    local_action: FAILURE_SOURCE_ACTION10_LOCAL_ACTION_V1,
                })
                .into_iter()
                .collect(),
            families: vec![CanonicalFamily::Source, CanonicalFamily::Failure],
        }
    }

    fn material(release: &IndexedProgramRelease) -> ChainDerivedFailureSourceAction10MaterialV1 {
        let mut addresses = [Address::default(); 24];
        for (index, address) in addresses.iter_mut().enumerate() {
            let mut bytes = [0_u8; 32];
            bytes[0] = u8::try_from(index + 1).unwrap();
            *address = Address::new_from_array(bytes);
        }
        addresses[20] = Address::new_from_array([0x71; 32]);
        addresses[22] = SYSTEM_PROGRAM_ID;
        addresses[23] = RENT_SYSVAR_ID;
        ChainDerivedFailureSourceAction10MaterialV1 {
            checked_release_key: release.key(),
            release_manifest_sha256: release.release_manifest_sha256,
            capability_profile_id: release.capability_profile_id,
            program_id: release.program_id,
            program_data: release.program_data,
            call_ordinal: 1,
            kind: SourceHandoffKindV2::FailureResult,
            handoff_id: [0x51; 32],
            source_work_receipt_authentication_id: [0x52; 32],
            source_work_receipt_account: addresses[15],
            source_handoff_receipt_account: addresses[17],
            source_handoff_post_data_id: [0x53; 32],
            valid_before_slot: 90,
            keeper_payment_lamports: 10,
            liveness_work_before_lamports: 30,
            liveness_work_after_lamports: 20,
            custody_principal_before_lamports: 40,
            custody_principal_after_lamports: 20,
            custody_rent_debit_lamports: 20,
            ordered_accounts: ordered_action10_accounts(addresses).unwrap(),
            keeper: addresses[20],
        }
    }

    #[test]
    fn checked_release_tuple_is_the_only_action10_callability_gate() {
        let disabled = checked_release(false);
        assert_eq!(
            material(&disabled).unsigned_instruction(&disabled),
            Err(FailureSourceAction10MaterialError::CheckedRelease)
        );
        let enabled = checked_release(true);
        let draft = material(&enabled).unsigned_instruction(&enabled).unwrap();
        assert_eq!(draft.program_id, enabled.program_id);
        assert_eq!(draft.required_signers, vec![Address::new_from_array([0x71; 32])]);
    }

    #[test]
    fn material_refuses_program_data_and_profile_identity_swaps() {
        let release = checked_release(true);
        let material = material(&release);
        let mut substituted = release.clone();
        substituted.program_data = Address::new_from_array([0x72; 32]);
        assert_eq!(
            material.unsigned_instruction(&substituted),
            Err(FailureSourceAction10MaterialError::CheckedRelease)
        );
        substituted = release.clone();
        substituted.capability_profile_id = [0x73; 32];
        assert_eq!(
            material.unsigned_instruction(&substituted),
            Err(FailureSourceAction10MaterialError::CheckedRelease)
        );
    }
}
