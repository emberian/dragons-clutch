//! Chain-derived General V3 Hot execution and packet construction.
//!
//! The operator never owns an action-specific account list. It authenticates
//! the selected General artifacts, expands the exact selected AccountProfile,
//! derives Product width from the finalized Product graph, and then compiles a
//! single unsigned v0 message through one exact canonical lookup table. It
//! performs no RPC, signing, submission, or account mutation.

use dclutch_account_profile_contract::lifecycle_v3::{
    CoordinateScopeV3, LifecycleOperationV3, LifecycleRegisterKindV3, LifecycleRegistersV3,
    LifecycleSeedInputValueV3, SelectedLifecycleV3,
};
use dclutch_account_profile_contract::v2::{
    DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE, PhysicalAccountDataGeometryV2,
};
use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
    HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
    HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3, HotExecutionEnvelopeV3,
};
use dclutch_effect_kernel::v2::FixedRole;
use dclutch_execution_strategy_contract::v2::{BankTransportV2, classify_bank_transport_v2};
use dclutch_general_adapter_contract::artifacts_v3::{
    GeneralArtifactBytesV3, GeneralArtifactSelectionV3, authenticate_general_artifacts_v3,
};
use dclutch_general_adapter_contract::{
    admitted_accelerator_v3::authenticate_frozen_selection_v3,
    effect_artifacts_v3::{GeneralChildFrameV3, general_effect_route_frame_v3},
    hot_candidate_v3::{identity as general_identity, scalar as general_scalar},
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateV3},
    runtime_manifest::SettlementManifestV2,
    runtime_selection::{
        RUNTIME_SELECTION_CURSOR_BYTES_V2, RuntimeSelectionCursorV2,
        consider_verified_candidate_v2, freeze_selection_v2,
    },
    runtime_settlement::{
        RuntimeSettlementActionV2, RuntimeSettlementViewV2,
        evaluate_runtime_settlement_in_place_v2, initialize_runtime_settlement_in_place_v2,
        runtime_settlement_effect_len_v2,
    },
    runtime_verify::RuntimeCandidateVerifierV2,
    runtime_width::{SettlementCursorV2, VerifiedCandidateV2, settlement_cursor_len},
    state_artifacts_v3::{
        GENERAL_CLOSE_PAYER_ACCOUNT_V3, GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_PAYER_ACCOUNT_V3, GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_PRIMARY_STATE_ACCOUNT_V3, GENERAL_TERMINAL_STATE_ACCOUNT_V3,
        GeneralChildRentWidthsV5, GeneralReadonlyEvidenceKindV3,
        encode_general_state_lifecycle_v5_atomic, general_child_account_start_v3,
        general_readonly_evidence_count_v3, general_readonly_evidence_v3,
        general_state_lifecycle_bytes_v5,
    },
};
use dclutch_general_codec::{
    Action, SelectionPolicyV1,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
};
use dclutch_general_config_contract::v3::GeneralConfigV3;
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Finality, Observation, ObservedAccount,
    product_graph_observation_v3::{
        AuthenticatedProductGraphObservationV3, FinalizedProductGraphAccountsV3,
        authenticate_product_graph_observation_v3,
    },
    versioned::{VersionedMessagePlanV0, compile_v0_message},
};

const HOT_RUNTIME_LOGICAL_PREFIX_V3: usize = 5;
const ADMITTED_AOT_FIXED_EXTRAS_V3: usize = 8;
const ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3: usize = 6;

/// One exact finalized account plus its requested transaction privileges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralObservedAccountMetaV3 {
    /// Exact finalized account observation.
    pub account: ObservedAccount,
    /// Whether the top-level transaction requests signer privilege.
    pub is_signer: bool,
    /// Whether the top-level transaction requests writable privilege.
    pub is_writable: bool,
}

impl GeneralObservedAccountMetaV3 {
    fn meta(&self) -> AccountMeta {
        AccountMeta {
            pubkey: self.account.key,
            is_signer: self.is_signer,
            is_writable: self.is_writable,
        }
    }
}

/// Checked release evidence required before an operator recognizes General V3.
///
/// A chain reader cannot create this authority from self-consistent chain state
/// alone. A release checker constructs it after the selected immutable Trading
/// and admitted-accelerator ArtifactReleases match a user-supplied manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedGeneralHotReleaseV3 {
    /// Exact selected Trading program.
    pub trading_program: Pubkey,
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Exact immutable admitted General accelerator ArtifactRelease identity.
    pub general_artifact_release: [u8; 32],
    /// Digest of the user-supplied checked multiprogram manifest.
    pub checked_manifest_digest: [u8; 32],
}

/// Same-finalized physical state for one General Hot construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralHotStateV3 {
    /// Exact common physical Hot38 frame in canonical ABI order.
    pub fixed_accounts: Vec<GeneralObservedAccountMetaV3>,
    /// Exact admitted-AOT transport accounts between Hot38 and runtime state.
    pub strategy_accounts: Vec<GeneralObservedAccountMetaV3>,
    /// Packed AccountProfile physical representatives after the injected
    /// `[root, config, Product, portfolio, linked-basis]` representatives.
    pub runtime_suffix_accounts: Vec<GeneralObservedAccountMetaV3>,
    /// Immutable execution release set selected by Market.
    pub release_set: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Lowest finalized slot accepted for this construction attempt.
    pub minimum_finalized_slot: u64,
    /// Checked current Hot release, absent for unknown deployments.
    pub checked_release: Option<CheckedGeneralHotReleaseV3>,
}

/// Canonical action-state addresses derived from the authenticated lifecycle policy.
///
/// These values are an operator projection, never authority. Trading derives the
/// same addresses and bumps again before it creates, authenticates, or closes
/// either account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralLifecycleProjectionV3 {
    /// Canonical selection or settlement state selected by the action.
    pub primary_state: Pubkey,
    /// Canonical primary-state PDA bump written into the request witness.
    pub primary_state_bump: u8,
    /// Close-only canonical terminal state.
    pub terminal_state: Option<Pubkey>,
    /// Close-only canonical terminal-state PDA bump.
    pub terminal_state_bump: Option<u8>,
    /// Close-only terminal coordinate, equal to the consumed revision plus one.
    pub terminal_coordinate: Option<u64>,
    /// First family child account after the exact lifecycle frame.
    pub child_account_start: u16,
}

/// Exact content identities of the complete authenticated General artifact graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralHotArtifactDigestsV3 {
    /// Action-selector CapabilityProgramSetV2.
    pub program_set: [u8; 32],
    /// Action-selected CapabilityProgramV4 descriptor.
    pub descriptor: [u8; 32],
    /// Immutable GeneralConfigV3.
    pub config: [u8; 32],
    /// Runtime-width Profile13 AccountProfileV2.
    pub account_profile: [u8; 32],
    /// Protected StateLifecyclePolicyV5.
    pub lifecycle_policy: [u8; 32],
    /// Exact action RequestProfileV1.
    pub request_profile: [u8; 32],
    /// Selected ExecutionStrategyProgramV2.
    pub strategy: [u8; 32],
    /// Translation-equivalence certificate.
    pub certificate: [u8; 32],
    /// Registry admission for that certificate.
    pub admission: [u8; 32],
    /// Exact admitted TransitionProgramV3.
    pub transition: [u8; 32],
    /// Exact action EffectProgramV3.
    pub effect: [u8; 32],
}

/// Complete unsigned General instruction with exact checked provenance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralHotInstructionV3 {
    /// Exact unsigned Trading instruction.
    pub instruction: Instruction,
    /// Exact General action encoded in the family request.
    pub action: Action,
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Exact finalized observation shared by every input.
    pub observation: Observation,
    /// Wallet keys which the instruction itself requires to sign.
    pub required_instruction_signers: Vec<Pubkey>,
    /// Checked multiprogram manifest digest.
    pub checked_manifest_digest: [u8; 32],
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Exact admitted General accelerator ArtifactRelease identity.
    pub general_artifact_release: [u8; 32],
    /// Complete exact content identities selected by the artifact graph.
    pub artifacts: GeneralHotArtifactDigestsV3,
    /// Authenticated Product graph-root content digest.
    pub product_record: [u8; 32],
    /// Digest of the exact canonical family request.
    pub family_request_digest: [u8; 32],
    /// Canonical action-state projection derived from the lifecycle artifact.
    pub lifecycle: GeneralLifecycleProjectionV3,
}

/// Packet-safe unsigned General transaction plus its exact signer report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralHotTransactionPlanV3 {
    /// Packet-safe v0 message compiled through the exact canonical LUT.
    pub message: VersionedMessagePlanV0,
    /// Exact eventual wallet signer order, beginning with the fee payer.
    pub required_signers: Vec<Pubkey>,
    /// Exact General action carried by the instruction.
    pub action: Action,
    /// Checked multiprogram manifest digest.
    pub checked_manifest_digest: [u8; 32],
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Exact immutable Trading ArtifactRelease identity.
    pub trading_artifact_release: [u8; 32],
    /// Exact immutable admitted General accelerator ArtifactRelease identity.
    pub general_artifact_release: [u8; 32],
    /// Complete exact content identities selected by the artifact graph.
    pub artifacts: GeneralHotArtifactDigestsV3,
    /// Authenticated Product graph-root content digest.
    pub product_record: [u8; 32],
    /// Canonical action-state projection carried by the request.
    pub lifecycle: GeneralLifecycleProjectionV3,
}

/// One exact prior-child receipt appended to a DCE5 route request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralReceiptDependencyV5 {
    /// Child role which produced the retained receipt.
    pub producer_role: FixedRole,
    /// Strictly earlier route which produced the retained receipt.
    pub producer_route: u16,
    /// Exact authenticated receipt width.
    pub expected_receipt_bytes: u16,
}

/// One action-selected child route derived from the authenticated DCE5 artifact.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralChildRouteV5 {
    /// Route ordinal in exact execution order.
    pub route: u16,
    /// State-owning child role.
    pub role: FixedRole,
    /// First logical AccountProfile coordinate of the child frame.
    pub account_start: u16,
    /// Exact semantic-owner child-frame width.
    pub account_count: u16,
    /// Exact prior receipts appended in declared order.
    pub receipt_dependencies: Vec<GeneralReceiptDependencyV5>,
}

/// Stable transaction-complete General successor instruction for frontends.
///
/// The request, runtime width, scratch span, state PDAs, child frames, and
/// receipt order are all derived from one hostile-decoded finalized snapshot.
/// This value contains no signer material and performs no submission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralSuccessorInstructionV5 {
    /// Fully checked unsigned Hot instruction.
    pub hot: GeneralHotInstructionV3,
    /// Canonical chain-derived controller request, including PDA bump witnesses.
    pub request: ControllerRequestV2,
    /// Product-derived outcome width.
    pub outcome_count: u32,
    /// Canonical authenticated scratch-page count for the selected bank geometry.
    pub scratch_page_count: u32,
    /// Exact action-specific DCE5 child route and receipt order.
    pub child_routes: Vec<GeneralChildRouteV5>,
}

/// Stable packet-safe unsigned General successor plan for frontends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralSuccessorTransactionPlanV0 {
    /// Exact packet-safe v0 message and signer report.
    pub hot: GeneralHotTransactionPlanV3,
    /// Canonical chain-derived controller request.
    pub request: ControllerRequestV2,
    /// Canonical authenticated scratch-page count.
    pub scratch_page_count: u32,
    /// Exact DCE5 child route and receipt order.
    pub child_routes: Vec<GeneralChildRouteV5>,
}

/// Stable refusal from General artifact, account, release, or packet checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralHotOperatorErrorV3 {
    /// The selected deployment was not recognized by checked release evidence.
    UnrecognizedRelease,
    /// A required content identity was zero or differed from selected bytes.
    ContentIdentity,
    /// Accounts did not share one sufficiently recent finalized snapshot.
    Snapshot,
    /// The common Hot38 physical frame differed.
    FixedFrame,
    /// Product/domain/portfolio finalization or composition differed.
    Product,
    /// The selected General artifact bundle refused.
    Artifact,
    /// Admitted-AOT transport geometry differed from the selected bank.
    StrategyGeometry,
    /// AccountProfile expansion, privilege, alias, or width differed.
    RuntimeGeometry,
    /// Lifecycle bytes, plan geometry, state shape, PDA, or bump differed.
    Lifecycle,
    /// Chain-derived selection, candidate, verifier, manifest, or settlement state refused.
    ChainState,
    /// Required signer reporting differed from the compiled message.
    Signer,
    /// The lookup table was not the exact canonical address set.
    LookupTable,
    /// Lookup-table or packet compilation refused.
    Routing(crate::versioned::Error),
    /// Checked arithmetic or encoding overflowed.
    Arithmetic,
}

/// Build one transaction-complete General successor instruction from chain state.
///
/// Unlike [`build_general_hot_instruction_v3`], this is the stable public
/// frontend seam: the caller selects only an action. The optimistic revision,
/// best-valid-submitted candidate identity, candidate/page coordinate,
/// manifest-row coordinate, and lifecycle bumps are derived from authenticated
/// accounts. The selected artifacts determine all account privileges, child
/// request templates, and ordered receipt dependencies.
pub fn build_general_successor_instruction_v5(
    state: &GeneralHotStateV3,
    artifact_selection: GeneralArtifactSelectionV3,
    artifact_bytes: GeneralArtifactBytesV3<'_>,
    action: Action,
) -> Result<GeneralSuccessorInstructionV5, GeneralHotOperatorErrorV3> {
    let checked = state
        .checked_release
        .ok_or(GeneralHotOperatorErrorV3::UnrecognizedRelease)?;
    validate_release(checked, artifact_selection)?;
    validate_fixed_frame(state, checked)?;
    let product = authenticate_product_graph(state)?;
    let request = derive_general_request_v5(
        state,
        artifact_selection,
        artifact_bytes,
        action,
        product.outcome_count,
        product.product_record,
    )?;
    let hot = build_general_hot_instruction_v3(state, artifact_selection, artifact_bytes, request)?;
    let request = ControllerRequestV2::decode(
        hot.instruction
            .data
            .get(HOT_FAMILY_REQUEST_OFFSET_V3..)
            .ok_or(GeneralHotOperatorErrorV3::Artifact)?,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    let request_bytes = request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    let bundle = authenticate_general_artifacts_v3(
        artifact_selection,
        artifact_bytes,
        &request_bytes,
        product.outcome_count,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    let scratch_page_count = selected_bank_span_count_v3(bundle)?;
    let child_routes = project_child_routes_v5(bundle)?;
    Ok(GeneralSuccessorInstructionV5 {
        hot,
        request,
        outcome_count: product.outcome_count,
        scratch_page_count,
        child_routes,
    })
}

/// Compile a transaction-complete successor instruction into an unsigned v0 message.
pub fn compile_general_successor_v0(
    report: &GeneralSuccessorInstructionV5,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_table: &ObservedAccount,
) -> Result<GeneralSuccessorTransactionPlanV0, GeneralHotOperatorErrorV3> {
    let hot = compile_general_hot_v0(&report.hot, payer, recent_blockhash, lookup_table)?;
    Ok(GeneralSuccessorTransactionPlanV0 {
        hot,
        request: report.request,
        scratch_page_count: report.scratch_page_count,
        child_routes: report.child_routes.clone(),
    })
}

/// Build one complete chain-derived General Hot instruction.
///
/// `request` is re-encoded canonically. Its two bump fields are untrusted
/// placeholders: this constructor replaces them with bumps derived from the
/// authenticated lifecycle policy and the exact observed state addresses. The
/// action-specific account width and privileges come only from the selected
/// AccountProfile; this operator carries no parallel per-action account table.
pub fn build_general_hot_instruction_v3(
    state: &GeneralHotStateV3,
    artifact_selection: GeneralArtifactSelectionV3,
    artifact_bytes: GeneralArtifactBytesV3<'_>,
    request: ControllerRequestV2,
) -> Result<GeneralHotInstructionV3, GeneralHotOperatorErrorV3> {
    let checked = state
        .checked_release
        .ok_or(GeneralHotOperatorErrorV3::UnrecognizedRelease)?;
    validate_release(checked, artifact_selection)?;
    let observation = validate_fixed_frame(state, checked)?;
    let product = authenticate_product_graph(state)?;
    let mut canonical_request = ControllerRequestV2 {
        state_bump: 0,
        terminal_record_bump: 0,
        ..request
    };
    let provisional_request_bytes = canonical_request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    if provisional_request_bytes.len() != CONTROLLER_REQUEST_BYTES_V2 {
        return Err(GeneralHotOperatorErrorV3::Arithmetic);
    }
    let provisional_bundle = authenticate_general_artifacts_v3(
        artifact_selection,
        artifact_bytes,
        &provisional_request_bytes,
        product.outcome_count,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    if provisional_bundle.request != canonical_request {
        return Err(GeneralHotOperatorErrorV3::Artifact);
    }
    let provisional_lifecycle = project_general_lifecycle_v5(
        state,
        provisional_bundle,
        canonical_request,
        checked.trading_program,
    )?;
    canonical_request.state_bump = provisional_lifecycle.primary_state_bump;
    canonical_request.terminal_record_bump = provisional_lifecycle
        .terminal_state_bump
        .unwrap_or_default();
    let request_bytes = canonical_request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    let bundle = authenticate_general_artifacts_v3(
        artifact_selection,
        artifact_bytes,
        &request_bytes,
        product.outcome_count,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    if bundle.request != canonical_request {
        return Err(GeneralHotOperatorErrorV3::Artifact);
    }
    let lifecycle =
        project_general_lifecycle_v5(state, bundle, canonical_request, checked.trading_program)?;
    if lifecycle != provisional_lifecycle {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    validate_strategy_geometry(state, bundle)?;
    validate_runtime_geometry(state, bundle)?;

    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?
        .account
        .key;
    let root = state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(request_bytes.len()).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
        state.release_set,
        market.to_bytes(),
        state.generation,
        hash(&root.account.data).to_bytes(),
    )
    .map_err(|_| GeneralHotOperatorErrorV3::FixedFrame)?;
    let mut data = Vec::with_capacity(
        HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(request_bytes.len())
            .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?,
    );
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(&request_bytes);

    let mut accounts = Vec::with_capacity(
        state
            .fixed_accounts
            .len()
            .checked_add(state.strategy_accounts.len())
            .and_then(|value| value.checked_add(state.runtime_suffix_accounts.len()))
            .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?,
    );
    accounts.extend(
        state
            .fixed_accounts
            .iter()
            .map(GeneralObservedAccountMetaV3::meta),
    );
    accounts.extend(
        state
            .strategy_accounts
            .iter()
            .map(GeneralObservedAccountMetaV3::meta),
    );
    accounts.extend(
        state
            .runtime_suffix_accounts
            .iter()
            .map(GeneralObservedAccountMetaV3::meta),
    );
    let required_instruction_signers = signer_keys(&accounts)?;
    let instruction = Instruction {
        program_id: checked.trading_program,
        accounts,
        data,
    };
    Ok(GeneralHotInstructionV3 {
        instruction,
        action: canonical_request.action,
        outcome_count: product.outcome_count,
        observation,
        required_instruction_signers,
        checked_manifest_digest: checked.checked_manifest_digest,
        trading_artifact_release: checked.trading_artifact_release,
        general_artifact_release: checked.general_artifact_release,
        artifacts: artifact_digests(artifact_bytes),
        product_record: product.product_record,
        family_request_digest: hash(&request_bytes).to_bytes(),
        lifecycle,
    })
}

/// Compile one General instruction into an unsigned packet-safe v0 message.
///
/// Exactly one finalized active LUT is accepted, and its address sequence must
/// equal [`canonical_general_lookup_addresses_v3`]. Extra addresses, alternate
/// ordering, and stale tables refuse even when Solana message compilation could
/// otherwise use them.
pub fn compile_general_hot_v0(
    report: &GeneralHotInstructionV3,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_table: &ObservedAccount,
) -> Result<GeneralHotTransactionPlanV3, GeneralHotOperatorErrorV3> {
    if payer == Pubkey::default()
        || report.observation.finality != Finality::Finalized
        || report.observation.slot == 0
        || lookup_table.observation != report.observation
        || lookup_table.owner != lookup_table_program::id()
        || lookup_table.executable
    {
        return Err(GeneralHotOperatorErrorV3::Snapshot);
    }
    let expected = canonical_general_lookup_addresses_v3(&report.instruction, payer)?;
    let table = AddressLookupTable::deserialize(&lookup_table.data)
        .map_err(|_| GeneralHotOperatorErrorV3::LookupTable)?;
    if table.addresses.as_ref() != expected.as_slice() {
        return Err(GeneralHotOperatorErrorV3::LookupTable);
    }
    let message = compile_v0_message(
        payer,
        core::slice::from_ref(&report.instruction),
        recent_blockhash,
        report.observation,
        core::slice::from_ref(lookup_table),
    )
    .map_err(GeneralHotOperatorErrorV3::Routing)?;
    let mut required_signers = vec![payer];
    for signer in &report.required_instruction_signers {
        if !required_signers.contains(signer) {
            required_signers.push(*signer);
        }
    }
    if usize::from(message.required_signatures) != required_signers.len() {
        return Err(GeneralHotOperatorErrorV3::Signer);
    }
    Ok(GeneralHotTransactionPlanV3 {
        message,
        required_signers,
        action: report.action,
        checked_manifest_digest: report.checked_manifest_digest,
        outcome_count: report.outcome_count,
        trading_artifact_release: report.trading_artifact_release,
        general_artifact_release: report.general_artifact_release,
        artifacts: report.artifacts,
        product_record: report.product_record,
        lifecycle: report.lifecycle,
    })
}

/// Return the exact sorted, duplicate-free LUT address sequence for one General instruction.
pub fn canonical_general_lookup_addresses_v3(
    instruction: &Instruction,
    payer: Pubkey,
) -> Result<Vec<Pubkey>, GeneralHotOperatorErrorV3> {
    let mut signer_keys = vec![payer];
    for account in &instruction.accounts {
        if account.is_signer && !signer_keys.contains(&account.pubkey) {
            signer_keys.push(account.pubkey);
        }
    }
    let mut addresses = instruction
        .accounts
        .iter()
        .filter(|account| {
            !signer_keys.contains(&account.pubkey) && account.pubkey != instruction.program_id
        })
        .map(|account| account.pubkey)
        .collect::<Vec<_>>();
    addresses.sort_unstable_by_key(Pubkey::to_bytes);
    addresses.dedup();
    if addresses.is_empty() || addresses.len() > 256 {
        return Err(GeneralHotOperatorErrorV3::LookupTable);
    }
    Ok(addresses)
}

fn validate_release(
    checked: CheckedGeneralHotReleaseV3,
    selection: GeneralArtifactSelectionV3,
) -> Result<(), GeneralHotOperatorErrorV3> {
    if checked.trading_program == Pubkey::default()
        || checked.trading_artifact_release == [0; 32]
        || checked.general_artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || selection.program_set == [0; 32]
        || selection.config == [0; 32]
        || selection.artifact_release != checked.general_artifact_release
    {
        return Err(GeneralHotOperatorErrorV3::UnrecognizedRelease);
    }
    Ok(())
}

fn derive_general_request_v5(
    state: &GeneralHotStateV3,
    selection: GeneralArtifactSelectionV3,
    artifacts: GeneralArtifactBytesV3<'_>,
    action: Action,
    outcome_count: u32,
    product_record: [u8; 32],
) -> Result<ControllerRequestV2, GeneralHotOperatorErrorV3> {
    if hash(artifacts.config).to_bytes() != selection.config {
        return Err(GeneralHotOperatorErrorV3::ContentIdentity);
    }
    let config = GeneralConfigV3::decode(artifacts.config)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if config.program_set_id() != selection.program_set {
        return Err(GeneralHotOperatorErrorV3::ContentIdentity);
    }
    match action {
        Action::Consider => {
            derive_consider_request_v5(state, config, outcome_count, product_record)
        }
        Action::Freeze => derive_freeze_request_v5(state, config, outcome_count, product_record),
        Action::InitializeSettlement => {
            derive_initialize_request_v5(state, config, outcome_count, product_record)
        }
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
            derive_settlement_request_v5(state, config, action, outcome_count, product_record)
        }
    }
}

fn derive_consider_request_v5(
    state: &GeneralHotStateV3,
    config: GeneralConfigV3,
    outcome_count: u32,
    product_record: [u8; 32],
) -> Result<ControllerRequestV2, GeneralHotOperatorErrorV3> {
    let policy_account = readonly_evidence_account_v5(
        state,
        Action::Consider,
        GeneralReadonlyEvidenceKindV3::SelectionPolicy,
    )?;
    let submitted_account = readonly_evidence_account_v5(
        state,
        Action::Consider,
        GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate,
    )?;
    let policy = SelectionPolicyV1::decode(&policy_account.account.data)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let submitted = VerifiedCandidateV2::decode(&submitted_account.account.data)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let submitted_header = submitted.header();
    if policy.policy_id != config.selection_policy_id()
        || submitted_header.outcome_count != outcome_count
        || submitted_header.product_id != product_record
        || submitted_header.price_scale != config.price_scale()
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    let vacant = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let selection_before =
        primary_state_body_v5(state, GeneralLocalStateKindV3::Selection)?.unwrap_or(&vacant);
    let expected_revision = if selection_before.iter().all(|byte| *byte == 0) {
        0
    } else {
        let current = RuntimeSelectionCursorV2::decode(selection_before)
            .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
        let header = current.header();
        if header.outcome_count != outcome_count
            || header.policy_id != policy.policy_id
            || header.product_id != submitted_header.product_id
            || header.batch_id != submitted_header.batch_id
            || header.price_scale != submitted_header.price_scale
        {
            return Err(GeneralHotOperatorErrorV3::ChainState);
        }
        header.revision
    };
    let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut output = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    consider_verified_candidate_v2(
        policy,
        selection_before,
        submitted.as_bytes(),
        expected_revision,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    Ok(ControllerRequestV2 {
        action: Action::Consider,
        expected_revision,
        candidate_id: Some(submitted_header.candidate_id),
        page_index: submitted_header.candidate_coordinate,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: 0,
        terminal_record_bump: 0,
    })
}

fn derive_freeze_request_v5(
    state: &GeneralHotStateV3,
    config: GeneralConfigV3,
    outcome_count: u32,
    product_record: [u8; 32],
) -> Result<ControllerRequestV2, GeneralHotOperatorErrorV3> {
    let selection_before = primary_state_body_v5(state, GeneralLocalStateKindV3::Selection)?
        .ok_or(GeneralHotOperatorErrorV3::ChainState)?;
    let selection = RuntimeSelectionCursorV2::decode(selection_before)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let selection_header = selection.header();
    if selection_header.outcome_count != outcome_count
        || selection_header.product_id != product_record
        || selection_header.policy_id != config.selection_policy_id()
        || selection_header.price_scale != config.price_scale()
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    let expected_revision = selection_header.revision;
    let mut scratch = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    let mut output = [0_u8; RUNTIME_SELECTION_CURSOR_BYTES_V2];
    freeze_selection_v2(
        selection_before,
        expected_revision,
        &mut scratch,
        &mut output,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    Ok(ControllerRequestV2 {
        action: Action::Freeze,
        expected_revision,
        candidate_id: None,
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: 0,
        terminal_record_bump: 0,
    })
}

fn derive_initialize_request_v5(
    state: &GeneralHotStateV3,
    config: GeneralConfigV3,
    outcome_count: u32,
    product_record: [u8; 32],
) -> Result<ControllerRequestV2, GeneralHotOperatorErrorV3> {
    if primary_state_body_v5(state, GeneralLocalStateKindV3::Settlement)?.is_some() {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    let frozen = readonly_evidence_account_v5(
        state,
        Action::InitializeSettlement,
        GeneralReadonlyEvidenceKindV3::FrozenSelection,
    )?;
    let verifier_account = readonly_evidence_account_v5(
        state,
        Action::InitializeSettlement,
        GeneralReadonlyEvidenceKindV3::RuntimeVerifier,
    )?;
    let verified_account = readonly_evidence_account_v5(
        state,
        Action::InitializeSettlement,
        GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
    )?;
    let verified = VerifiedCandidateV2::decode(&verified_account.account.data)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let verified_header = verified.header();
    if verified_header.product_id != product_record
        || verified_header.price_scale != config.price_scale()
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    authenticate_frozen_selection_v3(
        config.selection_policy_id(),
        product_record,
        config.price_scale(),
        Some(verified_header.candidate_id),
        outcome_count,
        &frozen.account.data,
        verified.as_bytes(),
    )
    .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let verifier = RuntimeCandidateVerifierV2::decode(&verifier_account.account.data)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if !verifier.is_complete()
        || verifier.header().outcome_count != outcome_count
        || verifier.header().candidate_id != verified_header.candidate_id
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    let mut cursor = vec![
        0_u8;
        settlement_cursor_len(outcome_count)
            .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?
    ];
    initialize_runtime_settlement_in_place_v2(
        &verifier_account.account.data,
        verified.as_bytes(),
        0,
        &mut cursor,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    Ok(ControllerRequestV2 {
        action: Action::InitializeSettlement,
        expected_revision: 0,
        candidate_id: Some(verified_header.candidate_id),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: 0,
        terminal_record_bump: 0,
    })
}

fn derive_settlement_request_v5(
    state: &GeneralHotStateV3,
    config: GeneralConfigV3,
    action: Action,
    outcome_count: u32,
    product_record: [u8; 32],
) -> Result<ControllerRequestV2, GeneralHotOperatorErrorV3> {
    let cursor_bytes = primary_state_body_v5(state, GeneralLocalStateKindV3::Settlement)?
        .ok_or(GeneralHotOperatorErrorV3::ChainState)?;
    let cursor = SettlementCursorV2::decode(cursor_bytes)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let cursor_header = cursor.header();
    let verified_account = readonly_evidence_account_v5(
        state,
        action,
        GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate,
    )?;
    let verified = VerifiedCandidateV2::decode(&verified_account.account.data)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let verified_header = verified.header();
    if cursor_header.outcome_count != outcome_count
        || verified_header.outcome_count != outcome_count
        || cursor_header.candidate_id != verified_header.candidate_id
        || verified_header.product_id != product_record
        || verified_header.price_scale != config.price_scale()
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    let settlement_action = match action {
        Action::Collect => RuntimeSettlementActionV2::Collect,
        Action::Materialize => RuntimeSettlementActionV2::Materialize,
        Action::Distribute => RuntimeSettlementActionV2::Distribute,
        Action::Close => RuntimeSettlementActionV2::Close,
        Action::Consider | Action::Freeze | Action::InitializeSettlement => {
            return Err(GeneralHotOperatorErrorV3::ChainState);
        }
    };
    let (manifest, page_index, execution_index, manifest_order_index) =
        if matches!(action, Action::Collect | Action::Distribute) {
            let account = readonly_evidence_account_v5(
                state,
                action,
                GeneralReadonlyEvidenceKindV3::SettlementManifest,
            )?;
            let manifest = SettlementManifestV2::decode(&account.account.data)
                .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            let header = manifest.header();
            if header.outcome_count != outcome_count
                || header.candidate_id != verified_header.candidate_id
                || header.candidate_coordinate != verified_header.candidate_coordinate
            {
                return Err(GeneralHotOperatorErrorV3::ChainState);
            }
            let expected_order = cursor_header
                .next_order
                .checked_add(1)
                .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
            let mut selected = None;
            for ordinal in 0..header.order_count {
                if manifest
                    .order(ordinal)
                    .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?
                    .header()
                    .order_coordinate
                    == expected_order
                {
                    selected = Some(ordinal);
                    break;
                }
            }
            let selected = selected.ok_or(GeneralHotOperatorErrorV3::ChainState)?;
            let selected_order = manifest
                .order(selected)
                .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            let selected_header = selected_order.header();
            let execution_index = u8::try_from(selected_header.source_execution_index)
                .map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?;
            let manifest_order_index =
                u8::try_from(selected).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?;
            (
                Some(manifest.as_bytes()),
                selected_header.source_page_index,
                execution_index,
                manifest_order_index,
            )
        } else {
            (None, 0, 0, 0)
        };
    let inventory_bytes = usize::try_from(outcome_count)
        .map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?
        .checked_mul(8)
        .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
    let mut cursor_workspace = vec![0_u8; cursor_bytes.len()];
    let mut inventory_workspace = vec![0_u8; inventory_bytes];
    let mut effect_workspace = vec![
        0_u8;
        runtime_settlement_effect_len_v2(outcome_count)
            .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?
    ];
    evaluate_runtime_settlement_in_place_v2(
        RuntimeSettlementViewV2 {
            action: settlement_action,
            cursor_before: cursor_bytes,
            verified: verified.as_bytes(),
            manifest,
            manifest_order_index: u32::from(manifest_order_index),
            expected_revision: cursor_header.revision,
            surplus_beneficiary: if action == Action::Close {
                Some(config.quote_surplus_beneficiary())
            } else {
                None
            },
        },
        &mut cursor_workspace,
        &mut inventory_workspace,
        &mut effect_workspace,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    Ok(ControllerRequestV2 {
        action,
        expected_revision: cursor_header.revision,
        candidate_id: Some(verified_header.candidate_id),
        page_index,
        execution_index,
        manifest_order_index,
        state_bump: 0,
        terminal_record_bump: 0,
    })
}

fn primary_state_body_v5(
    state: &GeneralHotStateV3,
    expected_kind: GeneralLocalStateKindV3,
) -> Result<Option<&[u8]>, GeneralHotOperatorErrorV3> {
    let account = logical_runtime_account(state, usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3))?;
    if account.account.data.is_empty() {
        return Ok(None);
    }
    let decoded = GeneralLocalStateV3::decode(&account.account.data)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if decoded.header().kind != expected_kind {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    Ok(Some(decoded.body()))
}

fn readonly_evidence_account_v5(
    state: &GeneralHotStateV3,
    action: Action,
    expected_kind: GeneralReadonlyEvidenceKindV3,
) -> Result<&GeneralObservedAccountMetaV3, GeneralHotOperatorErrorV3> {
    let mut index = 0_u16;
    while index < general_readonly_evidence_count_v3(action) {
        let evidence = general_readonly_evidence_v3(action, index)
            .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
        if evidence.kind == expected_kind {
            let account = logical_runtime_account(state, usize::from(evidence.coordinate))?;
            if account.is_signer || account.is_writable || account.account.executable {
                return Err(GeneralHotOperatorErrorV3::ChainState);
            }
            return Ok(account);
        }
        index = index
            .checked_add(1)
            .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
    }
    Err(GeneralHotOperatorErrorV3::ChainState)
}

fn project_child_routes_v5(
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
) -> Result<Vec<GeneralChildRouteV5>, GeneralHotOperatorErrorV3> {
    let mut output = Vec::with_capacity(usize::from(bundle.effect.route_count()));
    for route_index in 0..bundle.effect.route_count() {
        let route = bundle
            .effect
            .route(route_index)
            .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
        let selected = general_effect_route_frame_v3(bundle.request.action, route_index)
            .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
        let expected_role = match selected.frame {
            GeneralChildFrameV3::ClaimsProtocolPosition(_)
            | GeneralChildFrameV3::ClaimsAffine { .. } => FixedRole::Claims,
            GeneralChildFrameV3::Custody(_) => FixedRole::Custody,
        };
        if route.role() != expected_role || route.fixed_account_start() != selected.account_start {
            return Err(GeneralHotOperatorErrorV3::Artifact);
        }
        let mut receipt_dependencies =
            Vec::with_capacity(usize::from(route.receipt_dependency_count()));
        for dependency_index in 0..route.receipt_dependency_count() {
            let dependency = bundle
                .effect
                .route_receipt_dependency(route_index, dependency_index)
                .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
            receipt_dependencies.push(GeneralReceiptDependencyV5 {
                producer_role: dependency.producer_role(),
                producer_route: dependency.producer_route(),
                expected_receipt_bytes: dependency.expected_receipt_bytes(),
            });
        }
        output.push(GeneralChildRouteV5 {
            route: route_index,
            role: expected_role,
            account_start: selected.account_start,
            account_count: selected
                .frame
                .account_count()
                .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?,
            receipt_dependencies,
        });
    }
    Ok(output)
}

fn artifact_digests(bytes: GeneralArtifactBytesV3<'_>) -> GeneralHotArtifactDigestsV3 {
    GeneralHotArtifactDigestsV3 {
        program_set: hash(bytes.program_set).to_bytes(),
        descriptor: hash(bytes.descriptor).to_bytes(),
        config: hash(bytes.config).to_bytes(),
        account_profile: hash(bytes.account_profile).to_bytes(),
        lifecycle_policy: hash(bytes.lifecycle_policy).to_bytes(),
        request_profile: hash(bytes.request_profile).to_bytes(),
        strategy: hash(bytes.strategy).to_bytes(),
        certificate: hash(bytes.certificate).to_bytes(),
        admission: hash(bytes.admission).to_bytes(),
        transition: hash(bytes.transition).to_bytes(),
        effect: hash(bytes.effect).to_bytes(),
    }
}

fn validate_fixed_frame(
    state: &GeneralHotStateV3,
    checked: CheckedGeneralHotReleaseV3,
) -> Result<Observation, GeneralHotOperatorErrorV3> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || state.release_set == [0; 32]
        || state.minimum_finalized_slot == 0
    {
        return Err(GeneralHotOperatorErrorV3::FixedFrame);
    }
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let trading = state
        .fixed_accounts
        .get(HOT_TRADING_PROGRAM_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let registry = state
        .fixed_accounts
        .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let rent = state
        .fixed_accounts
        .get(HOT_RENT_SYSVAR_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let instructions = state
        .fixed_accounts
        .get(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    if market.account.key == Pubkey::default()
        || trading.account.key != checked.trading_program
        || !trading.account.executable
        || !registry.account.executable
        || rent.account.key != sysvar::rent::ID
        || instructions.account.key != sysvar::instructions::ID
    {
        return Err(GeneralHotOperatorErrorV3::FixedFrame);
    }
    let observation = market.account.observation;
    if observation.finality != Finality::Finalized
        || observation.slot < state.minimum_finalized_slot
    {
        return Err(GeneralHotOperatorErrorV3::Snapshot);
    }
    for (index, value) in state.fixed_accounts.iter().enumerate() {
        if value.account.observation != observation
            || value.is_signer
            || value.is_writable != (index == HOT_ROOT_ACCOUNT_V3)
        {
            return Err(GeneralHotOperatorErrorV3::FixedFrame);
        }
    }
    for value in state
        .strategy_accounts
        .iter()
        .chain(&state.runtime_suffix_accounts)
    {
        if value.account.observation != observation
            || value.account.observation.finality != Finality::Finalized
        {
            return Err(GeneralHotOperatorErrorV3::Snapshot);
        }
    }
    Ok(observation)
}

fn authenticate_product_graph(
    state: &GeneralHotStateV3,
) -> Result<AuthenticatedProductGraphObservationV3, GeneralHotOperatorErrorV3> {
    let registry = state
        .fixed_accounts
        .get(HOT_REGISTRY_PROGRAM_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::Product)?
        .account
        .key;
    let account = |index: usize| {
        state
            .fixed_accounts
            .get(index)
            .map(|value| &value.account)
            .ok_or(GeneralHotOperatorErrorV3::Product)
    };
    authenticate_product_graph_observation_v3(FinalizedProductGraphAccountsV3 {
        registry_program: registry,
        product_raw: account(HOT_PRODUCT_RAW_ACCOUNT_V3)?,
        product_staging: account(HOT_PRODUCT_RAW_ACCOUNT_V3 + 1)?,
        domain_raw: account(HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3)?,
        domain_staging: account(HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3 + 1)?,
        portfolio_raw: account(HOT_PORTFOLIO_RAW_ACCOUNT_V3)?,
        portfolio_staging: account(HOT_PORTFOLIO_RAW_ACCOUNT_V3 + 1)?,
    })
    .map_err(|_| GeneralHotOperatorErrorV3::Product)
}

fn validate_strategy_geometry(
    state: &GeneralHotStateV3,
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
) -> Result<(), GeneralHotOperatorErrorV3> {
    let caller_count = usize::try_from(selected_bank_span_count_v3(bundle)?)
        .map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?;
    let expected = ADMITTED_AOT_FIXED_EXTRAS_V3
        .checked_add(caller_count)
        .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
    if state.strategy_accounts.len() != expected
        || state
            .strategy_accounts
            .iter()
            .any(|account| account.is_signer || account.is_writable)
        || state
            .strategy_accounts
            .get(ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3)
            .is_none_or(|account| !account.account.executable)
    {
        return Err(GeneralHotOperatorErrorV3::StrategyGeometry);
    }
    Ok(())
}

fn selected_bank_span_count_v3(
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
) -> Result<u32, GeneralHotOperatorErrorV3> {
    let scalar_count = bundle
        .effect
        .scalar_count(bundle.tail_count)
        .map_err(|_| GeneralHotOperatorErrorV3::StrategyGeometry)?;
    let identity_count = bundle
        .effect
        .identity_count(bundle.tail_count)
        .map_err(|_| GeneralHotOperatorErrorV3::StrategyGeometry)?;
    match classify_bank_transport_v2(
        u32::try_from(scalar_count).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
        u32::try_from(identity_count).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::StrategyGeometry)?
    {
        BankTransportV2::InlineReturnData { bank_bytes } if bank_bytes != 0 => Ok(1),
        BankTransportV2::AuthenticatedScratchPages { page_count, .. } if page_count != 0 => {
            Ok(page_count)
        }
        BankTransportV2::InlineReturnData { .. } => {
            Err(GeneralHotOperatorErrorV3::StrategyGeometry)
        }
        BankTransportV2::AuthenticatedScratchPages { .. } => {
            Err(GeneralHotOperatorErrorV3::StrategyGeometry)
        }
    }
}

fn validate_runtime_geometry(
    state: &GeneralHotStateV3,
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
) -> Result<(), GeneralHotOperatorErrorV3> {
    let profile = bundle.account_profile;
    if profile.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || profile.dynamic_fixed_span_count() != 1
    {
        return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
    }
    let span_counts = [selected_bank_span_count_v3(bundle)?];
    let logical_count = profile
        .logical_account_count_with_dynamic_spans(bundle.tail_count, &span_counts)
        .map_err(|_| GeneralHotOperatorErrorV3::RuntimeGeometry)?;
    let physical_count = profile
        .physical_account_count_with_dynamic_spans(bundle.tail_count, &span_counts)
        .map_err(|_| GeneralHotOperatorErrorV3::RuntimeGeometry)?;
    if logical_count < HOT_RUNTIME_LOGICAL_PREFIX_V3
        || physical_count < HOT_RUNTIME_LOGICAL_PREFIX_V3
        || state.runtime_suffix_accounts.len()
            != physical_count
                .checked_sub(HOT_RUNTIME_LOGICAL_PREFIX_V3)
                .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)?
    {
        return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
    }
    let mut ordinal = 0_usize;
    while ordinal < physical_count {
        let account = physical_runtime_account(state, ordinal)?;
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(bundle.tail_count, &span_counts, ordinal)
            .map_err(|_| GeneralHotOperatorErrorV3::RuntimeGeometry)?;
        let privileges = geometry.privileges();
        if account.is_signer != privileges.signer()
            || account.is_writable != privileges.writable()
            || account.account.executable != privileges.executable()
            || !physical_data_matches_v3(geometry.data(), account.account.data.len())
        {
            return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
        }
        let mut prior = 0_usize;
        while prior < ordinal {
            if physical_runtime_account(state, prior)?.account.key == account.account.key {
                return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
            }
            prior = prior
                .checked_add(1)
                .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
    }
    let span = profile
        .dynamic_fixed_span(0)
        .map_err(|_| GeneralHotOperatorErrorV3::RuntimeGeometry)?;
    if span.count_scalar()
        != u16::try_from(general_scalar::INPUT_SCRATCH_PAGE_COUNT)
            .map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?
        || usize::from(span.insertion_coordinate()).checked_add(
            usize::try_from(span_counts[0]).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
        ) != Some(logical_count)
    {
        return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
    }
    Ok(())
}

fn physical_data_matches_v3(geometry: PhysicalAccountDataGeometryV2, actual: usize) -> bool {
    match geometry {
        PhysicalAccountDataGeometryV2::Exact { bytes } => actual == bytes,
        PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => {
            actual == 0 || actual == live_bytes
        }
        PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
            actual >= minimum_bytes
        }
        PhysicalAccountDataGeometryV2::Opaque => true,
    }
}

fn physical_runtime_account(
    state: &GeneralHotStateV3,
    physical_ordinal: usize,
) -> Result<&GeneralObservedAccountMetaV3, GeneralHotOperatorErrorV3> {
    if physical_ordinal < HOT_RUNTIME_LOGICAL_PREFIX_V3 {
        return logical_runtime_account(state, physical_ordinal);
    }
    state
        .runtime_suffix_accounts
        .get(
            physical_ordinal
                .checked_sub(HOT_RUNTIME_LOGICAL_PREFIX_V3)
                .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)?,
        )
        .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)
}

fn logical_runtime_account(
    state: &GeneralHotStateV3,
    coordinate: usize,
) -> Result<&GeneralObservedAccountMetaV3, GeneralHotOperatorErrorV3> {
    let fixed_index = match coordinate {
        0 => Some(HOT_ROOT_ACCOUNT_V3),
        1 => Some(HOT_CONFIG_RAW_ACCOUNT_V3),
        2 => Some(HOT_PRODUCT_RAW_ACCOUNT_V3),
        3 => Some(HOT_PORTFOLIO_RAW_ACCOUNT_V3),
        4 => Some(HOT_LINKED_BASIS_RAW_ACCOUNT_V3),
        _ => None,
    };
    if let Some(index) = fixed_index {
        state
            .fixed_accounts
            .get(index)
            .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)
    } else {
        state
            .runtime_suffix_accounts
            .get(
                coordinate
                    .checked_sub(HOT_RUNTIME_LOGICAL_PREFIX_V3)
                    .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)?,
            )
            .ok_or(GeneralHotOperatorErrorV3::RuntimeGeometry)
    }
}

fn project_general_lifecycle_v5(
    state: &GeneralHotStateV3,
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
    request: ControllerRequestV2,
    trading_program: Pubkey,
) -> Result<GeneralLifecycleProjectionV3, GeneralHotOperatorErrorV3> {
    let policy_bytes = general_state_lifecycle_bytes_v5(request.action)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let mut scratch = vec![0_u8; policy_bytes];
    let mut canonical = vec![0_u8; policy_bytes];
    let child_widths = selected_child_rent_widths_v5(bundle)?;
    encode_general_state_lifecycle_v5_atomic(
        request.action,
        child_widths,
        &mut scratch,
        &mut canonical,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    if bundle.lifecycle_policy.bytes() != canonical.as_slice() {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }

    let plan_count = bundle
        .lifecycle_policy
        .action_plan_count(request.action as u32)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let expected_plan_count = if request.action == Action::Close {
        2
    } else {
        1
    };
    if plan_count != expected_plan_count {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let scalar_width = affine_register_width(
        bundle.account_profile.common_scalar_count(),
        bundle.account_profile.item_scalar_stride(),
        bundle.tail_count,
    )?;
    let identity_width = affine_register_width(
        bundle.account_profile.common_identity_count(),
        bundle.account_profile.item_identity_stride(),
        bundle.tail_count,
    )?;
    let mut scalars = vec![0_u64; scalar_width];
    let mut identities = vec![[0_u8; 32]; identity_width];
    let root = logical_runtime_account(state, 0)?.account.key;
    set_identity(
        &mut identities,
        general_identity::GENERAL_ROOT,
        root.to_bytes(),
    )?;
    set_identity(
        &mut identities,
        general_identity::CANDIDATE,
        request.candidate_id.unwrap_or([0; 32]),
    )?;
    let terminal_coordinate =
        canonical_terminal_coordinate_v3(request.action, request.expected_revision)?;
    if let Some(value) = terminal_coordinate {
        set_scalar(
            &mut scalars,
            general_scalar::CURSOR_TERMINAL_COORDINATE,
            value,
        )?;
    }
    let registers = LifecycleRegistersV3 {
        scalars: &scalars,
        identities: &identities,
    };
    let primary_plan = bundle
        .lifecycle_policy
        .action_plan(request.action as u32, 0)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let primary_expected_operation = if request.action == Action::Close {
        LifecycleOperationV3::Close
    } else {
        LifecycleOperationV3::AuthenticateOrCreate
    };
    let primary = derive_lifecycle_state_v3(
        state,
        bundle.account_profile,
        bundle.tail_count,
        registers,
        primary_plan,
        primary_expected_operation,
        usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
        if request.action == Action::Close {
            None
        } else {
            Some(usize::from(GENERAL_PRIMARY_PAYER_ACCOUNT_V3))
        },
        Some(usize::from(if request.action == Action::Close {
            GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3
        } else {
            GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3
        })),
        trading_program,
        if matches!(request.action, Action::Consider | Action::Freeze) {
            GeneralLocalStateKindV3::Selection
        } else {
            GeneralLocalStateKindV3::Settlement
        },
    )?;

    let terminal = if request.action == Action::Close {
        let plan = bundle
            .lifecycle_policy
            .action_plan(request.action as u32, 1)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
        let terminal = derive_lifecycle_state_v3(
            state,
            bundle.account_profile,
            bundle.tail_count,
            registers,
            plan,
            LifecycleOperationV3::AuthenticateOrCreate,
            usize::from(GENERAL_TERMINAL_STATE_ACCOUNT_V3),
            Some(usize::from(GENERAL_CLOSE_PAYER_ACCOUNT_V3)),
            Some(usize::from(GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3)),
            trading_program,
            GeneralLocalStateKindV3::Settlement,
        )?;
        if terminal.key == primary.key {
            return Err(GeneralHotOperatorErrorV3::Lifecycle);
        }
        Some(terminal)
    } else {
        None
    };
    let child_account_start = general_child_account_start_v3(request.action);
    let expected_child_start = if request.action == Action::Close {
        9
    } else {
        8
    };
    if child_account_start != expected_child_start {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let projection = GeneralLifecycleProjectionV3 {
        primary_state: primary.key,
        primary_state_bump: primary.bump,
        terminal_state: terminal.map(|value| value.key),
        terminal_state_bump: terminal.map(|value| value.bump),
        terminal_coordinate,
        child_account_start,
    };
    if (request.state_bump != 0 && request.state_bump != projection.primary_state_bump)
        || (request.terminal_record_bump != 0
            && Some(request.terminal_record_bump) != projection.terminal_state_bump)
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    Ok(projection)
}

fn selected_child_rent_widths_v5(
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
) -> Result<Option<GeneralChildRentWidthsV5>, GeneralHotOperatorErrorV3> {
    if bundle.request.action != Action::InitializeSettlement {
        if bundle.lifecycle_policy.current_rent_quote_count() != 0 {
            return Err(GeneralHotOperatorErrorV3::Lifecycle);
        }
        return Ok(None);
    }
    let expected_destinations = [
        general_scalar::POSITION_RENT_PRINCIPAL,
        general_scalar::ADMISSION_RENT_PRINCIPAL,
        general_scalar::CUSTODY_REPLAY_RENT_LAMPORTS,
        general_scalar::CUSTODY_VAULT_RENT_LAMPORTS,
    ];
    if usize::from(bundle.lifecycle_policy.current_rent_quote_count())
        != expected_destinations.len()
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    // Product N and the semantic-owner fixed child widths are regenerated by
    // `GeneralChildRentWidthsV5` and the canonical V5 encoder below. The sole
    // release-variable input is the selected vault width committed by the V5
    // policy; it is never taken from the family request or GeneralConfig.
    let mut vault_width = None;
    for (ordinal, expected_destination) in expected_destinations.into_iter().enumerate() {
        let quote = bundle
            .lifecycle_policy
            .current_rent_quote(
                u16::try_from(ordinal).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
            )
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
        let destination = quote.scalar_destination();
        if destination.kind() != LifecycleRegisterKindV3::Scalar
            || destination.scope() != CoordinateScopeV3::Fixed
            || u32::from(destination.index()) != expected_destination
            || quote.exact_data_len() == 0
        {
            return Err(GeneralHotOperatorErrorV3::Lifecycle);
        }
        if ordinal == 3 {
            vault_width = Some(quote.exact_data_len());
        }
    }
    GeneralChildRentWidthsV5::new(
        bundle.tail_count,
        vault_width.ok_or(GeneralHotOperatorErrorV3::Lifecycle)?,
    )
    .map(Some)
    .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DerivedLifecycleStateV3 {
    key: Pubkey,
    bump: u8,
}

#[allow(clippy::too_many_arguments)]
fn derive_lifecycle_state_v3(
    state: &GeneralHotStateV3,
    profile: dclutch_account_profile_contract::v2::AccountProfileV2<'_>,
    tail_count: u32,
    registers: LifecycleRegistersV3<'_>,
    selected: SelectedLifecycleV3<'_>,
    expected_operation: LifecycleOperationV3,
    expected_state: usize,
    expected_payer: Option<usize>,
    expected_rent_credit: Option<usize>,
    trading_program: Pubkey,
    state_kind: GeneralLocalStateKindV3,
) -> Result<DerivedLifecycleStateV3, GeneralHotOperatorErrorV3> {
    if selected.operation() != expected_operation
        || selected.invocation_count(tail_count).ok() != Some(1)
        || selected.invocation_item(tail_count, 0).ok() != Some(None)
        || !selected
            .uses_canonical_bump()
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let indices = selected
        .project_account_indices(profile, tail_count, None)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    if indices.state() != expected_state
        || indices.payer() != expected_payer
        || indices.rent_credit() != expected_rent_credit
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let seed_count = selected
        .seed_count()
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let mut seed_values = Vec::with_capacity(
        usize::from(seed_count)
            .checked_sub(1)
            .ok_or(GeneralHotOperatorErrorV3::Lifecycle)?,
    );
    let mut saw_bump = false;
    for ordinal in 0..seed_count {
        match selected
            .materialize_seed_input(profile, tail_count, None, registers, ordinal)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?
        {
            LifecycleSeedInputValueV3::Bytes(value) if !saw_bump && !value.is_empty() => {
                seed_values.push(value.as_slice().to_vec());
            }
            LifecycleSeedInputValueV3::CanonicalBump
                if !saw_bump && ordinal.checked_add(1) == Some(seed_count) =>
            {
                saw_bump = true;
            }
            LifecycleSeedInputValueV3::Bytes(_) | LifecycleSeedInputValueV3::CanonicalBump => {
                return Err(GeneralHotOperatorErrorV3::Lifecycle);
            }
        }
    }
    if !saw_bump || seed_values.is_empty() {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let seed_refs = seed_values.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let (key, bump) = Pubkey::find_program_address(&seed_refs, &trading_program);
    let state_account = logical_runtime_account(state, indices.state())?;
    if state_account.account.key != key
        || state_account.account.key == Pubkey::default()
        || state_account.is_signer
        || !state_account.is_writable
        || state_account.account.executable
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let data_bytes = usize::try_from(
        selected
            .target_data_bytes(tail_count)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?;
    let live = state_account.account.owner == trading_program
        && state_account.account.data.len() == data_bytes;
    let vacant =
        state_account.account.owner == system_program::ID && state_account.account.data.is_empty();
    let accepted = match expected_operation {
        LifecycleOperationV3::Authenticate | LifecycleOperationV3::Close => live,
        LifecycleOperationV3::Create => vacant,
        LifecycleOperationV3::AuthenticateOrCreate => live || vacant,
    };
    if !accepted {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    if live {
        let decoded = GeneralLocalStateV3::decode(&state_account.account.data)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
        if decoded.header().bump != bump || decoded.header().kind != state_kind {
            return Err(GeneralHotOperatorErrorV3::Lifecycle);
        }
    }
    validate_lifecycle_funding_accounts(
        state,
        indices.state(),
        indices.payer(),
        indices.rent_credit(),
    )?;
    Ok(DerivedLifecycleStateV3 { key, bump })
}

fn canonical_terminal_coordinate_v3(
    action: Action,
    expected_revision: u64,
) -> Result<Option<u64>, GeneralHotOperatorErrorV3> {
    if action == Action::Close {
        expected_revision
            .checked_add(1)
            .map(Some)
            .ok_or(GeneralHotOperatorErrorV3::Arithmetic)
    } else {
        Ok(None)
    }
}

fn validate_lifecycle_funding_accounts(
    state: &GeneralHotStateV3,
    state_index: usize,
    payer_index: Option<usize>,
    rent_credit_index: Option<usize>,
) -> Result<(), GeneralHotOperatorErrorV3> {
    let state_key = logical_runtime_account(state, state_index)?.account.key;
    let payer_key = payer_index
        .map(|index| {
            let payer = logical_runtime_account(state, index)?;
            if payer.account.key == Pubkey::default()
                || !payer.is_signer
                || !payer.is_writable
                || payer.account.executable
            {
                return Err(GeneralHotOperatorErrorV3::Lifecycle);
            }
            Ok(payer.account.key)
        })
        .transpose()?;
    let rent_credit_key = rent_credit_index
        .map(|index| {
            let credit = logical_runtime_account(state, index)?;
            if credit.account.key == Pubkey::default()
                || credit.is_signer
                || !credit.is_writable
                || credit.account.executable
            {
                return Err(GeneralHotOperatorErrorV3::Lifecycle);
            }
            Ok(credit.account.key)
        })
        .transpose()?;
    if payer_key == Some(state_key)
        || rent_credit_key == Some(state_key)
        || payer_key.is_some() && payer_key == rent_credit_key
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    Ok(())
}

fn affine_register_width(
    common: u16,
    stride: u16,
    tail_count: u32,
) -> Result<usize, GeneralHotOperatorErrorV3> {
    usize::from(stride)
        .checked_mul(
            usize::try_from(tail_count).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
        )
        .and_then(|tail| usize::from(common).checked_add(tail))
        .ok_or(GeneralHotOperatorErrorV3::Arithmetic)
}

fn set_scalar(
    scalars: &mut [u64],
    coordinate: u32,
    value: u64,
) -> Result<(), GeneralHotOperatorErrorV3> {
    *scalars
        .get_mut(usize::try_from(coordinate).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?)
        .ok_or(GeneralHotOperatorErrorV3::Lifecycle)? = value;
    Ok(())
}

fn set_identity(
    identities: &mut [[u8; 32]],
    coordinate: u32,
    value: [u8; 32],
) -> Result<(), GeneralHotOperatorErrorV3> {
    *identities
        .get_mut(usize::try_from(coordinate).map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?)
        .ok_or(GeneralHotOperatorErrorV3::Lifecycle)? = value;
    Ok(())
}

fn signer_keys(accounts: &[AccountMeta]) -> Result<Vec<Pubkey>, GeneralHotOperatorErrorV3> {
    let mut signers = Vec::new();
    for account in accounts.iter().filter(|account| account.is_signer) {
        if account.pubkey == Pubkey::default() {
            return Err(GeneralHotOperatorErrorV3::Signer);
        }
        if !signers.contains(&account.pubkey) {
            signers.push(account.pubkey);
        }
    }
    Ok(signers)
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use dclutch_account_profile_contract::lifecycle_v3::{
        Error as LifecycleErrorV3, StateLifecyclePolicyV5,
    };
    use dclutch_account_profile_contract::v2::encode::{
        AccountAliasInputV2, AccountPrivilegesV2, AccountRuleWithPrestateInputV2,
    };
    use dclutch_execution_strategy_contract::admitted_v3::ADMITTED_RUNTIME_ACCOUNTS_START_V3;
    use dclutch_general_adapter_contract::account_rules_v3::{
        GeneralExternalAccountWidthsV3, general_account_profile_fixed_count_v3,
        general_account_profile_rule_v3, general_scratch_page_rule_v3,
    };
    use dclutch_general_adapter_contract::hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_scalar_count_v3,
    };
    use dclutch_general_adapter_contract::release_v3::GENERAL_ACTIONS_V3;
    use dclutch_general_adapter_contract::runtime_width::{
        SettlementCursorHeaderV2, SettlementPhaseV2, VerifiedCandidateHeaderV2,
        settlement_cursor_len, verified_candidate_len,
    };
    use dclutch_general_adapter_contract::state_artifacts_v3::{
        encode_general_state_lifecycle_v3_atomic, general_state_lifecycle_bytes_v3,
    };
    use dclutch_general_adapter_contract::{
        local_state_v3::{
            GeneralLocalStateHeaderV3, encode_general_local_state_v3_atomic,
            general_local_state_len_v3,
        },
        runtime_manifest::settlement_manifest_len_v2,
    };
    use dclutch_general_codec::{MAX_SELECTION_CRITERIA, SelectionCriterion};
    use dclutch_general_config_contract::v3::GeneralConfigV3Input;

    use super::*;
    use solana_address_lookup_table_interface::state::LookupTableMeta;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn observation() -> Observation {
        Observation {
            slot: 500,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn report(outcome_count: u32, data_bytes: usize) -> GeneralHotInstructionV3 {
        let actor = key(1);
        let mut accounts = vec![AccountMeta::new_readonly(actor, true)];
        accounts.extend((2_u8..92).map(|value| AccountMeta::new(key(value), false)));
        GeneralHotInstructionV3 {
            instruction: Instruction {
                program_id: key(200),
                accounts,
                data: vec![7; data_bytes],
            },
            action: Action::Collect,
            outcome_count,
            observation: observation(),
            required_instruction_signers: vec![actor],
            checked_manifest_digest: [8; 32],
            trading_artifact_release: [9; 32],
            general_artifact_release: [10; 32],
            artifacts: GeneralHotArtifactDigestsV3 {
                program_set: [11; 32],
                descriptor: [12; 32],
                config: [13; 32],
                account_profile: [14; 32],
                lifecycle_policy: [15; 32],
                request_profile: [16; 32],
                strategy: [17; 32],
                certificate: [18; 32],
                admission: [19; 32],
                transition: [20; 32],
                effect: [21; 32],
            },
            product_record: [22; 32],
            family_request_digest: [23; 32],
            lifecycle: GeneralLifecycleProjectionV3 {
                primary_state: key(203),
                primary_state_bump: 7,
                terminal_state: None,
                terminal_state_bump: None,
                terminal_coordinate: None,
                child_account_start: 8,
            },
        }
    }

    /// Exact General Hot frame geometry, derived only from semantic owners.
    ///
    /// No count here is copied from a campaign table. The fixed frame is
    /// `HOT_FIXED_ACCOUNT_COUNT_V3` with exactly one writable account (the
    /// composite root, per `validate_fixed_frame`); the strategy extras are
    /// this module's own admitted-AOT constants plus the transport
    /// `classify_bank_transport_v2` selects for General's own bank width; and
    /// every runtime account, alias and privilege comes from General's own
    /// `general_account_profile_rule_v3`.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct GeneralFrameGeometryV3 {
        accounts: usize,
        writable: usize,
        signers: usize,
        scratch_pages: usize,
        physical_runtime: usize,
    }

    /// External account widths for the rule generator.
    ///
    /// The generator asserts these are nonzero and copies them into
    /// `data_length`. A data width moves no account count, no privilege and no
    /// alias, so it cannot move a packet size; these are plausible chain widths
    /// and the geometry below is invariant under any other admissible set.
    fn packet_neutral_widths() -> GeneralExternalAccountWidthsV3 {
        GeneralExternalAccountWidthsV3 {
            linked_basis_prefix: 64,
            result_domain: 96,
            rent_sysvar: 17,
            core_market: 512,
            activation_cache: 256,
            upgradeable_program: 36,
            trading_programdata_prefix: 45,
            claims_programdata_prefix: 45,
            core_programdata_prefix: 45,
            realm_record: 256,
            rent_credit: 64,
        }
    }

    /// `AccountPrivilegesV2` is write-only in the encoder, so read it back by
    /// the one comparison the type does support.
    fn privilege_pair(rule: AccountRuleWithPrestateInputV2) -> (bool, bool) {
        [false, true]
            .into_iter()
            .flat_map(|signer| {
                [false, true].into_iter().flat_map(move |writable| {
                    [false, true]
                        .into_iter()
                        .map(move |executable| (signer, writable, executable))
                })
            })
            .find(|(signer, writable, executable)| {
                rule.rule.privileges == AccountPrivilegesV2::new(*signer, *writable, *executable)
            })
            .map(|(signer, writable, _)| (signer, writable))
            .expect("privilege tuple is one of eight")
    }

    fn general_scratch_pages_v3(outcome_count: u32) -> usize {
        let scalars = general_hot_scalar_count_v3(outcome_count).expect("General scalar count");
        match classify_bank_transport_v2(scalars, GENERAL_HOT_COMMON_IDENTITIES_V3)
            .expect("bank transport")
        {
            BankTransportV2::AuthenticatedScratchPages { page_count, .. } => {
                usize::try_from(page_count).expect("bounded page count")
            }
            BankTransportV2::InlineReturnData { .. } => 1,
        }
    }

    fn general_frame_geometry_v3(action: Action, outcome_count: u32) -> GeneralFrameGeometryV3 {
        let widths = packet_neutral_widths();
        let pages = general_scratch_pages_v3(outcome_count);
        let logical =
            general_account_profile_fixed_count_v3(action).expect("General logical account count");
        let (mut physical_runtime, mut writable, mut signers) = (0_usize, 1_usize, 0_usize);
        for coordinate in 0..logical {
            let rule =
                general_account_profile_rule_v3(action, coordinate, widths).expect("General rule");
            if matches!(rule.rule.alias, AccountAliasInputV2::Fixed(_)) {
                // An alias is a second logical name for a physical account the
                // frame already carries; it costs no transaction account.
                continue;
            }
            physical_runtime += 1;
            if usize::from(coordinate) < HOT_RUNTIME_LOGICAL_PREFIX_V3 {
                // Root, config, Product, portfolio and linked basis are carried
                // by the fixed frame and already counted there.
                continue;
            }
            let (signer, is_writable) = privilege_pair(rule);
            writable += usize::from(is_writable);
            signers += usize::from(signer);
        }
        let (page_signer, page_writable) = privilege_pair(general_scratch_page_rule_v3());
        physical_runtime += pages;
        writable += pages * usize::from(page_writable);
        signers += pages * usize::from(page_signer);
        GeneralFrameGeometryV3 {
            accounts: HOT_FIXED_ACCOUNT_COUNT_V3
                + ADMITTED_AOT_FIXED_EXTRAS_V3
                + pages
                + (physical_runtime - HOT_RUNTIME_LOGICAL_PREFIX_V3),
            writable,
            signers,
            scratch_pages: pages,
            physical_runtime,
        }
    }

    /// One General Hot instruction with the exact geometry of a real action.
    ///
    /// The keys are synthetic and the artifact digests are placeholders,
    /// because neither can move a wire size: a packet is a function of the
    /// account count, the static/looked-up split, the signer count and the
    /// instruction data width, and every one of those is derived above.
    fn real_frame_report(action: Action, outcome_count: u32) -> GeneralHotInstructionV3 {
        let geometry = general_frame_geometry_v3(action, outcome_count);
        let program_id = key(200);
        let mut accounts = Vec::with_capacity(geometry.accounts);
        let mut next = 0_u32;
        let mut fresh = || {
            next += 1;
            let mut bytes = [0_u8; 32];
            bytes[0] = 0x40;
            bytes[1..5].copy_from_slice(&next.to_le_bytes());
            Pubkey::new_from_array(bytes)
        };
        for index in 0..HOT_FIXED_ACCOUNT_COUNT_V3 {
            let pubkey = if index == HOT_TRADING_PROGRAM_ACCOUNT_V3 {
                program_id
            } else {
                fresh()
            };
            accounts.push(AccountMeta {
                pubkey,
                is_signer: false,
                is_writable: index == HOT_ROOT_ACCOUNT_V3,
            });
        }
        let strategy = ADMITTED_AOT_FIXED_EXTRAS_V3 + geometry.scratch_pages;
        for _ in 0..strategy {
            accounts.push(AccountMeta::new_readonly(fresh(), false));
        }
        let mut signers = geometry.signers;
        let mut writable = geometry.writable - 1;
        for _ in 0..(geometry.physical_runtime - HOT_RUNTIME_LOGICAL_PREFIX_V3) {
            let is_signer = signers > 0;
            signers = signers.saturating_sub(1);
            let is_writable = writable > 0;
            writable = writable.saturating_sub(1);
            accounts.push(AccountMeta {
                pubkey: fresh(),
                is_signer,
                is_writable,
            });
        }
        assert_eq!(accounts.len(), geometry.accounts);
        let mut instruction = report(outcome_count, 0);
        instruction.action = action;
        instruction.instruction = Instruction {
            program_id,
            accounts,
            data: vec![0; HOT_FAMILY_REQUEST_OFFSET_V3 + CONTROLLER_REQUEST_BYTES_V2],
        };
        instruction.required_instruction_signers =
            signer_keys(&instruction.instruction.accounts).expect("instruction signers");
        instruction
    }

    fn lookup(report: &GeneralHotInstructionV3, payer: Pubkey) -> ObservedAccount {
        let addresses = canonical_general_lookup_addresses_v3(&report.instruction, payer)
            .expect("canonical addresses");
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(key(201)),
                last_extended_slot: observation().slot - 1,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(addresses),
        };
        ObservedAccount {
            observation: observation(),
            key: key(202),
            owner: lookup_table_program::id(),
            lamports: 1_000_000,
            executable: false,
            data: table.serialize_for_tests().expect("lookup bytes"),
        }
    }

    fn lifecycle_funding_state() -> GeneralHotStateV3 {
        let observed = |value: u8, owner: Pubkey| ObservedAccount {
            observation: observation(),
            key: key(value),
            owner,
            lamports: 1_000_000,
            executable: false,
            data: Vec::new(),
        };
        GeneralHotStateV3 {
            fixed_accounts: Vec::new(),
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts: vec![
                GeneralObservedAccountMetaV3 {
                    account: observed(30, system_program::ID),
                    is_signer: false,
                    is_writable: true,
                },
                GeneralObservedAccountMetaV3 {
                    account: observed(31, system_program::ID),
                    is_signer: true,
                    is_writable: true,
                },
                GeneralObservedAccountMetaV3 {
                    account: observed(32, key(33)),
                    is_signer: false,
                    is_writable: true,
                },
            ],
            release_set: [1; 32],
            generation: 1,
            minimum_finalized_slot: observation().slot,
            checked_release: None,
        }
    }

    fn selection_policy() -> SelectionPolicyV1 {
        let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
        criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
        criteria[2] = SelectionCriterion::MinimizeCandidateId;
        SelectionPolicyV1 {
            policy_id: [41; 32],
            criterion_count: 3,
            criteria,
        }
    }

    fn general_config(policy: SelectionPolicyV1) -> GeneralConfigV3 {
        GeneralConfigV3::new(GeneralConfigV3Input {
            capacity_profile_id: [31; 32],
            claim_basis_id: [32; 32],
            program_set_id: [33; 32],
            generation: 1,
            price_scale: 1,
            collection_slots: 1,
            selection_slots: 1,
            settlement_slots: 1,
            max_orders_per_candidate: 10,
            max_pages_per_candidate: 10,
            continuation_reward_lamports: 1,
            selection_policy_id: policy.policy_id,
            quote_surplus_beneficiary: [34; 32],
        })
        .expect("General config")
    }

    fn verified_candidate(width: u32) -> Vec<u8> {
        let mut output = vec![0; verified_candidate_len(width).expect("verified width")];
        VerifiedCandidateV2::encode_into(
            VerifiedCandidateHeaderV2 {
                outcome_count: width,
                page_count: 2,
                candidate_coordinate: 7,
                revision: 9,
                candidate_id: [51; 32],
                product_id: [52; 32],
                batch_id: [53; 32],
                filled_lots: 3,
                quote_debit: 3,
                quote_credit: 0,
                price_scale: 1,
            },
            &vec![3; usize::try_from(width).expect("width")],
            &vec![3; usize::try_from(width).expect("width")],
            &mut output,
        )
        .expect("verified candidate");
        output
    }

    fn vacant_consider_state(policy: SelectionPolicyV1, verified: Vec<u8>) -> GeneralHotStateV3 {
        let observed = |value: u8, data: Vec<u8>| ObservedAccount {
            observation: observation(),
            key: key(value),
            owner: system_program::ID,
            lamports: 1_000_000,
            executable: false,
            data,
        };
        GeneralHotStateV3 {
            fixed_accounts: Vec::new(),
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts: vec![
                GeneralObservedAccountMetaV3 {
                    account: observed(61, Vec::new()),
                    is_signer: false,
                    is_writable: true,
                },
                GeneralObservedAccountMetaV3 {
                    account: observed(62, Vec::new()),
                    is_signer: true,
                    is_writable: true,
                },
                GeneralObservedAccountMetaV3 {
                    account: observed(63, Vec::new()),
                    is_signer: false,
                    is_writable: true,
                },
                GeneralObservedAccountMetaV3 {
                    account: observed(64, policy.to_bytes().expect("selection policy").to_vec()),
                    is_signer: false,
                    is_writable: false,
                },
                GeneralObservedAccountMetaV3 {
                    account: observed(65, verified),
                    is_signer: false,
                    is_writable: false,
                },
            ],
            release_set: [1; 32],
            generation: 1,
            minimum_finalized_slot: observation().slot,
            checked_release: None,
        }
    }

    fn local_state(kind: GeneralLocalStateKindV3, body: &[u8], width: u32) -> Vec<u8> {
        let len = general_local_state_len_v3(kind, width).expect("local state width");
        let mut scratch = vec![0; len];
        let mut output = vec![0; len];
        encode_general_local_state_v3_atomic(
            GeneralLocalStateHeaderV3 {
                kind,
                bump: 7,
                rent_principal: 11,
                beneficiary: [12; 32],
            },
            body,
            &mut scratch,
            &mut output,
        )
        .expect("local state");
        output
    }

    fn open_selection(width: u32, policy: SelectionPolicyV1, verified: &[u8]) -> Vec<u8> {
        let mut scratch = [0; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        let mut output = [0; RUNTIME_SELECTION_CURSOR_BYTES_V2];
        consider_verified_candidate_v2(
            policy,
            &[0; RUNTIME_SELECTION_CURSOR_BYTES_V2],
            verified,
            0,
            &mut scratch,
            &mut output,
        )
        .expect("open selection");
        local_state(GeneralLocalStateKindV3::Selection, &output, width)
    }

    fn settlement_cursor(
        width: u32,
        phase: SettlementPhaseV2,
        revision: u64,
        order_count: u32,
        next_order: u32,
        inventory: u64,
        quote_inventory: u64,
    ) -> Vec<u8> {
        let mut body = vec![0; settlement_cursor_len(width).expect("cursor width")];
        SettlementCursorV2::encode_into(
            SettlementCursorHeaderV2 {
                outcome_count: width,
                order_count,
                next_order,
                revision,
                candidate_id: [51; 32],
                quote_inventory,
                complete_set_quantity: 0,
                terminal_coordinate: 0,
                phase,
            },
            &vec![inventory; usize::try_from(width).expect("width")],
            &mut body,
        )
        .expect("settlement cursor");
        local_state(GeneralLocalStateKindV3::Settlement, &body, width)
    }

    fn manifest(width: u32, rows: &[(u32, u32, u32)]) -> Vec<u8> {
        let order_count = u32::try_from(rows.len()).expect("manifest order count");
        let mut output =
            vec![0; settlement_manifest_len_v2(width, order_count).expect("manifest width")];
        output.get_mut(0..8).expect("manifest offset in bounds").copy_from_slice(b"DCGMAN02");
        output.get_mut(8..10).expect("manifest offset in bounds").copy_from_slice(&2_u16.to_le_bytes());
        *output.get_mut(10).expect("manifest offset in bounds") = 11;
        output.get_mut(12..16).expect("manifest offset in bounds").copy_from_slice(&width.to_le_bytes());
        output.get_mut(16..20).expect("manifest offset in bounds").copy_from_slice(&order_count.to_le_bytes());
        output.get_mut(20..24).expect("manifest offset in bounds").copy_from_slice(&7_u32.to_le_bytes());
        output.get_mut(24..32).expect("manifest offset in bounds").copy_from_slice(&2_u64.to_le_bytes());
        output.get_mut(32..64).expect("manifest offset in bounds").copy_from_slice(&[51; 32]);
        let row_bytes =
            dclutch_general_adapter_contract::runtime_manifest::settlement_order_len_v2(width)
                .expect("order width");
        for (ordinal, (order_coordinate, source_page_index, source_execution_index)) in
            rows.iter().copied().enumerate()
        {
            let row = 64 + ordinal * row_bytes;
            output.get_mut(row..row + 8).expect("manifest offset in bounds").copy_from_slice(b"DCGORD02");
            output.get_mut(row + 8..row + 10).expect("manifest offset in bounds").copy_from_slice(&2_u16.to_le_bytes());
            *output.get_mut(row + 10).expect("manifest offset in bounds") = 12;
            output.get_mut(row + 12..row + 16).expect("manifest offset in bounds").copy_from_slice(&width.to_le_bytes());
            output.get_mut(row + 16..row + 20).expect("manifest offset in bounds").copy_from_slice(&order_coordinate.to_le_bytes());
            output.get_mut(row + 20..row + 24).expect("manifest offset in bounds").copy_from_slice(&source_page_index.to_le_bytes());
            output.get_mut(row + 24..row + 32).expect("manifest offset in bounds").copy_from_slice(&9_u64.to_le_bytes());
            output.get_mut(row + 32..row + 64).expect("manifest offset in bounds").copy_from_slice(&[51; 32]);
            let order_byte = u8::try_from(order_coordinate).expect("order identity");
            output.get_mut(row + 64..row + 96).expect("manifest offset in bounds").copy_from_slice(&[order_byte; 32]);
            output.get_mut(row + 96..row + 128).expect("manifest offset in bounds").copy_from_slice(&[72; 32]);
            output.get_mut(row + 128..row + 136).expect("manifest offset in bounds").copy_from_slice(&3_u64.to_le_bytes());
            output.get_mut(row + 136..row + 144).expect("manifest offset in bounds").copy_from_slice(&3_u64.to_le_bytes());
            output.get_mut(row + 152..row + 156).expect("manifest offset in bounds").copy_from_slice(&source_execution_index.to_le_bytes());
            let input_start = row + 160;
            let output_start = input_start + usize::try_from(width).expect("width") * 8;
            for index in 0..usize::try_from(width).expect("width") {
                let offset = index * 8;
                output
                    .get_mut(input_start + offset..input_start + offset + 8)
                    .expect("manifest offset in bounds")
                    .copy_from_slice(&3_u64.to_le_bytes());
                output
                    .get_mut(output_start + offset..output_start + offset + 8)
                    .expect("manifest offset in bounds")
                    .copy_from_slice(&3_u64.to_le_bytes());
            }
        }
        SettlementManifestV2::decode(&output).expect("canonical manifest");
        output
    }

    fn settlement_state(
        action: Action,
        local: Vec<u8>,
        verified: Vec<u8>,
        manifest: Option<Vec<u8>>,
    ) -> GeneralHotStateV3 {
        let observed = |value: u8, data: Vec<u8>| ObservedAccount {
            observation: observation(),
            key: key(value),
            owner: key(200),
            lamports: 1_000_000,
            executable: false,
            data,
        };
        let mut runtime_suffix_accounts = vec![GeneralObservedAccountMetaV3 {
            account: observed(81, local),
            is_signer: false,
            is_writable: true,
        }];
        let lifecycle_prefix = if action == Action::Close { 4 } else { 3 };
        while runtime_suffix_accounts.len() < lifecycle_prefix {
            let ordinal = u8::try_from(runtime_suffix_accounts.len()).expect("ordinal");
            let coordinate = 5_u16 + u16::from(ordinal);
            let payer = if action == Action::Close {
                GENERAL_CLOSE_PAYER_ACCOUNT_V3
            } else {
                GENERAL_PRIMARY_PAYER_ACCOUNT_V3
            };
            runtime_suffix_accounts.push(GeneralObservedAccountMetaV3 {
                account: observed(82 + ordinal, Vec::new()),
                is_signer: coordinate == payer,
                is_writable: true,
            });
        }
        runtime_suffix_accounts.push(GeneralObservedAccountMetaV3 {
            account: observed(91, verified),
            is_signer: false,
            is_writable: false,
        });
        if let Some(bytes) = manifest {
            runtime_suffix_accounts.push(GeneralObservedAccountMetaV3 {
                account: observed(92, bytes),
                is_signer: false,
                is_writable: false,
            });
        }
        GeneralHotStateV3 {
            fixed_accounts: Vec::new(),
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts,
            release_set: [1; 32],
            generation: 1,
            minimum_finalized_slot: observation().slot,
            checked_release: None,
        }
    }

    #[test]
    fn canonical_lut_compiles_packet_and_reports_payer_then_actor() {
        for outcome_count in [1_u32, 258] {
            let report = report(outcome_count, 192);
            let payer = key(250);
            let lookup = lookup(&report, payer);
            let plan =
                compile_general_hot_v0(&report, payer, Hash::new_from_array([16; 32]), &lookup)
                    .expect("packet-safe General action");
            assert_eq!(plan.required_signers, vec![payer, key(1)]);
            assert_eq!(plan.message.required_signatures, 2);
            assert!(plan.message.loaded_addresses >= 90);
            assert!(plan.message.wire_bytes <= crate::versioned::PACKET_DATA_BYTES);
            assert_eq!(plan.outcome_count, outcome_count);
            assert_eq!(plan.lifecycle, report.lifecycle);
        }
    }

    /// The packet witness the General campaign's own comment owes.
    ///
    /// `programs/dclutch-general-accelerator-sbf/program-test/tests/lifecycle.rs`
    /// measures six of seven N=258 actions at 1,273-1,328 legacy bytes against
    /// the 1,232-byte maximum, and excuses it with "the production operator
    /// separately proves the same account set packet-safe through its exact
    /// ALT-backed v0 plan". That plan is this one, and until now nothing ran a
    /// real General account set through it: the fixture above fabricates
    /// ninety-one metas and carries `outcome_count` as a label that moves no
    /// geometry.
    ///
    /// This runs the real thing. Every account count, privilege and alias comes
    /// from General's own `general_account_profile_rule_v3` and the transport
    /// `classify_bank_transport_v2` selects; the instruction data is the exact
    /// hot envelope plus the exact 64-byte controller request. The recorded
    /// numbers are the wire this operator would actually submit.
    #[test]
    fn every_action_is_alt_packet_safe_at_the_canonical_runtime_width() {
        let payer = key(250);
        let blockhash = Hash::new_from_array([16; 32]);
        for (action, accounts, wire) in [
            (Action::Consider, 86, 664),
            (Action::Freeze, 84, 660),
            (Action::InitializeSettlement, 118, 918),
            (Action::Collect, 113, 813),
            (Action::Materialize, 111, 809),
            (Action::Distribute, 113, 813),
            (Action::Close, 112, 811),
        ] {
            let report = real_frame_report(action, 258);
            assert_eq!(report.instruction.accounts.len(), accounts, "{action:?}");
            let plan = compile_general_hot_v0(&report, payer, blockhash, &lookup(&report, payer))
                .expect("packet-safe General action at the canonical width");
            assert_eq!(plan.message.wire_bytes, wire, "{action:?}");
            assert!(plan.message.wire_bytes <= crate::versioned::PACKET_DATA_BYTES);
            assert!(plan.message.loaded_addresses > 0);
        }
    }

    /// The lookup table is what buys the margin, and the legacy wire is what
    /// the campaign measured. Compiling the same account set with no table
    /// refuses, so the ALT is load-bearing rather than decorative.
    #[test]
    fn the_same_account_set_without_a_table_is_not_packet_safe() {
        let payer = key(250);
        let report = real_frame_report(Action::InitializeSettlement, 258);
        let inline = crate::versioned::compile_v0_message_with_optional_tables(
            payer,
            core::slice::from_ref(&report.instruction),
            Hash::new_from_array([16; 32]),
            observation(),
            &[],
        );
        assert_eq!(inline, Err(crate::versioned::Error::PacketTooLarge));
    }

    /// The control: this derivation reproduces the real-ELF campaign exactly.
    ///
    /// `docs/evidence/GENERAL_ACCELERATOR_CAMPAIGN_2026_08_27.md` recorded the
    /// instruction-account count of every N=258 action executed against the
    /// real `dclutch_general_accelerator_sbf.so`. That frame is two harness
    /// accounts (the request record and the accelerator program), then the
    /// admitted fixed frame, then one account per *logical* profile
    /// coordinate -- the accelerator reads an aliased coordinate as its own
    /// readonly account, where the Trading Hot frame carries the physical
    /// account once. Seven independent numbers, none of them derived from this
    /// crate, and the profile generator reproduces all seven.
    #[test]
    fn the_derived_geometry_reproduces_the_executed_campaign_frame() {
        for (action, campaign_accounts) in [
            (Action::Consider, 47),
            (Action::Freeze, 45),
            (Action::InitializeSettlement, 102),
            (Action::Collect, 83),
            (Action::Materialize, 81),
            (Action::Distribute, 83),
            (Action::Close, 100),
        ] {
            let logical =
                usize::from(general_account_profile_fixed_count_v3(action).expect("logical count"))
                    + general_scratch_pages_v3(258);
            assert_eq!(
                2 + ADMITTED_RUNTIME_ACCOUNTS_START_V3 + logical,
                campaign_accounts,
                "{action:?}"
            );
        }
        // Consider and Freeze alias nothing, so for those two the campaign
        // frame and the Hot frame agree on the physical count as well.
        for action in [Action::Consider, Action::Freeze] {
            let geometry = general_frame_geometry_v3(action, 258);
            assert_eq!(
                usize::from(general_account_profile_fixed_count_v3(action).expect("count"))
                    + geometry.scratch_pages,
                geometry.physical_runtime,
                "{action:?}"
            );
        }
    }

    /// N=1 and N=258 differ only by the scratch-page span, and both fit.
    #[test]
    fn the_runtime_width_moves_only_the_scratch_page_span() {
        for action in GENERAL_ACTIONS_V3 {
            let narrow = general_frame_geometry_v3(action, 1);
            let wide = general_frame_geometry_v3(action, 258);
            assert_eq!(
                wide.accounts - narrow.accounts,
                2 * (wide.scratch_pages - narrow.scratch_pages),
                "{action:?} account width follows only the bank transport"
            );
            assert_eq!(narrow.signers, wide.signers);
            assert_eq!(narrow.writable, wide.writable);
        }
    }

    #[test]
    fn stale_or_noncanonical_lookup_and_oversized_packet_refuse() {
        let payer = key(250);
        let canonical = report(258, 192);
        let mut stale = lookup(&canonical, payer);
        stale.observation.slot += 1;
        assert_eq!(
            compile_general_hot_v0(&canonical, payer, Hash::new_from_array([16; 32]), &stale,),
            Err(GeneralHotOperatorErrorV3::Snapshot)
        );

        let mut extra = lookup(&canonical, payer);
        let decoded = AddressLookupTable::deserialize(&extra.data).expect("table");
        let mut addresses = decoded.addresses.into_owned();
        addresses.push(key(249));
        addresses.sort_unstable_by_key(Pubkey::to_bytes);
        let table = AddressLookupTable {
            meta: decoded.meta,
            addresses: Cow::Owned(addresses),
        };
        extra.data = table.serialize_for_tests().expect("extra table");
        assert_eq!(
            compile_general_hot_v0(&canonical, payer, Hash::new_from_array([16; 32]), &extra,),
            Err(GeneralHotOperatorErrorV3::LookupTable)
        );

        let oversized = report(258, 2_000);
        let lookup = lookup(&oversized, payer);
        assert_eq!(
            compile_general_hot_v0(&oversized, payer, Hash::new_from_array([16; 32]), &lookup,),
            Err(GeneralHotOperatorErrorV3::Routing(
                crate::versioned::Error::PacketTooLarge
            ))
        );
    }

    #[test]
    fn consider_request_is_chain_derived_for_runtime_widths_and_substitution_refuses() {
        let policy = selection_policy();
        let config = general_config(policy);
        for outcome_count in [1_u32, 258] {
            let state = vacant_consider_state(policy, verified_candidate(outcome_count));
            let request = derive_consider_request_v5(&state, config, outcome_count, [52; 32])
                .expect("chain-derived Consider request");
            assert_eq!(request.action, Action::Consider);
            assert_eq!(request.expected_revision, 0);
            assert_eq!(request.candidate_id, Some([51; 32]));
            assert_eq!(request.page_index, 7);
            assert_eq!(request.execution_index, 0);
            assert_eq!(request.state_bump, 0);
        }

        let substituted_config = general_config(SelectionPolicyV1 {
            policy_id: [42; 32],
            ..policy
        });
        let state = vacant_consider_state(policy, verified_candidate(1));
        assert_eq!(
            derive_consider_request_v5(&state, substituted_config, 1, [52; 32]),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );
    }

    #[test]
    fn freeze_and_settlement_requests_derive_exact_chain_progress() {
        let policy = selection_policy();
        let config = general_config(policy);
        for width in [1_u32, 258] {
            let verified = verified_candidate(width);
            let selection = open_selection(width, policy, &verified);
            let freeze_state = GeneralHotStateV3 {
                fixed_accounts: Vec::new(),
                strategy_accounts: Vec::new(),
                runtime_suffix_accounts: vec![GeneralObservedAccountMetaV3 {
                    account: ObservedAccount {
                        observation: observation(),
                        key: key(81),
                        owner: key(200),
                        lamports: 1_000_000,
                        executable: false,
                        data: selection,
                    },
                    is_signer: false,
                    is_writable: true,
                }],
                release_set: [1; 32],
                generation: 1,
                minimum_finalized_slot: observation().slot,
                checked_release: None,
            };
            let freeze = derive_freeze_request_v5(&freeze_state, config, width, [52; 32])
                .expect("derived Freeze");
            assert_eq!(freeze.action, Action::Freeze);
            assert_eq!(freeze.expected_revision, 1);
            assert_eq!(freeze.candidate_id, None);

            for (action, phase, inventory, quote_inventory) in [
                (Action::Collect, SettlementPhaseV2::Collecting, 0, 0),
                (Action::Materialize, SettlementPhaseV2::Materializing, 3, 3),
                (Action::Distribute, SettlementPhaseV2::Distributing, 3, 3),
                (Action::Close, SettlementPhaseV2::ReadyToClose, 0, 3),
            ] {
                let row_action = matches!(action, Action::Collect | Action::Distribute);
                let next_order = 1;
                let state = settlement_state(
                    action,
                    settlement_cursor(
                        width,
                        phase,
                        5,
                        if row_action { 2 } else { 1 },
                        next_order,
                        inventory,
                        quote_inventory,
                    ),
                    verified.clone(),
                    row_action.then(|| manifest(width, &[(1, 0, 0), (2, 2, 0)])),
                );
                let request = derive_settlement_request_v5(&state, config, action, width, [52; 32])
                    .expect("derived settlement action");
                assert_eq!(request.action, action);
                assert_eq!(request.expected_revision, 5);
                assert_eq!(request.candidate_id, Some([51; 32]));
                if matches!(action, Action::Collect | Action::Distribute) {
                    assert_eq!(request.page_index, 2);
                    assert_eq!(request.execution_index, 0);
                    assert_eq!(request.manifest_order_index, 1);
                } else {
                    assert_eq!(request.page_index, 0);
                    assert_eq!(request.execution_index, 0);
                }
            }
        }
    }

    #[test]
    fn settlement_manifest_order_and_candidate_substitution_refuse() {
        let policy = selection_policy();
        let config = general_config(policy);
        let width = 1;
        let verified = verified_candidate(width);
        let cursor = settlement_cursor(width, SettlementPhaseV2::Collecting, 5, 1, 0, 0, 0);
        let out_of_order = settlement_state(
            Action::Collect,
            cursor.clone(),
            verified.clone(),
            Some(manifest(width, &[(2, 2, 0)])),
        );
        assert_eq!(
            derive_settlement_request_v5(&out_of_order, config, Action::Collect, width, [52; 32],),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );

        let mut substituted = verified;
        *substituted.get_mut(32).expect("verified candidate byte exists") ^= 1;
        let candidate_substitution = settlement_state(
            Action::Collect,
            cursor,
            substituted,
            Some(manifest(width, &[(1, 0, 0)])),
        );
        assert_eq!(
            derive_settlement_request_v5(
                &candidate_substitution,
                config,
                Action::Collect,
                width,
                [52; 32],
            ),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );
    }

    #[test]
    fn lifecycle_funding_requires_exact_signer_privileges_and_no_alias() {
        let state = lifecycle_funding_state();
        assert_eq!(
            validate_lifecycle_funding_accounts(&state, 5, Some(6), Some(7)),
            Ok(())
        );

        let mut unsigned = state.clone();
        unsigned
            .runtime_suffix_accounts
            .get_mut(1)
            .expect("payer")
            .is_signer = false;
        assert_eq!(
            validate_lifecycle_funding_accounts(&unsigned, 5, Some(6), Some(7)),
            Err(GeneralHotOperatorErrorV3::Lifecycle)
        );

        let mut signer_credit = state.clone();
        signer_credit
            .runtime_suffix_accounts
            .get_mut(2)
            .expect("RentCredit")
            .is_signer = true;
        assert_eq!(
            validate_lifecycle_funding_accounts(&signer_credit, 5, Some(6), Some(7)),
            Err(GeneralHotOperatorErrorV3::Lifecycle)
        );

        let mut alias = state;
        let state_key = alias
            .runtime_suffix_accounts
            .first()
            .expect("state")
            .account
            .key;
        alias
            .runtime_suffix_accounts
            .get_mut(2)
            .expect("RentCredit")
            .account
            .key = state_key;
        assert_eq!(
            validate_lifecycle_funding_accounts(&alias, 5, Some(6), Some(7)),
            Err(GeneralHotOperatorErrorV3::Lifecycle)
        );
    }

    #[test]
    fn terminal_coordinate_is_revision_successor_and_overflow_refuses() {
        assert_eq!(
            canonical_terminal_coordinate_v3(Action::Close, 41),
            Ok(Some(42))
        );
        assert_eq!(
            canonical_terminal_coordinate_v3(Action::Freeze, u64::MAX),
            Ok(None)
        );
        assert_eq!(
            canonical_terminal_coordinate_v3(Action::Close, u64::MAX),
            Err(GeneralHotOperatorErrorV3::Arithmetic)
        );
    }

    #[test]
    fn stale_v4_lifecycle_bytes_cannot_enter_the_operator() {
        let width = general_state_lifecycle_bytes_v3(Action::InitializeSettlement)
            .expect("legacy V4 width");
        let mut scratch = vec![0_u8; width];
        let mut legacy = vec![0_u8; width];
        encode_general_state_lifecycle_v3_atomic(
            Action::InitializeSettlement,
            &mut scratch,
            &mut legacy,
        )
        .expect("canonical legacy evidence");
        assert_eq!(
            StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &legacy),
            Err(LifecycleErrorV3::UnsupportedProfile)
        );
    }
}
