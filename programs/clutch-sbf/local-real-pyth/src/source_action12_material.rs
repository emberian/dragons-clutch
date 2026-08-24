//! Chain-derived unsigned material for current Source action 12.
//!
//! No browser request or generic payload DTO enters this boundary. One
//! finalized hostile account snapshot is decoded into the exact Source route,
//! schedule, terminal receipt, terminal policy, open lineage, target close
//! split, and custody postimage. The resulting material remains unsigned and
//! blockhash-free, and it remains unavailable until the checked release
//! explicitly admits `(77, 2, 12)`.
//!
//! The later whole-lifecycle custody retirement is a distinct Product-owned
//! transition. It authenticates the Retiring SeriesMarketLinkV2 and exact
//! Source occurrence before returning final principal; those accounts are not
//! silently appended to this frozen 14-account generation close.

use crate::rpc_index::{
    CanonicalFamily, CanonicalIntentCoordinate, IndexedProgramRelease, ObservedRpcAccount,
    RpcCommitment,
};
use crate::transaction_builder::{
    ConstructionError, ExactEquation, IntegerUnit, OwnedInstructionDraft, SemanticOwner,
};
use clutch_solana_layout::artifact::ArtifactKind;
use clutch_solana_layout::registry::{ExtensionFamily, SourceSeriesAction};
use clutch_solana_layout::source_series::{
    account_contract_v2, validate_account_metas_v2, CloseGenerationIntentV2,
    ObservedSourceAccountMetaV2, SourceMutableFamilyV2, CLOSE_GENERATION_PAYLOAD_BYTES_V2,
};
use clutch_source_plane_v3::{
    ContentId, FixedCodec, OpenRawPageV3, SourceHeadV3, StatisticResultV3, WindowWorkV3,
};
use clutch_source_plane_v3_adapter::PdaRecipeV3;
use clutch_source_plane_v3_runtime::{
    account_data_id, authenticate_reopen_lineage_account,
    authenticate_source_no_reopen_terminal, authenticate_source_release_account,
    authenticate_source_reopen_generation_request_before_close, authenticate_source_route,
    authenticate_source_work_receipt_account, close_lineage_generation, decode_runtime_account,
    plan_runtime_account_close_from_header, AuthenticatedReopenLineageV1,
    AuthenticatedSourceRouteV1, LineageAccessV1, LineageFamilyV1, ReopenLineageV1,
    RuntimeAccountHeaderV1, RuntimeAccountViewV1, RuntimeDerivedPdaV1, RuntimeKey,
    SourceFundingCustodyLedgerV1, SourceNoReopenTerminalAccessV1,
    SourceNoReopenTerminalV1, SourceReceiptDispositionV1, SourceReopenFamilyV1,
    SourceReopenGenerationRequestV1, SourceWorkReceiptAccessV1, SourceWorkReceiptAccountV1,
    SourceWorkScheduleBindingV1, SOURCE_FUNDING_CUSTODY_ACCOUNT_BYTES,
    SOURCE_NO_REOPEN_TERMINAL_BYTES, SOURCE_REOPEN_GENERATION_REQUEST_BYTES,
    SOURCE_WORK_RECEIPT_ACCOUNT_BYTES, SOURCE_WORK_SCHEDULE_BYTES,
};
use solana_address::Address;
use solana_instruction::AccountMeta;

/// Exact bounded operator-side validity horizon. It is not protocol time and
/// cannot substitute for the onchain Clock check.
pub const SOURCE_ACTION12_VALIDITY_SLOTS_V1: u64 = 32;
/// Exact current Source family selected by the close handler.
pub const SOURCE_ACTION12_FAMILY_V1: ExtensionFamily = ExtensionFamily::SourceSeries;
/// Exact current Source local action.
pub const SOURCE_ACTION12_LOCAL_ACTION_V1: u8 = 12;

const OWNER_SCHEMA_V1: &str = "dragons-clutch/operator/source-action12-material/v1";
const OWNER_PACKAGE_V1: &str = "clutch-source-plane-v3-runtime";
const SEED_PRODUCT_ARTIFACT_V1: &[u8] = b"dc:product-artifact:v1";
const SEED_SOURCE_FUNDING_CUSTODY_V1: &[u8] = b"dc:source-funding:v1";
const SEED_SOURCE_REOPEN_REQUEST_V1: &[u8] = b"dc-sp3-reopen-request";
const SYSTEM_PROGRAM_ID: Address = Address::new_from_array([0; 32]);

/// Construction result for the current Source terminal-generation route.
pub type SourceAction12MaterialResult<T> =
    core::result::Result<T, SourceAction12MaterialError>;

type Result<T> = SourceAction12MaterialResult<T>;

/// Fail-closed operator refusal. The program repeats every authority check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceAction12MaterialError {
    /// Checked release identity or exact capability tuple is absent.
    CheckedRelease,
    /// RPC accounts do not share one finalized release-bound snapshot.
    ChainSnapshot,
    /// One hostile owner, PDA, body, lineage, or terminal join refused.
    ChainAuthority,
    /// Rent-principal, donation, or custody conservation refused.
    Funding,
    /// Exact integer arithmetic overflowed.
    Arithmetic,
    /// Canonical account or instruction construction refused.
    Construction,
}

impl core::fmt::Display for SourceAction12MaterialError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::CheckedRelease => "checked release does not admit Source action 12",
            Self::ChainSnapshot => "Source close accounts are not one finalized snapshot",
            Self::ChainAuthority => "hostile Source close authority failed authentication",
            Self::Funding => "Source close rent or custody conservation refused",
            Self::Arithmetic => "Source action-12 arithmetic overflowed",
            Self::Construction => "canonical Source action-12 construction refused",
        })
    }
}

impl std::error::Error for SourceAction12MaterialError {}

/// Immutable terminal-policy family selected from exact account bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceAction12TerminalPolicyKindV1 {
    /// Shared Product/Failure resolution permanently forbids reopening.
    NoReopen,
    /// The release-selected generation authority already persisted a complete
    /// typed request for the next generation.
    ReopenRequest,
}

/// Complete raw finalized snapshot needed to derive Source action 12.
/// Semantic IDs and payload bytes are deliberately absent.
#[derive(Clone, Copy, Debug)]
pub struct SourceAction12ChainSnapshotV1<'a> {
    pub source_release: &'a ObservedRpcAccount,
    pub adapter_program: &'a ObservedRpcAccount,
    pub adapter_program_data: &'a ObservedRpcAccount,
    pub parser_program: &'a ObservedRpcAccount,
    pub parser_program_data: &'a ObservedRpcAccount,
    pub parser_config: &'a ObservedRpcAccount,
    pub source_spec: &'a ObservedRpcAccount,
    pub source_work_schedule: &'a ObservedRpcAccount,
    pub terminal_policy: &'a ObservedRpcAccount,
    pub generation_target: &'a ObservedRpcAccount,
    pub generation_lineage: &'a ObservedRpcAccount,
    pub terminal_work_receipt: &'a ObservedRpcAccount,
    pub source_funding_custody: &'a ObservedRpcAccount,
    pub neutral_sink: &'a ObservedRpcAccount,
}

/// Opaque, chain-derived terminal-generation material.
///
/// This static projection is untrusted by the program. It has no signer,
/// blockhash, signing hook, submission hook, or browser-shaped request.
#[derive(Clone, Debug)]
pub struct ChainDerivedSourceAction12MaterialV1 {
    checked_release_key: String,
    release_manifest_sha256: [u8; 32],
    capability_profile_id: [u8; 32],
    program_id: Address,
    program_data: Address,
    family: SourceMutableFamilyV2,
    terminal_policy_kind: SourceAction12TerminalPolicyKindV1,
    source_release_manifest_id: [u8; 32],
    lineage_state_before_id: [u8; 32],
    lineage_state_after_id: [u8; 32],
    semantic_terminal_receipt_id: [u8; 32],
    terminal_receipt_authentication_id: [u8; 32],
    terminal_policy_authentication_id: [u8; 32],
    target_close_receipt_id: [u8; 32],
    target_balance_before_lamports: u64,
    payer_refund_lamports: u64,
    neutral_surplus_lamports: u64,
    custody_balance_before_lamports: u64,
    custody_balance_after_lamports: u64,
    custody_principal_before_lamports: u64,
    custody_principal_after_lamports: u64,
    custody_post_data_id: [u8; 32],
    neutral_balance_before_lamports: u64,
    neutral_balance_after_lamports: u64,
    valid_before_slot: u64,
    ordered_accounts: Vec<AccountMeta>,
}

impl ChainDerivedSourceAction12MaterialV1 {
    /// Mutable runtime family derived from the hostile lineage and target.
    pub const fn family(&self) -> SourceMutableFamilyV2 {
        self.family
    }

    /// Exact immutable policy account family authorizing the close.
    pub const fn terminal_policy_kind(&self) -> SourceAction12TerminalPolicyKindV1 {
        self.terminal_policy_kind
    }

    /// Digest of the deterministic retained lineage postimage.
    pub const fn lineage_state_after_id(&self) -> [u8; 32] {
        self.lineage_state_after_id
    }

    /// Digest of the exact custody-ledger postimage after principal recycling.
    pub const fn custody_post_data_id(&self) -> [u8; 32] {
        self.custody_post_data_id
    }

    /// Exact close movement identity recomputed from the target header.
    pub const fn target_close_receipt_id(&self) -> [u8; 32] {
        self.target_close_receipt_id
    }

    /// Exact owner/PDA/body/schedule authentication of the retained terminal
    /// receipt. This is evidence for post-launch reconciliation, not payload
    /// authority.
    pub const fn terminal_receipt_authentication_id(&self) -> [u8; 32] {
        self.terminal_receipt_authentication_id
    }

    /// Exact authentication identity of the no-reopen or reopen-request
    /// policy account consumed by the close.
    pub const fn terminal_policy_authentication_id(&self) -> [u8; 32] {
        self.terminal_policy_authentication_id
    }

    /// Construct one unsigned action only after the checked release admits
    /// `(77, 2, 12)`. Action 12 intentionally uses replay sequence zero.
    pub fn unsigned_instruction(
        &self,
        release: &IndexedProgramRelease,
    ) -> Result<OwnedInstructionDraft> {
        let coordinate = CanonicalIntentCoordinate {
            family_tag: SOURCE_ACTION12_FAMILY_V1.tag(),
            family_version: SOURCE_ACTION12_FAMILY_V1.version(),
            local_action: SOURCE_ACTION12_LOCAL_ACTION_V1,
        };
        if release.key() != self.checked_release_key
            || release.program_id != self.program_id
            || release.program_data != self.program_data
            || release.release_manifest_sha256 != self.release_manifest_sha256
            || release.capability_profile_id != self.capability_profile_id
            || release.enabled_intents.binary_search(&coordinate).is_err()
        {
            return Err(SourceAction12MaterialError::CheckedRelease);
        }
        let intent = CloseGenerationIntentV2 {
            family: self.family,
            source_release_manifest_id: self.source_release_manifest_id,
            expected_lineage_state_id: self.lineage_state_before_id,
            semantic_terminal_receipt_id: self.semantic_terminal_receipt_id,
            valid_before_slot: self.valid_before_slot,
        };
        let mut payload = [0_u8; CLOSE_GENERATION_PAYLOAD_BYTES_V2];
        intent
            .encode(&mut payload)
            .map_err(|_| SourceAction12MaterialError::Construction)?;
        OwnedInstructionDraft::checked_release_source_close_generation(
            release,
            SemanticOwner {
                package: OWNER_PACKAGE_V1.into(),
                schema: OWNER_SCHEMA_V1.into(),
                release_sha256: self.release_manifest_sha256,
            },
            self.ordered_accounts.clone(),
            vec![
                ExactEquation {
                    name: "closed Source target partitions into principal and neutral surplus"
                        .into(),
                    unit: IntegerUnit::Lamports,
                    left: u128::from(self.target_balance_before_lamports),
                    right: u128::from(self.payer_refund_lamports)
                        + u128::from(self.neutral_surplus_lamports),
                },
                ExactEquation {
                    name: "Source lifecycle custody receives only stored target principal".into(),
                    unit: IntegerUnit::Lamports,
                    left: u128::from(self.custody_balance_before_lamports)
                        + u128::from(self.payer_refund_lamports),
                    right: u128::from(self.custody_balance_after_lamports),
                },
                ExactEquation {
                    name: "Source custody semantic principal recycles exact target principal"
                        .into(),
                    unit: IntegerUnit::Lamports,
                    left: u128::from(self.custody_principal_before_lamports)
                        + u128::from(self.payer_refund_lamports),
                    right: u128::from(self.custody_principal_after_lamports),
                },
                ExactEquation {
                    name: "Source target donation and surplus flow only to the neutral sink".into(),
                    unit: IntegerUnit::Lamports,
                    left: u128::from(self.neutral_balance_before_lamports)
                        + u128::from(self.neutral_surplus_lamports),
                    right: u128::from(self.neutral_balance_after_lamports),
                },
            ],
            &payload,
        )
        .map_err(map_construction)
    }
}

/// Reconstruct exact action-12 bytes, postimages, and metas from one finalized
/// snapshot. No capability is enabled by this constructor.
pub fn derive_source_action12_material_v1(
    release: &IndexedProgramRelease,
    snapshot: SourceAction12ChainSnapshotV1<'_>,
) -> Result<ChainDerivedSourceAction12MaterialV1> {
    authenticate_release_shape(release)?;
    authenticate_snapshot_provenance(release, snapshot)?;
    let program_id = release.program_id;
    let program_key = runtime_key(program_id);

    let manifest = clutch_source_plane_v3_runtime::SourceReleaseManifestV2::decode(
        &snapshot.source_release.data,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let release_recipe = PdaRecipeV3::source_release(
        manifest
            .id()
            .map_err(|_| SourceAction12MaterialError::ChainAuthority)?,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let authenticated_release = authenticate_source_release_account(
        program_key,
        account_view(snapshot.source_release, false, false),
        derive_recipe(program_id, release_recipe)?,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    if snapshot.adapter_program.address != release.program_id
        || snapshot.adapter_program_data.address != release.program_data
    {
        return Err(SourceAction12MaterialError::CheckedRelease);
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
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let schedule = authenticate_schedule(program_id, route, snapshot.source_work_schedule)?;
    let custody = authenticate_custody(
        program_id,
        route,
        schedule,
        snapshot.source_funding_custody,
    )?;

    let lineage_body = ReopenLineageV1::decode(&snapshot.generation_lineage.data)
        .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let lineage_recipe = PdaRecipeV3::reopen_lineage(
        lineage_body
            .recipe_id()
            .map_err(|_| SourceAction12MaterialError::ChainAuthority)?,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let lineage = authenticate_reopen_lineage_account(
        route,
        account_view(snapshot.generation_lineage, true, false),
        derive_recipe(program_id, lineage_recipe)?,
        LineageAccessV1::Mutable,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let state = lineage.lineage();
    let family = mutable_family(state.family)?;
    if !state.is_open
        || state.latest_generation == 0
        || state.active_account.bytes() != snapshot.generation_target.address.to_bytes()
    {
        return Err(SourceAction12MaterialError::ChainAuthority);
    }

    let terminal_receipt = authenticate_terminal_receipt(
        program_id,
        route,
        schedule,
        snapshot.terminal_work_receipt,
    )?;
    let receipt = terminal_receipt.receipt();
    if receipt.disposition() != SourceReceiptDispositionV1::TerminalSuccess
        || receipt.work_kind().is_some()
        || receipt.call_ordinal() != 0
        || receipt.call_ceiling_lamports() != 0
    {
        return Err(SourceAction12MaterialError::ChainAuthority);
    }
    let semantic_terminal_id = receipt.semantic_receipt_id();
    let terminal_policy = authenticate_terminal_policy(
        program_id,
        route,
        lineage,
        semantic_terminal_id,
        snapshot.terminal_policy,
    )?;
    if terminal_policy.family != family {
        return Err(SourceAction12MaterialError::ChainAuthority);
    }

    let (header, final_state_id) = authenticate_generation_target(
        program_id,
        route,
        lineage,
        family,
        snapshot.generation_target,
    )?;
    let close = plan_runtime_account_close_from_header(
        runtime_key(snapshot.generation_target.address),
        header,
        route.neutral_sink(),
        snapshot.generation_target.lamports,
        semantic_terminal_id,
    )
    .map_err(|_| SourceAction12MaterialError::Funding)?;
    if close.generation != state.latest_generation
        || close.account.bytes() != snapshot.generation_target.address.to_bytes()
        || close.neutral_sink != route.neutral_sink()
        || (close.payer_refund_lamports != 0
            && close.principal_recipient.bytes()
                != snapshot.source_funding_custody.address.to_bytes())
        || (close.payer_refund_lamports == 0
            && !close.principal_recipient.is_zero())
    {
        return Err(SourceAction12MaterialError::Funding);
    }
    let lineage_after = close_lineage_generation(
        state,
        runtime_key(snapshot.generation_target.address),
        header.generation,
        final_state_id,
        semantic_terminal_id,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let lineage_after_bytes = lineage_after
        .encode()
        .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let lineage_state_after_id = account_data_id(
        runtime_key(snapshot.generation_lineage.address),
        &lineage_after_bytes,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    if lineage_state_after_id != terminal_policy.projected_closed_lineage_state_id {
        return Err(SourceAction12MaterialError::ChainAuthority);
    }

    let custody_balance_after_lamports = snapshot
        .source_funding_custody
        .lamports
        .checked_add(close.payer_refund_lamports)
        .ok_or(SourceAction12MaterialError::Arithmetic)?;
    let custody_after = if close.payer_refund_lamports == 0 {
        custody
    } else {
        custody
            .transition(
                0,
                close.payer_refund_lamports,
                custody_balance_after_lamports,
                close.close_receipt_id,
            )
            .map_err(|_| SourceAction12MaterialError::Funding)?
    };
    let custody_after_bytes = custody_after
        .encode()
        .map_err(|_| SourceAction12MaterialError::Funding)?;
    let custody_post_data_id = account_data_id(
        runtime_key(snapshot.source_funding_custody.address),
        &custody_after_bytes,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let neutral_balance_after_lamports = snapshot
        .neutral_sink
        .lamports
        .checked_add(close.neutral_surplus_lamports)
        .ok_or(SourceAction12MaterialError::Arithmetic)?;
    let valid_before_slot = snapshot
        .source_release
        .provenance
        .slot
        .checked_add(SOURCE_ACTION12_VALIDITY_SLOTS_V1)
        .ok_or(SourceAction12MaterialError::Arithmetic)?;

    let ordered_accounts = ordered_action12_accounts([
        snapshot.source_release.address,
        snapshot.adapter_program.address,
        snapshot.adapter_program_data.address,
        snapshot.parser_program.address,
        snapshot.parser_program_data.address,
        snapshot.parser_config.address,
        snapshot.source_spec.address,
        snapshot.source_work_schedule.address,
        snapshot.terminal_policy.address,
        snapshot.generation_target.address,
        snapshot.generation_lineage.address,
        snapshot.terminal_work_receipt.address,
        snapshot.source_funding_custody.address,
        snapshot.neutral_sink.address,
    ])?;

    Ok(ChainDerivedSourceAction12MaterialV1 {
        checked_release_key: release.key(),
        release_manifest_sha256: release.release_manifest_sha256,
        capability_profile_id: release.capability_profile_id,
        program_id,
        program_data: release.program_data,
        family,
        terminal_policy_kind: terminal_policy.kind,
        source_release_manifest_id: route.release_manifest_id().bytes(),
        lineage_state_before_id: lineage.account_data_id().bytes(),
        lineage_state_after_id: lineage_state_after_id.bytes(),
        semantic_terminal_receipt_id: semantic_terminal_id.bytes(),
        terminal_receipt_authentication_id: terminal_receipt.id().bytes(),
        terminal_policy_authentication_id: terminal_policy.authentication_id.bytes(),
        target_close_receipt_id: close.close_receipt_id.bytes(),
        target_balance_before_lamports: snapshot.generation_target.lamports,
        payer_refund_lamports: close.payer_refund_lamports,
        neutral_surplus_lamports: close.neutral_surplus_lamports,
        custody_balance_before_lamports: snapshot.source_funding_custody.lamports,
        custody_balance_after_lamports,
        custody_principal_before_lamports: custody.remaining_principal_lamports,
        custody_principal_after_lamports: custody_after.remaining_principal_lamports,
        custody_post_data_id: custody_post_data_id.bytes(),
        neutral_balance_before_lamports: snapshot.neutral_sink.lamports,
        neutral_balance_after_lamports,
        valid_before_slot,
        ordered_accounts,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedTerminalPolicyProjectionV1 {
    kind: SourceAction12TerminalPolicyKindV1,
    family: SourceMutableFamilyV2,
    authentication_id: ContentId,
    projected_closed_lineage_state_id: ContentId,
}

fn authenticate_terminal_policy(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    terminal_semantic_id: ContentId,
    account: &ObservedRpcAccount,
) -> Result<AuthenticatedTerminalPolicyProjectionV1> {
    match account.data.len() {
        SOURCE_NO_REOPEN_TERMINAL_BYTES => {
            let body = SourceNoReopenTerminalV1::decode(&account.data)
                .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
            let terminal_id = body
                .id()
                .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
            let derived = derive_recipe(
                program_id,
                PdaRecipeV3::source_no_reopen_terminal(terminal_id)
                    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?,
            )?;
            let authenticated = authenticate_source_no_reopen_terminal(
                route,
                body,
                account_view(account, false, false),
                derived,
                SourceNoReopenTerminalAccessV1::ExistingReadOnly,
            )
            .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
            let state = lineage.lineage();
            let family = mutable_family_from_reopen(authenticated.family());
            if authenticated.terminal_id().map_err(|_| {
                SourceAction12MaterialError::ChainAuthority
            })? != terminal_semantic_id
                || authenticated.expected_lineage_state_id() != lineage.account_data_id()
                || authenticated.lineage_authentication_id() != lineage.id()
                || authenticated.lineage_account() != state.lineage_account
                || authenticated.target_account() != state.active_account
                || family != mutable_family(state.family)?
            {
                return Err(SourceAction12MaterialError::ChainAuthority);
            }
            let projected = projected_closed_lineage(lineage, terminal_semantic_id)?;
            Ok(AuthenticatedTerminalPolicyProjectionV1 {
                kind: SourceAction12TerminalPolicyKindV1::NoReopen,
                family,
                authentication_id: authenticated.id(),
                projected_closed_lineage_state_id: projected,
            })
        }
        SOURCE_REOPEN_GENERATION_REQUEST_BYTES => {
            let request = SourceReopenGenerationRequestV1::decode(&account.data)
                .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
            let request_id = request
                .id()
                .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
            let authority = address(route.generation_authority_program());
            let (expected, bump) = Address::find_program_address(
                &[SEED_SOURCE_REOPEN_REQUEST_V1, &request_id.bytes()],
                &authority,
            );
            let authenticated = authenticate_source_reopen_generation_request_before_close(
                route,
                account_view(account, false, false),
                RuntimeDerivedPdaV1 {
                    program_id: route.generation_authority_program(),
                    recipe_id: request_id,
                    address: runtime_key(expected),
                    bump,
                },
                lineage,
                terminal_semantic_id,
            )
            .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
            Ok(AuthenticatedTerminalPolicyProjectionV1 {
                kind: SourceAction12TerminalPolicyKindV1::ReopenRequest,
                family: mutable_family_from_reopen(authenticated.family()),
                authentication_id: authenticated.id(),
                projected_closed_lineage_state_id: authenticated
                    .projected_closed_lineage_state_id(),
            })
        }
        _ => Err(SourceAction12MaterialError::ChainAuthority),
    }
}

fn projected_closed_lineage(
    lineage: AuthenticatedReopenLineageV1,
    terminal_semantic_id: ContentId,
) -> Result<ContentId> {
    let state = lineage.lineage();
    let projected = close_lineage_generation(
        state,
        state.active_account,
        state.latest_generation,
        state.last_opened_state_id,
        terminal_semantic_id,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    account_data_id(state.lineage_account, &projected.encode().map_err(|_| {
        SourceAction12MaterialError::ChainAuthority
    })?)
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)
}

fn authenticate_terminal_receipt(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    schedule: SourceWorkScheduleBindingV1,
    account: &ObservedRpcAccount,
) -> Result<clutch_source_plane_v3_runtime::AuthenticatedSourceWorkReceiptV1> {
    if account.owner != program_id
        || account.executable
        || account.data.len() != SOURCE_WORK_RECEIPT_ACCOUNT_BYTES
    {
        return Err(SourceAction12MaterialError::ChainAuthority);
    }
    let body = SourceWorkReceiptAccountV1::decode(&account.data)
        .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let recipe = PdaRecipeV3::source_work_receipt(
        body.receipt_slot_id(route, schedule)
            .map_err(|_| SourceAction12MaterialError::ChainAuthority)?,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    authenticate_source_work_receipt_account(
        route,
        schedule,
        account_view(account, false, false),
        derive_recipe(program_id, recipe)?,
        SourceWorkReceiptAccessV1::ExistingReadOnly,
    )
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)
}

fn authenticate_generation_target(
    program_id: Address,
    route: AuthenticatedSourceRouteV1,
    lineage: AuthenticatedReopenLineageV1,
    family: SourceMutableFamilyV2,
    account: &ObservedRpcAccount,
) -> Result<(RuntimeAccountHeaderV1, ContentId)> {
    if account.owner != program_id || account.executable {
        return Err(SourceAction12MaterialError::ChainAuthority);
    }
    let header = match family {
        SourceMutableFamilyV2::SourceHead => {
            decode_runtime_account::<SourceHeadV3>(&account.data, route.neutral_sink())
                .map(|value| value.0)
        }
        SourceMutableFamilyV2::OpenRawPage => {
            decode_runtime_account::<OpenRawPageV3>(&account.data, route.neutral_sink())
                .map(|value| value.0)
        }
        SourceMutableFamilyV2::WindowWork => {
            decode_runtime_account::<WindowWorkV3>(&account.data, route.neutral_sink())
                .map(|value| value.0)
        }
        SourceMutableFamilyV2::StatisticResult => {
            decode_runtime_account::<StatisticResultV3>(&account.data, route.neutral_sink())
                .map(|value| value.0)
        }
    }
    .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let final_state_id = account_data_id(runtime_key(account.address), &account.data)
        .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let state = lineage.lineage();
    if state.active_account.bytes() != account.address.to_bytes()
        || state.latest_generation != header.generation
        || state.last_opened_state_id != final_state_id
    {
        return Err(SourceAction12MaterialError::ChainAuthority);
    }
    Ok((header, final_state_id))
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
        return Err(SourceAction12MaterialError::ChainAuthority);
    }
    let schedule = SourceWorkScheduleBindingV1::decode(&account.data)
        .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    schedule
        .validate_against(route)
        .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let schedule_id = schedule
        .id()
        .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let (expected, _) = Address::find_program_address(
        &[
            SEED_PRODUCT_ARTIFACT_V1,
            &[ArtifactKind::SourceWorkScheduleV1.byte()],
            &schedule_id.bytes(),
        ],
        &program_id,
    );
    if account.address != expected {
        return Err(SourceAction12MaterialError::ChainAuthority);
    }
    Ok(schedule)
}

fn authenticate_custody(
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
        return Err(SourceAction12MaterialError::ChainAuthority);
    }
    let ledger = SourceFundingCustodyLedgerV1::decode(&account.data)
        .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let explained = ledger
        .remaining_principal_lamports
        .checked_add(ledger.donation_lamports)
        .ok_or(SourceAction12MaterialError::Arithmetic)?;
    if !ledger.is_live()
        || schedule.payer().bytes() != account.address.to_bytes()
        || ledger.adapter_program.bytes() != program_id.to_bytes()
        || ledger.release_manifest_id != route.release_manifest_id()
        || ledger.route_id != route.route_id()
        || ledger.source_work_schedule_id != schedule.source_work_schedule_id()
        || ledger.lifecycle_id != schedule.lifecycle_id()
        || ledger.custody_account.bytes() != account.address.to_bytes()
        || ledger.neutral_sink != route.neutral_sink()
        || account.lamports < explained
    {
        return Err(SourceAction12MaterialError::ChainAuthority);
    }
    Ok(ledger)
}

fn authenticate_release_shape(release: &IndexedProgramRelease) -> Result<()> {
    release
        .validate()
        .map_err(|_| SourceAction12MaterialError::CheckedRelease)?;
    if !release.families.contains(&CanonicalFamily::Source) {
        return Err(SourceAction12MaterialError::CheckedRelease);
    }
    Ok(())
}

fn authenticate_snapshot_provenance(
    release: &IndexedProgramRelease,
    snapshot: SourceAction12ChainSnapshotV1<'_>,
) -> Result<()> {
    let first = &snapshot.source_release.provenance;
    let release_key = release.key();
    if first.slot == 0
        || first.commitment != RpcCommitment::Finalized
        || first.release_key.as_str() != release_key.as_str()
    {
        return Err(SourceAction12MaterialError::ChainSnapshot);
    }
    for provenance in [
        &snapshot.source_release.provenance,
        &snapshot.adapter_program.provenance,
        &snapshot.adapter_program_data.provenance,
        &snapshot.parser_program.provenance,
        &snapshot.parser_program_data.provenance,
        &snapshot.parser_config.provenance,
        &snapshot.source_spec.provenance,
        &snapshot.source_work_schedule.provenance,
        &snapshot.terminal_policy.provenance,
        &snapshot.generation_target.provenance,
        &snapshot.generation_lineage.provenance,
        &snapshot.terminal_work_receipt.provenance,
        &snapshot.source_funding_custody.provenance,
        &snapshot.neutral_sink.provenance,
    ] {
        if provenance.cluster_key != first.cluster_key
            || provenance.slot != first.slot
            || provenance.commitment != RpcCommitment::Finalized
            || provenance.release_key.as_str() != release_key.as_str()
        {
            return Err(SourceAction12MaterialError::ChainSnapshot);
        }
    }
    for account in [
        snapshot.source_release,
        snapshot.source_work_schedule,
        snapshot.generation_target,
        snapshot.generation_lineage,
        snapshot.terminal_work_receipt,
        snapshot.source_funding_custody,
    ] {
        if account.owner != release.program_id {
            return Err(SourceAction12MaterialError::ChainSnapshot);
        }
    }
    if snapshot.neutral_sink.address == Address::default()
        || snapshot.neutral_sink.executable
        || snapshot.terminal_policy.executable
    {
        return Err(SourceAction12MaterialError::ChainSnapshot);
    }
    Ok(())
}

fn ordered_action12_accounts(addresses: [Address; 14]) -> Result<Vec<AccountMeta>> {
    let action = SourceSeriesAction::CloseGeneration;
    let contract = account_contract_v2(action);
    if contract.len() != addresses.len() {
        return Err(SourceAction12MaterialError::Construction);
    }
    let mut accounts = Vec::with_capacity(addresses.len());
    let mut observed = Vec::with_capacity(addresses.len());
    for (index, pubkey) in addresses.into_iter().enumerate() {
        let expected = contract
            .meta(index)
            .ok_or(SourceAction12MaterialError::Construction)?;
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
        .map_err(|_| SourceAction12MaterialError::Construction)?;
    Ok(accounts)
}

fn mutable_family(family: LineageFamilyV1) -> Result<SourceMutableFamilyV2> {
    match family {
        LineageFamilyV1::SourceHead => Ok(SourceMutableFamilyV2::SourceHead),
        LineageFamilyV1::OpenRawPage => Ok(SourceMutableFamilyV2::OpenRawPage),
        LineageFamilyV1::WindowWork => Ok(SourceMutableFamilyV2::WindowWork),
        LineageFamilyV1::StatisticResult => Ok(SourceMutableFamilyV2::StatisticResult),
        LineageFamilyV1::EvaluationWork => Err(SourceAction12MaterialError::ChainAuthority),
    }
}

const fn mutable_family_from_reopen(family: SourceReopenFamilyV1) -> SourceMutableFamilyV2 {
    match family {
        SourceReopenFamilyV1::SourceHead => SourceMutableFamilyV2::SourceHead,
        SourceReopenFamilyV1::OpenRawPage => SourceMutableFamilyV2::OpenRawPage,
        SourceReopenFamilyV1::WindowWork => SourceMutableFamilyV2::WindowWork,
        SourceReopenFamilyV1::StatisticResult => SourceMutableFamilyV2::StatisticResult,
    }
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
        .map_err(|_| SourceAction12MaterialError::ChainAuthority)?;
    let mut seeds = Vec::with_capacity(usize::from(recipe.seed_count()));
    let mut index = 0_usize;
    while index < usize::from(recipe.seed_count()) {
        seeds.push(
            recipe
                .seed(index)
                .map_err(|_| SourceAction12MaterialError::ChainAuthority)?,
        );
        index += 1;
    }
    let (derived, bump) = Address::find_program_address(&seeds, &program_id);
    Ok(RuntimeDerivedPdaV1 {
        program_id: runtime_key(program_id),
        recipe_id: recipe
            .id()
            .map_err(|_| SourceAction12MaterialError::ChainAuthority)?,
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

fn map_construction(_error: ConstructionError) -> SourceAction12MaterialError {
    SourceAction12MaterialError::Construction
}

#[cfg(test)]
mod adversarial_tests {
    use super::*;

    fn address(byte: u8) -> Address {
        Address::new_from_array([byte; 32])
    }

    fn material() -> ChainDerivedSourceAction12MaterialV1 {
        ChainDerivedSourceAction12MaterialV1 {
            checked_release_key: "checked-release".into(),
            release_manifest_sha256: [2; 32],
            capability_profile_id: [3; 32],
            program_id: address(4),
            program_data: address(5),
            family: SourceMutableFamilyV2::StatisticResult,
            terminal_policy_kind: SourceAction12TerminalPolicyKindV1::NoReopen,
            source_release_manifest_id: [6; 32],
            lineage_state_before_id: [7; 32],
            lineage_state_after_id: [8; 32],
            semantic_terminal_receipt_id: [9; 32],
            terminal_receipt_authentication_id: [10; 32],
            terminal_policy_authentication_id: [11; 32],
            target_close_receipt_id: [12; 32],
            target_balance_before_lamports: 10,
            payer_refund_lamports: 4,
            neutral_surplus_lamports: 6,
            custody_balance_before_lamports: 20,
            custody_balance_after_lamports: 24,
            custody_principal_before_lamports: 30,
            custody_principal_after_lamports: 34,
            custody_post_data_id: [13; 32],
            neutral_balance_before_lamports: 40,
            neutral_balance_after_lamports: 46,
            valid_before_slot: 50,
            ordered_accounts: (20_u8..34_u8)
                .map(|byte| AccountMeta::new_readonly(address(byte), false))
                .collect(),
        }
    }

    #[test]
    fn close_account_contract_refuses_every_alias() {
        let mut addresses = [Address::default(); 14];
        for (index, value) in addresses.iter_mut().enumerate() {
            *value = address(u8::try_from(index + 1).unwrap());
        }
        assert!(ordered_action12_accounts(addresses).is_ok());
        addresses[13] = addresses[12];
        assert_eq!(
            ordered_action12_accounts(addresses),
            Err(SourceAction12MaterialError::Construction)
        );
    }

    #[test]
    fn absent_or_substituted_release_tuple_cannot_emit_bytes() {
        let value = material();
        let release = IndexedProgramRelease {
            program_id: value.program_id,
            program_data: value.program_data,
            elf_sha256: [1; 32],
            deployment_slot: 1,
            release_manifest_sha256: value.release_manifest_sha256,
            capability_profile_id: value.capability_profile_id,
            source_commit: "1".repeat(40),
            enabled_intents: Vec::new(),
            families: vec![CanonicalFamily::Source],
        };
        assert_eq!(
            value.unsigned_instruction(&release),
            Err(SourceAction12MaterialError::CheckedRelease)
        );
        let mut substituted = release;
        substituted.enabled_intents.push(CanonicalIntentCoordinate {
            family_tag: SOURCE_ACTION12_FAMILY_V1.tag(),
            family_version: SOURCE_ACTION12_FAMILY_V1.version(),
            local_action: SOURCE_ACTION12_LOCAL_ACTION_V1,
        });
        substituted.capability_profile_id = [99; 32];
        assert_eq!(
            value.unsigned_instruction(&substituted),
            Err(SourceAction12MaterialError::CheckedRelease)
        );
    }

    #[test]
    fn non_close_lineage_family_never_lowers_into_action12() {
        assert_eq!(
            mutable_family(LineageFamilyV1::EvaluationWork),
            Err(SourceAction12MaterialError::ChainAuthority)
        );
    }

    #[test]
    fn generic_successor_constructor_cannot_recreate_action12() {
        assert_eq!(
            OwnedInstructionDraft::allocated_successor(
                crate::transaction_builder::ProtocolFlow::SourcePlaneV3,
                "caller-shaped-close",
                SemanticOwner {
                    package: "caller".into(),
                    schema: "caller/v1".into(),
                    release_sha256: [1; 32],
                },
                address(1),
                Vec::new(),
                Vec::new(),
                vec![ExactEquation {
                    name: "caller assertion".into(),
                    unit: IntegerUnit::Lamports,
                    left: 0,
                    right: 0,
                }],
                clutch_solana_layout::registry::ExtensionAction::SourceV3(
                    SourceSeriesAction::CloseGeneration,
                ),
                &[0; CLOSE_GENERATION_PAYLOAD_BYTES_V2],
            ),
            Err(ConstructionError::UnallocatedRegistryCoordinate)
        );
    }
}
