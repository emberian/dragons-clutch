//! Chain-derived General V3 Hot execution and packet construction.
//!
//! The operator never owns an action-specific account list. It authenticates
//! the selected General artifacts, expands the exact selected AccountProfile,
//! derives Product width from the finalized Product graph, and then compiles
//! the exact leading heap-frame declaration plus Trading Hot into one unsigned
//! v0 message through one exact canonical lookup table. It performs no RPC,
//! signing, submission, or account mutation.

use dclutch_account_profile_contract::lifecycle_v3::{
    CoordinateScopeV3, LifecycleOperationV3, LifecycleRegisterKindV3, LifecycleRegistersV3,
    LifecycleSeedInputValueV3, SelectedLifecycleV3,
};
use dclutch_account_profile_contract::v2::{
    DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE, PhysicalAccountDataGeometryV2,
};
use dclutch_capability_program_contract::hot_v3::{
    DIRECT_HOT_HEAP_FRAME_BYTES_V1, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
    HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
    HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3,
    HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
    HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
    HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
    HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
    HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
    HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3, HotBumpHintsV1,
    HotExecutionEnvelopeV3,
};
use dclutch_capability_program_contract::v4::{CapabilityProgramV4, CapabilityRootAccountV4};
use dclutch_effect_kernel::v2::FixedRole;
use dclutch_execution_strategy_contract::admitted_v3::{
    ADMITTED_ACCELERATOR_PROGRAM_ACCOUNT_V3, ADMITTED_ADMISSION_RAW_ACCOUNT_V3,
    ADMITTED_CERTIFICATE_RAW_ACCOUNT_V3, ADMITTED_STRATEGY_EVIDENCE_COUNT_V3,
    ADMITTED_STRATEGY_EVIDENCE_START_V3,
};
use dclutch_execution_strategy_contract::v2::{BankTransportV2, classify_bank_transport_v2};
use dclutch_general_adapter_contract::artifacts_v3::{
    GeneralArtifactBytesV3, GeneralArtifactSelectionV3, GeneralDecodedRequestV3,
    GeneralRequestWireV3, authenticate_general_artifacts_v3, decode_general_request_v3,
};
use dclutch_general_adapter_contract::{
    admitted_accelerator_v3::authenticate_frozen_selection_v3,
    candidate_v1::{
        CandidateVerifyRowBuffersV1, CandidateVerifyRowViewV1, GeneralCandidateV1,
        authenticate_candidate_identity_v1, candidate_certificate_len_v1,
        candidate_verifier_len_v1, candidate_verify_manifest_orders_v1, verify_candidate_row_v1,
    },
    collection_v1::{
        BatchStatusV1, GeneralBatchOccurrenceTermsV1, GeneralBatchOpeningV1, GeneralBatchV1,
        GeneralOrderPhaseV1, GeneralOrderV1, GeneralSignedOrderTermsV1,
    },
    effect_artifacts_v3::{
        GeneralChildFrameV3, general_effect_route_count_v3, general_effect_route_frame_v3,
    },
    hot_candidate_v3::{identity as general_identity, scalar as general_scalar},
    local_state_v3::{GeneralLocalStateKindV3, GeneralLocalStateV3},
    runtime_manifest::{SettlementManifestV2, settlement_manifest_len_v2},
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
    runtime_width::{CandidateV2, SettlementCursorV2, VerifiedCandidateV2, settlement_cursor_len},
    state_artifacts_v3::{
        GENERAL_CLOSE_PAYER_ACCOUNT_V3, GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3,
        GENERAL_FAMILY_QUOTE_COUNT_V5, GENERAL_PRIMARY_PAYER_ACCOUNT_V3,
        GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3, GENERAL_PRIMARY_STATE_ACCOUNT_V3,
        GENERAL_TERMINAL_STATE_ACCOUNT_V3, GENERAL_VERIFY_PAYER_ACCOUNT_V3,
        GENERAL_VERIFY_RENT_CREDIT_ACCOUNT_V3, GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3,
        GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3, GeneralChildRentWidthsV5,
        GeneralReadonlyEvidenceKindV3, encode_general_family_state_lifecycle_v5_atomic,
        general_child_account_start_v3, general_family_state_lifecycle_bytes_v5,
        general_readonly_evidence_count_v3, general_readonly_evidence_start_v3,
        general_readonly_evidence_v3,
    },
    state_seeds_v3::GeneralStateRecipeV3,
};
use dclutch_general_codec::{
    Action, SelectionPolicyV1,
    successor_request_v2::{CONTROLLER_REQUEST_BYTES_V2, ControllerRequestV2},
};
use dclutch_general_config_contract::{
    root::{GeneralLifecycleV2, GeneralRootV2},
    v3::GeneralConfigV3,
};
use dclutch_hot_bump_miner_v1::{
    HotBumpCorpusV1, activated_custody_program_v1, mine_hot_bump_hints_v1,
};
use solana_address_lookup_table_interface::{
    program as lookup_table_program, state::AddressLookupTable,
};
use solana_compute_budget_interface::ComputeBudgetInstruction;
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
// The admitted CPI frame's evidence suffix is owned by
// `dclutch_execution_strategy_contract::admitted_v3`, which derives every slot
// from `ADMITTED_STRATEGY_EVIDENCE_START_V3` and pins the span's length to its
// last named account. The coordinates below are that table read relative to the
// start of the suffix, because `strategy_accounts` is the suffix, not the whole
// frame -- so they are subtracted from the contract's absolute coordinates
// rather than restated as the numbers they currently evaluate to.
const ADMITTED_AOT_FIXED_EXTRAS_V3: usize = ADMITTED_STRATEGY_EVIDENCE_COUNT_V3;
const ADMITTED_CERTIFICATE_RAW_EXTRA_V3: usize =
    ADMITTED_CERTIFICATE_RAW_ACCOUNT_V3 - ADMITTED_STRATEGY_EVIDENCE_START_V3;
const ADMITTED_ADMISSION_RAW_EXTRA_V3: usize =
    ADMITTED_ADMISSION_RAW_ACCOUNT_V3 - ADMITTED_STRATEGY_EVIDENCE_START_V3;
const ADMITTED_ACCELERATOR_PROGRAM_EXTRA_V3: usize =
    ADMITTED_ACCELERATOR_PROGRAM_ACCOUNT_V3 - ADMITTED_STRATEGY_EVIDENCE_START_V3;

/// Measured SBF heap frame required by every General successor transaction.
///
/// Candidate verification at the accepted N=258 profile carries the complete
/// Hot bank plus state-last verifier, certificate, and manifest candidates.
/// The default 32,768-byte frame refuses that real-ELF path; 65,536 bytes is
/// the existing protocol-wide Hot execution frame and is emitted explicitly
/// before Trading rather than assumed by the accelerator.
pub const GENERAL_HOT_HEAP_FRAME_BYTES_V3: u32 = DIRECT_HOT_HEAP_FRAME_BYTES_V1;

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

/// Mine the bumps this family's readers would otherwise search for on chain.
///
/// The DERIVATION is `dclutch_hot_bump_miner_v1`'s, shared with the Direct
/// builder, the Dealer LP builder, the Rational public outer builders and the
/// campaign's bundle builder. This function owns only the CORPUS -- which
/// coordinate of the General Hot frame is the Market, which is the root, and which account
/// names the Custody deployment.
///
/// Every hint is reproduced by the reader with `create_program_address` against
/// the account the frame supplied, so a wrong byte names a different address
/// and refuses at an equality that was already there. No conjunct moves.
///
/// # Which slots this corpus reaches, and which it deliberately leaves
///
/// `market`, `root` and Custody's transfer authority are derivable from the
/// frame this builder already authenticated. `child_relay[0]` is Custody's
/// replay cursor, whose seeds end in the projected child request's replay
/// context; `child_caller`'s seeds end in a digest over a request projected ON
/// chain; `lifecycle` is this family's created accounts in materialization
/// order. None of the three is projected here, so all three stay zero and
/// search, which is correct and merely slower.
fn general_hot_bump_hints_v3(
    state: &GeneralHotStateV3,
    trading_program: &Pubkey,
) -> Result<HotBumpHintsV1, GeneralHotOperatorErrorV3> {
    let fixed = |coordinate: usize| {
        state
            .fixed_accounts
            .get(coordinate)
            .ok_or(GeneralHotOperatorErrorV3::FixedFrame)
    };
    let market = &fixed(HOT_MARKET_ACCOUNT_V3)?.account;
    // Custody is not in the hot fixed frame; the Market's activation cache is,
    // and it names the release set's Custody deployment.
    let activation = &fixed(HOT_ACTIVATION_CACHE_ACCOUNT_V3)?.account;
    Ok(mine_hot_bump_hints_v1(&HotBumpCorpusV1 {
        market_key: market.key,
        market_data: &market.data,
        root_data: &fixed(HOT_ROOT_ACCOUNT_V3)?.account.data,
        core_program: fixed(HOT_CORE_PROGRAM_ACCOUNT_V3)?.account.key,
        trading_program: *trading_program,
        custody_program: activated_custody_program_v1(&activation.data),
        release_set: state.release_set,
    }))
}

/// Borrow the exact General artifact carriers from one canonical Hot frame.
///
/// This is the sole public mapping from physical Hot/admitted-AOT account
/// positions to [`GeneralArtifactBytesV3`]. It deliberately returns borrowed
/// account data rather than copying semantic bytes into a second document.
/// Callers still pass the result through the action-selected artifact join;
/// this helper owns only which canonical raw carrier supplies each field.
pub fn general_artifact_bytes_from_hot_state_v3(
    state: &GeneralHotStateV3,
) -> Result<GeneralArtifactBytesV3<'_>, GeneralHotOperatorErrorV3> {
    if state.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3 {
        return Err(GeneralHotOperatorErrorV3::FixedFrame);
    }
    if state.strategy_accounts.len() < ADMITTED_AOT_FIXED_EXTRAS_V3 {
        return Err(GeneralHotOperatorErrorV3::StrategyGeometry);
    }
    let fixed = |coordinate: usize| {
        state
            .fixed_accounts
            .get(coordinate)
            .map(|value| value.account.data.as_slice())
            .ok_or(GeneralHotOperatorErrorV3::FixedFrame)
    };
    let strategy = |coordinate: usize| {
        state
            .strategy_accounts
            .get(coordinate)
            .map(|value| value.account.data.as_slice())
            .ok_or(GeneralHotOperatorErrorV3::StrategyGeometry)
    };
    Ok(GeneralArtifactBytesV3 {
        program_set: fixed(HOT_PROGRAM_SET_RAW_ACCOUNT_V3)?,
        descriptor: fixed(HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?,
        config: fixed(HOT_CONFIG_RAW_ACCOUNT_V3)?,
        account_profile: fixed(HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)?,
        lifecycle_policy: fixed(HOT_LIFECYCLE_RAW_ACCOUNT_V3)?,
        request_profile: fixed(HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3)?,
        strategy: fixed(HOT_STRATEGY_RAW_ACCOUNT_V3)?,
        certificate: strategy(ADMITTED_CERTIFICATE_RAW_EXTRA_V3)?,
        admission: strategy(ADMITTED_ADMISSION_RAW_EXTRA_V3)?,
        transition: fixed(HOT_TRANSITION_RAW_ACCOUNT_V3)?,
        effect: fixed(HOT_EFFECT_RAW_ACCOUNT_V3)?,
    })
}

/// One canonical action-state address derived from the authenticated lifecycle policy.
///
/// This remains an operator projection, never authority. Trading derives the
/// same address and bump again before it creates, authenticates, or closes the
/// account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralLifecycleStateProjectionV3 {
    /// Exact logical AccountProfile coordinate occupied by this state.
    pub account_coordinate: u16,
    /// Canonical program-derived state address.
    pub account: Pubkey,
    /// Canonical PDA bump written into the generation-aware request witness.
    pub bump: u8,
    /// Whether the same finalized snapshot observed the state as materialized.
    /// `false` means the lifecycle policy authenticated a vacant System account
    /// at the same canonical address; it never means the coordinate is absent.
    pub is_materialized: bool,
}

/// Canonical action-state topology derived from the authenticated lifecycle policy.
///
/// Verify names all three of Candidate, Verifier, and conditional Result even
/// before the Result is created. The optional fields describe whether the
/// action selects those coordinates, not whether an observed account happens
/// to be vacant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralLifecycleProjectionV3 {
    /// Canonical primary action state.
    pub primary: GeneralLifecycleStateProjectionV3,
    /// Canonical secondary state for multi-state actions.
    pub secondary: Option<GeneralLifecycleStateProjectionV3>,
    /// Canonical conditional result state selected by VerifyCandidateRow.
    pub conditional_result: Option<GeneralLifecycleStateProjectionV3>,
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
    /// Exact leading ComputeBudget heap-frame declaration.
    pub heap_frame_bytes: u32,
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
    /// Exact heap frame the transaction compiler must declare before Trading.
    pub heap_frame_bytes: u32,
    /// Canonical chain-derived controller request, including PDA bump witnesses.
    pub request: GeneralDecodedRequestV3,
    /// Product-derived outcome width.
    pub outcome_count: u32,
    /// Accelerator invocations the selected bank geometry costs.
    ///
    /// This was named for the input scratch pages because their count was the
    /// same number: both came from `classify_bank_transport_v2`, which answers
    /// the RETURN-DATA question. The input bank is inline now and pages none of
    /// itself; what remains is the output invocation count, which is also the
    /// caller-authority span length, and that is what this always computed.
    pub admitted_invocation_count: u32,
    /// Exact action-specific DCE5 child route and receipt order.
    pub child_routes: Vec<GeneralChildRouteV5>,
}

/// Stable packet-safe unsigned General successor plan for frontends.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralSuccessorTransactionPlanV0 {
    /// Exact packet-safe v0 message and signer report.
    pub hot: GeneralHotTransactionPlanV3,
    /// Exact leading ComputeBudget heap-frame declaration.
    pub heap_frame_bytes: u32,
    /// Canonical chain-derived controller request.
    pub request: GeneralDecodedRequestV3,
    /// Accelerator invocations the selected bank geometry costs.
    pub admitted_invocation_count: u32,
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
    let hot = build_general_hot_instruction_decoded_v3(
        state,
        artifact_selection,
        artifact_bytes,
        request,
    )?;
    let request = decode_general_request_v3(
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
    let admitted_invocation_count = selected_admitted_invocation_count_v3(bundle)?;
    let child_routes = project_child_routes_v5(bundle)?;
    Ok(GeneralSuccessorInstructionV5 {
        hot,
        heap_frame_bytes: GENERAL_HOT_HEAP_FRAME_BYTES_V3,
        request,
        outcome_count: product.outcome_count,
        admitted_invocation_count,
        child_routes,
    })
}

/// Compile the exact heap-frame declaration and successor instruction into an
/// unsigned v0 message.
pub fn compile_general_successor_v0(
    report: &GeneralSuccessorInstructionV5,
    payer: Pubkey,
    recent_blockhash: Hash,
    lookup_table: &ObservedAccount,
) -> Result<GeneralSuccessorTransactionPlanV0, GeneralHotOperatorErrorV3> {
    if report.heap_frame_bytes != GENERAL_HOT_HEAP_FRAME_BYTES_V3 {
        return Err(GeneralHotOperatorErrorV3::StrategyGeometry);
    }
    let hot = compile_general_hot_v0(&report.hot, payer, recent_blockhash, lookup_table)?;
    Ok(GeneralSuccessorTransactionPlanV0 {
        hot,
        heap_frame_bytes: report.heap_frame_bytes,
        request: report.request,
        admitted_invocation_count: report.admitted_invocation_count,
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
    let request_bytes = request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    let request = decode_general_request_v3(&request_bytes)
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    build_general_hot_instruction_decoded_v3(state, artifact_selection, artifact_bytes, request)
}

/// Build one complete chain-derived General Hot instruction from either exact
/// admitted request generation.
///
/// The request has already passed its generation-specific hostile decoder. Its
/// three bump fields remain untrusted witnesses: this constructor replaces
/// them with the canonical lifecycle derivations before authenticating the
/// selected RequestProfile and complete artifact graph.
pub fn build_general_hot_instruction_decoded_v3(
    state: &GeneralHotStateV3,
    artifact_selection: GeneralArtifactSelectionV3,
    artifact_bytes: GeneralArtifactBytesV3<'_>,
    request: GeneralDecodedRequestV3,
) -> Result<GeneralHotInstructionV3, GeneralHotOperatorErrorV3> {
    let checked = state
        .checked_release
        .ok_or(GeneralHotOperatorErrorV3::UnrecognizedRelease)?;
    validate_release(checked, artifact_selection)?;
    let observation = validate_fixed_frame(state, checked)?;
    let product = authenticate_product_graph(state)?;
    let mut canonical_request = GeneralDecodedRequestV3 {
        state_bump: 0,
        terminal_record_bump: 0,
        result_state_bump: 0,
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
    if provisional_bundle
        .request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?
        != provisional_request_bytes
    {
        return Err(GeneralHotOperatorErrorV3::Artifact);
    }
    let provisional_lifecycle = project_general_lifecycle_v5(
        state,
        provisional_bundle,
        canonical_request,
        checked.trading_program,
    )?;
    canonical_request.state_bump = provisional_lifecycle.primary.bump;
    canonical_request.terminal_record_bump = provisional_lifecycle
        .secondary
        .map(|value| value.bump)
        .unwrap_or_default();
    canonical_request.result_state_bump = provisional_lifecycle
        .conditional_result
        .map(|value| value.bump)
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
    if bundle
        .request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?
        != request_bytes
    {
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
    .map_err(|_| GeneralHotOperatorErrorV3::FixedFrame)?
    .with_bump_hints(general_hot_bump_hints_v3(state, &checked.trading_program)?);
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

/// Compile the required heap frame followed by one General instruction into an
/// unsigned packet-safe v0 message.
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
    let heap_frame = ComputeBudgetInstruction::request_heap_frame(GENERAL_HOT_HEAP_FRAME_BYTES_V3);
    let instructions = [heap_frame, report.instruction.clone()];
    let message = compile_v0_message(
        payer,
        &instructions,
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
        heap_frame_bytes: GENERAL_HOT_HEAP_FRAME_BYTES_V3,
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
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    if hash(artifacts.config).to_bytes() != selection.config {
        return Err(GeneralHotOperatorErrorV3::ContentIdentity);
    }
    let config = GeneralConfigV3::decode(artifacts.config)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if config.program_set_id() != selection.program_set {
        return Err(GeneralHotOperatorErrorV3::ContentIdentity);
    }
    match action {
        Action::OpenBatch => derive_open_request_v5(
            state,
            artifacts,
            outcome_count,
            selection.config,
            config,
            product_record,
        ),
        Action::SubmitCandidate => derive_submit_request_v5(
            state,
            artifacts,
            outcome_count,
            selection.config,
            config,
            product_record,
        ),
        Action::VerifyCandidateRow => derive_verify_request_v5(
            state,
            artifacts,
            outcome_count,
            selection.config,
            product_record,
        ),
        Action::PlaceOrder
        | Action::CancelOrder
        | Action::CloseBatch
        | Action::ReleaseOrder
        | Action::CloseCandidate => {
            derive_front_request_v5(state, artifacts, action, outcome_count, selection.config)
        }
        Action::Consider => decoded_v2_request(derive_consider_request_v5(
            state,
            config,
            outcome_count,
            product_record,
        )?),
        Action::Freeze => decoded_v2_request(derive_freeze_request_v5(
            state,
            config,
            outcome_count,
            product_record,
        )?),
        Action::InitializeSettlement => decoded_v2_request(derive_initialize_request_v5(
            state,
            config,
            outcome_count,
            product_record,
        )?),
        Action::Collect | Action::Materialize | Action::Distribute | Action::Close => {
            decoded_v2_request(derive_settlement_request_v5(
                state,
                config,
                action,
                outcome_count,
                product_record,
            )?)
        }
    }
}

fn derive_open_request_v5(
    state: &GeneralHotStateV3,
    artifacts: GeneralArtifactBytesV3<'_>,
    outcome_count: u32,
    config_id: [u8; 32],
    config: GeneralConfigV3,
    product_record: [u8; 32],
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    let root = authenticated_active_root_v5(state, artifacts, config_id)?;
    derive_open_request_from_root_v5(outcome_count, config_id, config, product_record, root)
}

fn derive_open_request_from_root_v5(
    outcome_count: u32,
    config_id: [u8; 32],
    config: GeneralConfigV3,
    product_record: [u8; 32],
    root: GeneralRootV2,
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    if config_id != root.config_id() || config.generation() != root.generation() {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    let occurrence = GeneralBatchOccurrenceTermsV1::new(GeneralBatchOpeningV1 {
        outcome_count,
        sequence: root.next_batch_sequence(),
        generation: root.generation(),
        market: root.market(),
        product_id: product_record,
        config_id,
        price_scale: config.price_scale(),
        collection_close_slot: 0,
        settlement_close_slot: 0,
        max_orders: config.max_orders_per_candidate(),
    })
    .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    canonical_front_request_v5(
        Action::OpenBatch,
        occurrence.occurrence_id(),
        root.revision(),
    )
}

fn decoded_v2_request(
    request: ControllerRequestV2,
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    let bytes = request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    decode_general_request_v3(&bytes).map_err(|_| GeneralHotOperatorErrorV3::ChainState)
}

fn derive_front_request_v5(
    state: &GeneralHotStateV3,
    artifacts: GeneralArtifactBytesV3<'_>,
    action: Action,
    outcome_count: u32,
    config_id: [u8; 32],
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    let root = authenticated_active_root_v5(state, artifacts, config_id)?;

    derive_front_request_from_root_v5(state, action, outcome_count, config_id, root)
}

fn authenticated_active_root_v5(
    state: &GeneralHotStateV3,
    artifacts: GeneralArtifactBytesV3<'_>,
    config_id: [u8; 32],
) -> Result<GeneralRootV2, GeneralHotOperatorErrorV3> {
    let descriptor = CapabilityProgramV4::decode(artifacts.descriptor)
        .map_err(|_| GeneralHotOperatorErrorV3::Artifact)?;
    let root_account = state
        .fixed_accounts
        .get(HOT_ROOT_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?;
    let composite = CapabilityRootAccountV4::decode(&root_account.account.data, descriptor)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let root = GeneralRootV2::decode(composite.state())
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let market = state
        .fixed_accounts
        .get(HOT_MARKET_ACCOUNT_V3)
        .ok_or(GeneralHotOperatorErrorV3::FixedFrame)?
        .account
        .key
        .to_bytes();
    if root.lifecycle() != GeneralLifecycleV2::Active
        || root.market() != market
        || root.config_id() != config_id
        || root.generation() != state.generation
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    Ok(root)
}

fn derive_front_request_from_root_v5(
    state: &GeneralHotStateV3,
    action: Action,
    outcome_count: u32,
    config_id: [u8; 32],
    root: GeneralRootV2,
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    let (subject, expected_revision) = match action {
        Action::CloseBatch => {
            let body = primary_state_body_v5(state, GeneralLocalStateKindV3::Batch)?
                .ok_or(GeneralHotOperatorErrorV3::ChainState)?;
            let batch =
                GeneralBatchV1::decode(body).map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            authenticate_operator_batch_v5(batch, root, outcome_count, config_id)?;
            if batch.state().status != BatchStatusV1::Collecting {
                return Err(GeneralHotOperatorErrorV3::ChainState);
            }
            (batch.batch_id(), root.revision())
        }
        Action::PlaceOrder => {
            let body = primary_state_body_v5(state, GeneralLocalStateKindV3::Batch)?
                .ok_or(GeneralHotOperatorErrorV3::ChainState)?;
            let batch =
                GeneralBatchV1::decode(body).map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            authenticate_operator_batch_v5(batch, root, outcome_count, config_id)?;
            if batch.state().status != BatchStatusV1::Collecting {
                return Err(GeneralHotOperatorErrorV3::ChainState);
            }
            let evidence = readonly_evidence_account_v5(
                state,
                action,
                GeneralReadonlyEvidenceKindV3::OrderTerms,
            )?;
            let terms = GeneralSignedOrderTermsV1::decode(&evidence.account.data)
                .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            let header = terms.header();
            if header.outcome_count != outcome_count
                || header.market != root.market()
                || header.batch_id != batch.batch_id()
                || header.generation != root.generation()
            {
                return Err(GeneralHotOperatorErrorV3::ChainState);
            }
            (terms.order_id(), 0)
        }
        Action::CancelOrder => {
            let batch_body = primary_state_body_v5(state, GeneralLocalStateKindV3::Batch)?
                .ok_or(GeneralHotOperatorErrorV3::ChainState)?;
            let batch = GeneralBatchV1::decode(batch_body)
                .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            authenticate_operator_batch_v5(batch, root, outcome_count, config_id)?;
            let order_body = secondary_state_body_v5(state, GeneralLocalStateKindV3::Order)?
                .ok_or(GeneralHotOperatorErrorV3::ChainState)?;
            let order = GeneralOrderV1::decode(order_body)
                .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            authenticate_operator_order_v5(order, root, outcome_count, Some(batch.batch_id()))?;
            if batch.state().status != BatchStatusV1::Collecting
                || order.state().phase != GeneralOrderPhaseV1::Placed
            {
                return Err(GeneralHotOperatorErrorV3::ChainState);
            }
            (order.order_id(), 0)
        }
        Action::ReleaseOrder => {
            let body = primary_state_body_v5(state, GeneralLocalStateKindV3::Order)?
                .ok_or(GeneralHotOperatorErrorV3::ChainState)?;
            let order =
                GeneralOrderV1::decode(body).map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            authenticate_operator_order_v5(order, root, outcome_count, None)?;
            if order.state().phase != GeneralOrderPhaseV1::Placed {
                return Err(GeneralHotOperatorErrorV3::ChainState);
            }
            (order.order_id(), 0)
        }
        Action::CloseCandidate => {
            let body = primary_state_body_v5(state, GeneralLocalStateKindV3::Candidate)?
                .ok_or(GeneralHotOperatorErrorV3::ChainState)?;
            let candidate = GeneralCandidateV1::decode(body)
                .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            let opening = candidate.opening();
            if opening.outcome_count != outcome_count {
                return Err(GeneralHotOperatorErrorV3::ChainState);
            }
            let batch_body = readonly_local_state_body_v5(
                state,
                action,
                GeneralReadonlyEvidenceKindV3::ClosedBatch,
                GeneralLocalStateKindV3::Batch,
            )?;
            let batch = GeneralBatchV1::decode(batch_body)
                .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
            authenticate_operator_batch_v5(batch, root, outcome_count, config_id)?;
            let payer =
                logical_runtime_account(state, usize::from(GENERAL_PRIMARY_PAYER_ACCOUNT_V3))?;
            let solver = logical_runtime_account(
                state,
                usize::from(GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3),
            )?;
            if batch.state().status != BatchStatusV1::Closed
                || opening.batch_id != batch.batch_id()
                || solver.account.key.to_bytes() != opening.solver_id
                || !payer.is_signer
                || !payer.is_writable
                || payer.account.key == Pubkey::default()
                || solver.is_signer
                || !solver.is_writable
            {
                return Err(GeneralHotOperatorErrorV3::ChainState);
            }
            (opening.candidate_id, 0)
        }
        Action::Consider
        | Action::Freeze
        | Action::InitializeSettlement
        | Action::Collect
        | Action::Materialize
        | Action::Distribute
        | Action::Close
        | Action::OpenBatch
        | Action::SubmitCandidate
        | Action::VerifyCandidateRow => return Err(GeneralHotOperatorErrorV3::ChainState),
    };
    canonical_front_request_v5(action, subject, expected_revision)
}

fn canonical_front_request_v5(
    action: Action,
    subject: [u8; 32],
    expected_revision: u64,
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    let request = GeneralDecodedRequestV3 {
        wire: GeneralRequestWireV3::V3,
        action,
        expected_revision,
        candidate_id: Some(subject),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: 0,
        terminal_record_bump: 0,
        result_state_bump: 0,
    };
    let bytes = request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let decoded =
        decode_general_request_v3(&bytes).map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if decoded != request {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    Ok(decoded)
}

fn derive_submit_request_v5(
    state: &GeneralHotStateV3,
    artifacts: GeneralArtifactBytesV3<'_>,
    outcome_count: u32,
    config_id: [u8; 32],
    config: GeneralConfigV3,
    product_record: [u8; 32],
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    let root = authenticated_active_root_v5(state, artifacts, config_id)?;
    derive_submit_request_from_root_v5(
        state,
        outcome_count,
        config_id,
        config,
        product_record,
        root,
    )
}

fn derive_submit_request_from_root_v5(
    state: &GeneralHotStateV3,
    outcome_count: u32,
    config_id: [u8; 32],
    config: GeneralConfigV3,
    product_record: [u8; 32],
    root: GeneralRootV2,
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    if primary_state_body_v5(state, GeneralLocalStateKindV3::Candidate)?.is_some() {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    let batch_body = readonly_local_state_body_v5(
        state,
        Action::SubmitCandidate,
        GeneralReadonlyEvidenceKindV3::ClosedBatch,
        GeneralLocalStateKindV3::Batch,
    )?;
    let batch =
        GeneralBatchV1::decode(batch_body).map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    authenticate_operator_batch_v5(batch, root, outcome_count, config_id)?;
    let batch_opening = batch.opening();
    let settlement_duration = config
        .selection_slots()
        .checked_add(config.settlement_slots())
        .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
    let expected_settlement_close = batch_opening
        .collection_close_slot
        .checked_add(settlement_duration)
        .ok_or(GeneralHotOperatorErrorV3::Arithmetic)?;
    if config.generation() != root.generation()
        || batch.state().status != BatchStatusV1::Closed
        || batch_opening.product_id != product_record
        || batch_opening.price_scale != config.price_scale()
        || batch_opening.max_orders != config.max_orders_per_candidate()
        || batch_opening.settlement_close_slot != expected_settlement_close
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }

    let candidate_account = readonly_evidence_account_v5(
        state,
        Action::SubmitCandidate,
        GeneralReadonlyEvidenceKindV3::CandidateImage,
    )?;
    let candidate = CandidateV2::decode(&candidate_account.account.data)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    authenticate_candidate_identity_v1(candidate)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let candidate_header = candidate.header();
    if candidate_header.outcome_count != outcome_count
        || candidate_header.product_id != product_record
        || candidate_header.batch_id != batch.batch_id()
        || candidate_header.price_scale != config.price_scale()
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }

    let submitted_account = readonly_evidence_account_v5(
        state,
        Action::SubmitCandidate,
        GeneralReadonlyEvidenceKindV3::SubmittedCandidate,
    )?;
    let submitted = GeneralCandidateV1::decode(&submitted_account.account.data)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let opening = submitted.opening();
    let expected = GeneralCandidateV1::submit(
        batch,
        candidate,
        opening.page_revision,
        opening.row_count,
        opening.reward_rate_lamports,
        opening.solver_id,
        opening
            .work_capacity()
            .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?,
        opening.submitted_slot,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if expected != submitted || opening.candidate_id != candidate_header.candidate_id {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    let payer = logical_runtime_account(state, usize::from(GENERAL_PRIMARY_PAYER_ACCOUNT_V3))?;
    if !payer.is_signer || !payer.is_writable || payer.account.key.to_bytes() != opening.solver_id {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }

    let request = GeneralDecodedRequestV3 {
        wire: GeneralRequestWireV3::V3,
        action: Action::SubmitCandidate,
        expected_revision: 0,
        candidate_id: Some(candidate_header.candidate_id),
        page_index: 0,
        execution_index: 0,
        manifest_order_index: 0,
        state_bump: 0,
        terminal_record_bump: 0,
        result_state_bump: 0,
    };
    let bytes = request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if decode_general_request_v3(&bytes).map_err(|_| GeneralHotOperatorErrorV3::ChainState)?
        != request
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    Ok(request)
}

fn derive_verify_request_v5(
    state: &GeneralHotStateV3,
    artifacts: GeneralArtifactBytesV3<'_>,
    outcome_count: u32,
    config_id: [u8; 32],
    product_record: [u8; 32],
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    let root = authenticated_active_root_v5(state, artifacts, config_id)?;
    derive_verify_request_from_root_v5(state, outcome_count, config_id, product_record, root)
}

fn derive_verify_request_from_root_v5(
    state: &GeneralHotStateV3,
    outcome_count: u32,
    config_id: [u8; 32],
    product_record: [u8; 32],
    root: GeneralRootV2,
) -> Result<GeneralDecodedRequestV3, GeneralHotOperatorErrorV3> {
    let submission_body = primary_state_body_v5(state, GeneralLocalStateKindV3::Candidate)?
        .ok_or(GeneralHotOperatorErrorV3::ChainState)?;
    let submission = GeneralCandidateV1::decode(submission_body)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let opening = submission.opening();

    let batch_body = readonly_local_state_body_v5(
        state,
        Action::VerifyCandidateRow,
        GeneralReadonlyEvidenceKindV3::ClosedBatch,
        GeneralLocalStateKindV3::Batch,
    )?;
    let batch =
        GeneralBatchV1::decode(batch_body).map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    authenticate_operator_batch_v5(batch, root, outcome_count, config_id)?;
    if batch.opening().product_id != product_record
        || opening.batch_id != batch.batch_id()
        || opening.outcome_count != outcome_count
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }

    let candidate = readonly_evidence_account_v5(
        state,
        Action::VerifyCandidateRow,
        GeneralReadonlyEvidenceKindV3::CandidateImage,
    )?;
    let page = readonly_evidence_account_v5(
        state,
        Action::VerifyCandidateRow,
        GeneralReadonlyEvidenceKindV3::CandidatePage,
    )?;
    let order = readonly_evidence_account_v5(
        state,
        Action::VerifyCandidateRow,
        GeneralReadonlyEvidenceKindV3::EscrowedOrder,
    )?;
    let manifest = readonly_evidence_account_v5(
        state,
        Action::VerifyCandidateRow,
        GeneralReadonlyEvidenceKindV3::SettlementManifest,
    )?;
    let verifier_len =
        candidate_verifier_len_v1(submission).map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let certificate_len = candidate_certificate_len_v1(submission)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let vacant_cursor = vec![0_u8; verifier_len];
    let persisted_cursor = secondary_state_body_v5(state, GeneralLocalStateKindV3::Verifier)?;
    let cursor_before = persisted_cursor.unwrap_or(&vacant_cursor);
    let (expected_page_index, expected_row_index, expected_revision) = if persisted_cursor.is_none()
    {
        (0, 0, 0)
    } else {
        let header = RuntimeCandidateVerifierV2::decode(cursor_before)
            .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?
            .header();
        (
            header.next_page_index,
            header.next_row_index,
            header.revision,
        )
    };
    let result_account =
        logical_runtime_account(state, usize::from(GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3))?;
    if !result_account.account.data.is_empty() {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    let verified_before = vec![0_u8; certificate_len];
    let view = CandidateVerifyRowViewV1 {
        batch,
        submission,
        candidate: &candidate.account.data,
        page: &page.account.data,
        order: &order.account.data,
        cursor_before,
        verified_before: &verified_before,
        expected_page_index,
        expected_row_index,
        expected_revision,
    };
    let manifest_order_count = candidate_verify_manifest_orders_v1(&view)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let manifest_len = settlement_manifest_len_v2(outcome_count, manifest_order_count)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    let mut cursor_scratch = vec![0_u8; verifier_len];
    let mut cursor_output = vec![0_u8; verifier_len];
    let mut verified_scratch = vec![0_u8; certificate_len];
    let mut verified_output = vec![0_u8; certificate_len];
    let mut manifest_scratch = vec![0_u8; manifest_len];
    let mut manifest_output = vec![0_u8; manifest_len];
    verify_candidate_row_v1(
        view,
        CandidateVerifyRowBuffersV1 {
            cursor_scratch: &mut cursor_scratch,
            cursor_output: &mut cursor_output,
            verified_scratch: &mut verified_scratch,
            verified_output: &mut verified_output,
            manifest_scratch: &mut manifest_scratch,
            manifest_output: &mut manifest_output,
        },
    )
    .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if manifest_output != manifest.account.data {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }

    let request = GeneralDecodedRequestV3 {
        wire: GeneralRequestWireV3::V3,
        action: Action::VerifyCandidateRow,
        expected_revision,
        candidate_id: Some(opening.candidate_id),
        page_index: expected_page_index,
        execution_index: u8::try_from(expected_row_index)
            .map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
        manifest_order_index: 0,
        state_bump: 0,
        terminal_record_bump: 0,
        result_state_bump: 0,
    };
    let bytes = request
        .to_bytes()
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if decode_general_request_v3(&bytes).map_err(|_| GeneralHotOperatorErrorV3::ChainState)?
        != request
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    Ok(request)
}

fn authenticate_operator_batch_v5(
    batch: GeneralBatchV1,
    root: GeneralRootV2,
    outcome_count: u32,
    config_id: [u8; 32],
) -> Result<(), GeneralHotOperatorErrorV3> {
    let opening = batch.opening();
    if opening.outcome_count != outcome_count
        || opening.market != root.market()
        || opening.config_id != config_id
        || opening.generation != root.generation()
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    Ok(())
}

fn authenticate_operator_order_v5(
    order: GeneralOrderV1<'_>,
    root: GeneralRootV2,
    outcome_count: u32,
    expected_batch: Option<[u8; 32]>,
) -> Result<(), GeneralHotOperatorErrorV3> {
    let header = order.header();
    if header.outcome_count != outcome_count
        || header.market != root.market()
        || header.generation != root.generation()
        || expected_batch.is_some_and(|batch| header.batch_id != batch)
    {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    Ok(())
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
        Action::Consider
        | Action::Freeze
        | Action::InitializeSettlement
        | Action::OpenBatch
        | Action::PlaceOrder
        | Action::CancelOrder
        | Action::CloseBatch
        | Action::SubmitCandidate
        | Action::VerifyCandidateRow
        | Action::ReleaseOrder
        | Action::CloseCandidate => {
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
    local_state_body_v5(
        state,
        usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
        expected_kind,
    )
}

fn secondary_state_body_v5(
    state: &GeneralHotStateV3,
    expected_kind: GeneralLocalStateKindV3,
) -> Result<Option<&[u8]>, GeneralHotOperatorErrorV3> {
    local_state_body_v5(
        state,
        usize::from(GENERAL_TERMINAL_STATE_ACCOUNT_V3),
        expected_kind,
    )
}

fn local_state_body_v5(
    state: &GeneralHotStateV3,
    coordinate: usize,
    expected_kind: GeneralLocalStateKindV3,
) -> Result<Option<&[u8]>, GeneralHotOperatorErrorV3> {
    let account = logical_runtime_account(state, coordinate)?;
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

fn readonly_local_state_body_v5(
    state: &GeneralHotStateV3,
    action: Action,
    evidence_kind: GeneralReadonlyEvidenceKindV3,
    state_kind: GeneralLocalStateKindV3,
) -> Result<&[u8], GeneralHotOperatorErrorV3> {
    let account = readonly_evidence_account_v5(state, action, evidence_kind)?;
    let local = GeneralLocalStateV3::decode(&account.account.data)
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?;
    if local.header().kind != state_kind {
        return Err(GeneralHotOperatorErrorV3::ChainState);
    }
    Ok(local.body())
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
    let caller_count = usize::try_from(selected_admitted_invocation_count_v3(bundle)?)
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

fn selected_admitted_invocation_count_v3(
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
    // ZERO SPANS. General's only dynamic span was the input scratch-page
    // transport; the bank rides inline in the CPI instruction data now and the
    // profile declares none. The artifact profile stays Profile13, which is the
    // encoder carrying General's trusted environment and variable-data
    // prestates, so this asks for the count rather than for the discriminator
    // alone.
    if profile.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || profile.dynamic_fixed_span_count() != 0
    {
        return Err(GeneralHotOperatorErrorV3::RuntimeGeometry);
    }
    let span_counts: [u32; 0] = [];
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
    // The span's own conjuncts are gone with the span: there is no selector to
    // pin to `INPUT_SCRATCH_PAGE_COUNT` and no insertion coordinate to add a
    // width to. `logical_account_count_with_dynamic_spans` above already
    // refuses a width vector this profile does not declare.
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
    request: GeneralDecodedRequestV3,
    trading_program: Pubkey,
) -> Result<GeneralLifecycleProjectionV3, GeneralHotOperatorErrorV3> {
    // ONE POLICY FOR THE FAMILY, rebuilt without reference to the executing
    // action. The action still selects its plans out of it below; what it no
    // longer selects is a different artifact, because a Market's capability
    // manifest binds one `child_derivation_id` and fifteen artifacts have
    // fifteen digests.
    let policy_bytes = general_family_state_lifecycle_bytes_v5();
    let mut scratch = vec![0_u8; policy_bytes];
    let mut canonical = vec![0_u8; policy_bytes];
    let child_widths = selected_child_rent_widths_v5(bundle)?;
    encode_general_family_state_lifecycle_v5_atomic(child_widths, &mut scratch, &mut canonical)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    if bundle.lifecycle_policy.bytes() != canonical.as_slice() {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }

    let plan_count = bundle
        .lifecycle_policy
        .action_plan_count(request.action as u32)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let verify = request.action == Action::VerifyCandidateRow;
    let two_state = matches!(
        request.action,
        Action::Close | Action::PlaceOrder | Action::CancelOrder
    );
    let expected_plan_count = if verify {
        3
    } else if two_state {
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
    project_general_lifecycle_seed_identities_v5(state, request, &mut identities)?;
    let terminal_coordinate =
        canonical_terminal_coordinate_v3(request.action, request.expected_revision)?;
    if verify {
        // The public projection always derives the conditional Result address,
        // including on a nonterminal row. Enabling the already-authenticated
        // guard here selects its address recipe only; it does not claim that
        // the observed Result is materialized or that this row will create it.
        set_scalar(&mut scalars, general_scalar::VERIFY_TERMINAL, 1)?;
    }
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
    // THE JOIN IS RECORDED, AND IT IS THE ACTION'S.
    //
    // Every seed materialization below asks `require_join` whether this policy
    // and this AccountProfile were already proved to fit, and re-derives the
    // join when no evidence is attached -- with the WHOLE-POLICY form, which for
    // a family policy asks whether fifteen actions' plans fit one action's
    // frame. Attaching the action-scoped evidence once is what makes the
    // fallback unreachable rather than merely unused.
    let join = bundle
        .lifecycle_policy
        .validate_account_profile_join_for_action(bundle.account_profile, request.action as u32)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
    let primary_plan = bundle
        .lifecycle_policy
        .action_plan(request.action as u32, 0)
        .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?
        .with_validated_join(join);
    let primary_close = matches!(request.action, Action::Close | Action::CloseCandidate);
    let primary_expected_operation = if verify {
        LifecycleOperationV3::Authenticate
    } else if primary_close {
        LifecycleOperationV3::Close
    } else {
        LifecycleOperationV3::AuthenticateOrCreate
    };
    let primary_recipe = GeneralStateRecipeV3::primary_for_action(request.action);
    let primary = derive_lifecycle_state_v3(
        state,
        bundle.account_profile,
        bundle.tail_count,
        registers,
        primary_plan,
        primary_expected_operation,
        usize::from(GENERAL_PRIMARY_STATE_ACCOUNT_V3),
        if verify || primary_close {
            None
        } else if two_state {
            Some(usize::from(GENERAL_CLOSE_PAYER_ACCOUNT_V3))
        } else {
            Some(usize::from(GENERAL_PRIMARY_PAYER_ACCOUNT_V3))
        },
        if verify {
            None
        } else {
            Some(usize::from(if two_state {
                GENERAL_CLOSE_RENT_CREDIT_ACCOUNT_V3
            } else {
                GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3
            }))
        },
        trading_program,
        Some(local_state_kind_for_recipe_v5(primary_recipe)?),
    )?;

    let secondary = if verify {
        // Canonical policy order is Candidate/Authenticate,
        // Result/Create, Verifier/AuthenticateOrCreate. Physical coordinate
        // order remains Candidate, Verifier, Result.
        let plan = bundle
            .lifecycle_policy
            .action_plan(request.action as u32, 2)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?
            .with_validated_join(join);
        Some(derive_lifecycle_state_v3(
            state,
            bundle.account_profile,
            bundle.tail_count,
            registers,
            plan,
            LifecycleOperationV3::AuthenticateOrCreate,
            usize::from(GENERAL_VERIFY_VERIFIER_STATE_ACCOUNT_V3),
            Some(usize::from(GENERAL_VERIFY_PAYER_ACCOUNT_V3)),
            Some(usize::from(GENERAL_VERIFY_RENT_CREDIT_ACCOUNT_V3)),
            trading_program,
            Some(GeneralLocalStateKindV3::Verifier),
        )?)
    } else if two_state {
        let plan = bundle
            .lifecycle_policy
            .action_plan(request.action as u32, 1)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?
            .with_validated_join(join);
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
            Some(if request.action == Action::Close {
                GeneralLocalStateKindV3::Settlement
            } else {
                GeneralLocalStateKindV3::Order
            }),
        )?;
        Some(terminal)
    } else {
        None
    };
    let conditional_result = if verify {
        let plan = bundle
            .lifecycle_policy
            .action_plan(request.action as u32, 1)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?
            .with_validated_join(join);
        Some(derive_lifecycle_state_v3(
            state,
            bundle.account_profile,
            bundle.tail_count,
            registers,
            plan,
            LifecycleOperationV3::Create,
            usize::from(GENERAL_VERIFY_RESULT_STATE_ACCOUNT_V3),
            Some(usize::from(GENERAL_VERIFY_PAYER_ACCOUNT_V3)),
            Some(usize::from(GENERAL_VERIFY_RENT_CREDIT_ACCOUNT_V3)),
            trading_program,
            None,
        )?)
    } else {
        None
    };
    if secondary.is_some_and(|value| value.account == primary.account)
        || conditional_result.is_some_and(|value| {
            value.account == primary.account
                || secondary.is_some_and(|secondary| secondary.account == value.account)
        })
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    // Geometry coherence between the emitter's tables and this operator's
    // frame. Both conjuncts compare two quantities with independent authors, so
    // neither is a self-comparison.
    //
    // The single check this replaces compared `general_child_account_start_v3`
    // against literal 8/9 -- which is `general_readonly_evidence_start_v3`'s own
    // table, copied. Children begin AFTER evidence, so that equality holds only
    // for an action with no readonly evidence at all: the check refused six of
    // the seven actions outright. It had never fired because
    // `build_general_hot_instruction_v3` had no caller.
    //
    // (a) Readonly evidence begins exactly at the fixed-prefix boundary. The
    //     literals belong to THIS quantity: the five injected Hot runtime
    //     representatives plus the action's own lifecycle accounts: five for
    //     Verify, four for the other multi-state actions, and three otherwise.
    let expected_evidence_start = if verify {
        10
    } else if two_state {
        9
    } else {
        8
    };
    if general_readonly_evidence_start_v3(request.action) != expected_evidence_start {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    // (b) Children begin exactly where evidence ends: no gap, no overlap. The
    //     independent side is the EffectProgram's own route table, which is
    //     authored separately from the evidence coordinates -- not
    //     `general_child_account_start_v3`'s definition restated back at it.
    let child_account_start = general_child_account_start_v3(request.action);
    if general_effect_route_count_v3(request.action) != 0 {
        let first = general_effect_route_frame_v3(request.action, 0)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
        if first.account_start != child_account_start {
            return Err(GeneralHotOperatorErrorV3::Lifecycle);
        }
    }
    let projection = GeneralLifecycleProjectionV3 {
        primary,
        secondary,
        conditional_result,
        terminal_coordinate,
        child_account_start,
    };
    if (request.state_bump != 0 && request.state_bump != projection.primary.bump)
        || (request.terminal_record_bump != 0
            && Some(request.terminal_record_bump) != projection.secondary.map(|value| value.bump))
        || (request.result_state_bump != 0
            && Some(request.result_state_bump)
                != projection.conditional_result.map(|value| value.bump))
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    Ok(projection)
}

fn project_general_lifecycle_seed_identities_v5(
    state: &GeneralHotStateV3,
    request: GeneralDecodedRequestV3,
    identities: &mut [[u8; 32]],
) -> Result<(), GeneralHotOperatorErrorV3> {
    let subject = request.candidate_id.filter(|value| *value != [0; 32]);
    match GeneralStateRecipeV3::primary_for_action(request.action) {
        // ONE CURSOR PER BATCH, and this builder is the SECOND author of the
        // address that says so. `GENERAL_SELECTION_STATE_RECIPE_V3` gained the
        // batch identity register on 2026-09-04 and the AccountProfile gained
        // the two operations that write it; this arm did not, and a register
        // nothing writes is a well-formed zero -- one identical, wrong
        // selection address for every batch under every root, refused as
        // `Lifecycle` against the account the chain really holds. The two
        // sources below are the SAME two accounts the profile projects from,
        // read here through the records' own decoders.
        GeneralStateRecipeV3::Selection => set_identity(
            identities,
            general_identity::SELECTION_BATCH,
            selection_batch_identity_v5(state, request.action)?,
        )?,
        GeneralStateRecipeV3::Settlement | GeneralStateRecipeV3::Candidate => set_identity(
            identities,
            general_identity::CANDIDATE,
            subject.ok_or(GeneralHotOperatorErrorV3::Lifecycle)?,
        )?,
        GeneralStateRecipeV3::Batch => {
            let batch_id = if matches!(request.action, Action::OpenBatch | Action::CloseBatch) {
                subject.ok_or(GeneralHotOperatorErrorV3::Lifecycle)?
            } else {
                let body = primary_state_body_v5(state, GeneralLocalStateKindV3::Batch)?
                    .ok_or(GeneralHotOperatorErrorV3::Lifecycle)?;
                GeneralBatchV1::decode(body)
                    .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?
                    .batch_id()
            };
            set_identity(identities, general_identity::SELECTION_BATCH, batch_id)?;
            if matches!(request.action, Action::PlaceOrder | Action::CancelOrder) {
                set_identity(
                    identities,
                    general_identity::ORDER,
                    subject.ok_or(GeneralHotOperatorErrorV3::Lifecycle)?,
                )?;
            }
        }
        GeneralStateRecipeV3::Order => set_identity(
            identities,
            general_identity::ORDER,
            subject.ok_or(GeneralHotOperatorErrorV3::Lifecycle)?,
        )?,
        // These are secondary/conditional Verify recipes and can never be the
        // primary recipe selected by `primary_for_action`. Refuse if an
        // artifact ever tries to make either one primary rather than silently
        // projecting it as a local envelope.
        GeneralStateRecipeV3::Terminal
        | GeneralStateRecipeV3::Verifier
        | GeneralStateRecipeV3::VerifiedCandidate => {
            return Err(GeneralHotOperatorErrorV3::Lifecycle);
        }
    }
    Ok(())
}

/// The batch a selection cursor belongs to, from the record that owns the fact.
///
/// NEITHER SOURCE IS THE CALLER, and the two differ for one reason: the first
/// `Consider` of a batch CREATES the cursor, so at that moment the cursor is
/// thirty-two zero bytes and cannot name anything. The submitted
/// `VerifiedCandidate` already names its batch and is the record
/// `consider_verified_candidate_v2` compares against, so it is the source while
/// the cursor is being opened; by `Freeze` the cursor exists and is the record
/// whose `batch_id` the accelerator joins against the presented Batch, so it is
/// the source once it does. Deriving the address from a field of the account AT
/// that address is not circular: the derived address must BE the supplied
/// account's key, so only the genuine cursor for that batch satisfies it.
///
/// This mirrors `general_account_profile_operation_v3`'s two
/// `ProjectDataIdentity` operations exactly, at the same two coordinates, and a
/// disagreement between them surfaces as a refused build rather than a wrong
/// instruction: the address this function feeds is compared against the account
/// the chain holds.
fn selection_batch_identity_v5(
    state: &GeneralHotStateV3,
    action: Action,
) -> Result<[u8; 32], GeneralHotOperatorErrorV3> {
    match action {
        Action::Consider => Ok(VerifiedCandidateV2::decode(
            &readonly_evidence_account_v5(
                state,
                Action::Consider,
                GeneralReadonlyEvidenceKindV3::SubmittedVerifiedCandidate,
            )?
            .account
            .data,
        )
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?
        .header()
        .batch_id),
        Action::Freeze => Ok(RuntimeSelectionCursorV2::decode(
            primary_state_body_v5(state, GeneralLocalStateKindV3::Selection)?
                .ok_or(GeneralHotOperatorErrorV3::ChainState)?,
        )
        .map_err(|_| GeneralHotOperatorErrorV3::ChainState)?
        .header()
        .batch_id),
        // `primary_for_action` maps exactly these two onto the selection
        // recipe. A third would need its own source named here rather than
        // silently seeding on a zero.
        _ => Err(GeneralHotOperatorErrorV3::Lifecycle),
    }
}

fn local_state_kind_for_recipe_v5(
    recipe: GeneralStateRecipeV3,
) -> Result<GeneralLocalStateKindV3, GeneralHotOperatorErrorV3> {
    match recipe {
        GeneralStateRecipeV3::Selection => Ok(GeneralLocalStateKindV3::Selection),
        GeneralStateRecipeV3::Settlement | GeneralStateRecipeV3::Terminal => {
            Ok(GeneralLocalStateKindV3::Settlement)
        }
        GeneralStateRecipeV3::Batch => Ok(GeneralLocalStateKindV3::Batch),
        GeneralStateRecipeV3::Order => Ok(GeneralLocalStateKindV3::Order),
        GeneralStateRecipeV3::Candidate => Ok(GeneralLocalStateKindV3::Candidate),
        // Verifier has its own runtime body layout and VerifiedCandidate is a
        // raw terminal result. Neither may be decoded as a General local-state
        // envelope by this primary-only helper.
        GeneralStateRecipeV3::Verifier | GeneralStateRecipeV3::VerifiedCandidate => {
            Err(GeneralHotOperatorErrorV3::Lifecycle)
        }
    }
}

/// Recover the sole release-variable input to the family lifecycle policy.
///
/// Everything else the family encoder derives: Product N comes from the
/// authenticated tail count, and the three fixed child widths come from their
/// semantic owners. The selected Token or Token-2022 vault width does not --
/// a release chooses it -- so it is read back off the policy's own
/// InitializeSettlement custody-vault quote and never from the family request
/// or from GeneralConfig.
///
/// This function used to also re-assert the whole quote table by ordinal, per
/// action. It no longer does, and that is not a weakening: the caller rebuilds
/// the ENTIRE policy from this one width and compares it byte for byte, which
/// subsumes every conjunct that was here and holds for all fifteen actions
/// rather than for the three that declare quotes.
fn selected_child_rent_widths_v5(
    bundle: dclutch_general_adapter_contract::artifacts_v3::GeneralArtifactBundleV3<'_>,
) -> Result<GeneralChildRentWidthsV5, GeneralHotOperatorErrorV3> {
    if usize::from(bundle.lifecycle_policy.current_rent_quote_count())
        != GENERAL_FAMILY_QUOTE_COUNT_V5
    {
        return Err(GeneralHotOperatorErrorV3::Lifecycle);
    }
    let mut vault_width = None;
    for ordinal in 0..bundle.lifecycle_policy.current_rent_quote_count() {
        let quote = bundle
            .lifecycle_policy
            .current_rent_quote(ordinal)
            .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)?;
        let destination = quote.scalar_destination();
        if destination.kind() != LifecycleRegisterKindV3::Scalar
            || destination.scope() != CoordinateScopeV3::Fixed
            || u32::from(destination.index()) != general_scalar::CUSTODY_VAULT_RENT_LAMPORTS
            || !quote.applies_to(Action::InitializeSettlement as u32)
        {
            continue;
        }
        if vault_width.is_some() || quote.exact_data_len() == 0 {
            return Err(GeneralHotOperatorErrorV3::Lifecycle);
        }
        vault_width = Some(quote.exact_data_len());
    }
    GeneralChildRentWidthsV5::new(
        bundle.tail_count,
        vault_width.ok_or(GeneralHotOperatorErrorV3::Lifecycle)?,
    )
    .map_err(|_| GeneralHotOperatorErrorV3::Lifecycle)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DerivedLifecycleStateV3 {
    account_coordinate: u16,
    account: Pubkey,
    bump: u8,
    is_materialized: bool,
}

impl From<DerivedLifecycleStateV3> for GeneralLifecycleStateProjectionV3 {
    fn from(value: DerivedLifecycleStateV3) -> Self {
        Self {
            account_coordinate: value.account_coordinate,
            account: value.account,
            bump: value.bump,
            is_materialized: value.is_materialized,
        }
    }
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
    state_kind: Option<GeneralLocalStateKindV3>,
) -> Result<GeneralLifecycleStateProjectionV3, GeneralHotOperatorErrorV3> {
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
        let state_kind = state_kind.ok_or(GeneralHotOperatorErrorV3::Lifecycle)?;
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
    Ok(DerivedLifecycleStateV3 {
        account_coordinate: u16::try_from(indices.state())
            .map_err(|_| GeneralHotOperatorErrorV3::Arithmetic)?,
        account: key,
        bump,
        is_materialized: live,
    }
    .into())
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
        general_account_profile_rule_v3,
    };
    use dclutch_general_adapter_contract::collection_v1::{
        GeneralBatchOpeningV1, GeneralOrderHeaderV1, GeneralOrderStateV1, MakerFundingV1,
        general_order_len_v1, general_signed_order_terms_len_v1,
    };
    use dclutch_general_adapter_contract::hot_candidate_v3::{
        GENERAL_HOT_COMMON_IDENTITIES_V3, general_hot_scalar_count_v3,
    };
    use dclutch_general_adapter_contract::release_v3::GENERAL_ACTIONS_V3;
    use dclutch_general_adapter_contract::runtime_width::{
        CandidateHeaderV2, CandidateV2, ExecutionHeaderV2, ExecutionV2, PageHeaderV2, PageV2,
        SettlementCursorHeaderV2, SettlementPhaseV2, VerifiedCandidateHeaderV2, candidate_len,
        execution_len, page_len, settlement_cursor_len, verified_candidate_len,
    };
    use dclutch_general_adapter_contract::state_artifacts_v3::{
        encode_general_state_lifecycle_v3_atomic, general_state_lifecycle_bytes_v3,
    };
    use dclutch_general_adapter_contract::{
        candidate_v1::general_candidate_identity_v1,
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
                primary: GeneralLifecycleStateProjectionV3 {
                    account_coordinate: GENERAL_PRIMARY_STATE_ACCOUNT_V3,
                    account: key(203),
                    bump: 7,
                    is_materialized: true,
                },
                secondary: None,
                conditional_result: None,
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
        admitted_invocations: usize,
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

    /// Accelerator invocations, which is the caller-authority span length.
    ///
    /// Still `classify_bank_transport_v2`, because that is the OUTPUT question
    /// and the output still rides return data under `ChunkedBankV2`. It no
    /// longer counts input pages: there are none.
    fn general_admitted_invocations_v3(action: Action, outcome_count: u32) -> usize {
        let scalars =
            general_hot_scalar_count_v3(action, outcome_count).expect("General scalar count");
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
        let invocations = general_admitted_invocations_v3(action, outcome_count);
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
        // No page accounts to add to the runtime suffix, and none to grant
        // privileges to. The caller-authority span is still one account per
        // invocation and it sits in the strategy extras below.
        GeneralFrameGeometryV3 {
            accounts: HOT_FIXED_ACCOUNT_COUNT_V3
                + ADMITTED_AOT_FIXED_EXTRAS_V3
                + invocations
                + (physical_runtime - HOT_RUNTIME_LOGICAL_PREFIX_V3),
            writable,
            signers,
            admitted_invocations: invocations,
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
        let strategy = ADMITTED_AOT_FIXED_EXTRAS_V3 + geometry.admitted_invocations;
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

    fn artifact_carrier_state() -> GeneralHotStateV3 {
        let account = |value: u8, data: Vec<u8>| GeneralObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key: key(value),
                owner: key(200),
                lamports: 1_000_000,
                executable: false,
                data,
            },
            is_signer: false,
            is_writable: false,
        };
        GeneralHotStateV3 {
            fixed_accounts: (0..HOT_FIXED_ACCOUNT_COUNT_V3)
                .map(|index| {
                    account(
                        u8::try_from(index + 1).expect("fixed key"),
                        vec![u8::try_from(index).expect("fixed byte")],
                    )
                })
                .collect(),
            strategy_accounts: (0..ADMITTED_AOT_FIXED_EXTRAS_V3)
                .map(|index| {
                    account(
                        u8::try_from(0x80 + index).expect("strategy key"),
                        vec![u8::try_from(0x80 + index).expect("strategy byte")],
                    )
                })
                .collect(),
            runtime_suffix_accounts: Vec::new(),
            release_set: [1; 32],
            generation: 1,
            minimum_finalized_slot: observation().slot,
            checked_release: None,
        }
    }

    #[test]
    fn artifact_carriers_have_one_public_canonical_mapping() {
        let state = artifact_carrier_state();
        let bytes = general_artifact_bytes_from_hot_state_v3(&state).expect("artifact carriers");
        for (actual, coordinate) in [
            (bytes.program_set, HOT_PROGRAM_SET_RAW_ACCOUNT_V3),
            (bytes.descriptor, HOT_DESCRIPTOR_RAW_ACCOUNT_V3),
            (bytes.config, HOT_CONFIG_RAW_ACCOUNT_V3),
            (bytes.account_profile, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3),
            (bytes.lifecycle_policy, HOT_LIFECYCLE_RAW_ACCOUNT_V3),
            (bytes.request_profile, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3),
            (bytes.strategy, HOT_STRATEGY_RAW_ACCOUNT_V3),
            (bytes.transition, HOT_TRANSITION_RAW_ACCOUNT_V3),
            (bytes.effect, HOT_EFFECT_RAW_ACCOUNT_V3),
        ] {
            assert_eq!(actual, &[u8::try_from(coordinate).expect("fixed byte")]);
        }
        assert_eq!(
            bytes.certificate,
            &[u8::try_from(0x80 + ADMITTED_CERTIFICATE_RAW_EXTRA_V3).expect("certificate byte")]
        );
        assert_eq!(
            bytes.admission,
            &[u8::try_from(0x80 + ADMITTED_ADMISSION_RAW_EXTRA_V3).expect("admission byte")]
        );

        let mut short_fixed = state.clone();
        short_fixed.fixed_accounts.pop();
        assert!(matches!(
            general_artifact_bytes_from_hot_state_v3(&short_fixed),
            Err(GeneralHotOperatorErrorV3::FixedFrame)
        ));
        let mut short_strategy = state;
        short_strategy.strategy_accounts.clear();
        assert!(matches!(
            general_artifact_bytes_from_hot_state_v3(&short_strategy),
            Err(GeneralHotOperatorErrorV3::StrategyGeometry)
        ));
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

    fn front_local_state(kind: GeneralLocalStateKindV3, body: &[u8]) -> Vec<u8> {
        let width = general_local_state_len_v3(kind, 1).expect("local-state width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_general_local_state_v3_atomic(
            GeneralLocalStateHeaderV3 {
                kind,
                bump: 7,
                rent_principal: 1_000,
                beneficiary: [0x71; 32],
            },
            body,
            &mut scratch,
            &mut output,
        )
        .expect("canonical local state");
        output
    }

    fn front_state(
        primary: Vec<u8>,
        secondary: Option<Vec<u8>>,
        order_terms: Option<Vec<u8>>,
    ) -> GeneralHotStateV3 {
        let mut runtime_suffix_accounts = (0_u8..5)
            .map(|index| GeneralObservedAccountMetaV3 {
                account: ObservedAccount {
                    observation: observation(),
                    key: key(0x80 + index),
                    owner: key(200),
                    lamports: 1_000_000,
                    executable: false,
                    data: Vec::new(),
                },
                is_signer: false,
                is_writable: false,
            })
            .collect::<Vec<_>>();
        runtime_suffix_accounts[0].account.data = primary;
        if let Some(secondary) = secondary {
            runtime_suffix_accounts[1].account.data = secondary;
        }
        // PlaceOrder's sole evidence is logical coordinate nine, hence suffix
        // coordinate four after the five injected Hot representatives.
        if let Some(order_terms) = order_terms {
            runtime_suffix_accounts[4].account.data = order_terms;
        }
        GeneralHotStateV3 {
            fixed_accounts: Vec::new(),
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts,
            release_set: [1; 32],
            generation: 7,
            minimum_finalized_slot: observation().slot,
            checked_release: None,
        }
    }

    fn front_records() -> (GeneralRootV2, GeneralBatchV1, Vec<u8>, Vec<u8>) {
        let market = [0x41; 32];
        let config_id = [0x42; 32];
        let mut root = GeneralRootV2::active(market, config_id, 7).expect("active root");
        let expected_revision = root.revision();
        let batch = GeneralBatchV1::open(
            &mut root,
            GeneralBatchOpeningV1 {
                outcome_count: 1,
                sequence: 0,
                generation: 7,
                market,
                product_id: [0x43; 32],
                config_id,
                price_scale: 1,
                collection_close_slot: 50,
                settlement_close_slot: 100,
                max_orders: 8,
            },
            expected_revision,
            10,
        )
        .expect("open batch");
        let mut order_bytes = vec![0_u8; general_order_len_v1(1).expect("order width")];
        GeneralOrderV1::encode_rows_into(
            GeneralOrderHeaderV1 {
                outcome_count: 1,
                nonce: 9,
                owner_id: [0x44; 32],
                market,
                batch_id: batch.batch_id(),
                generation: 7,
                max_lots: 5,
                max_quote_debit_per_lot: 3,
                valid_until_slot: 100,
            },
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Placed,
                admitted_slot: 10,
                released_slot: 0,
            },
            |_| Ok((1, 2)),
            &mut order_bytes,
        )
        .expect("canonical order");
        let order = GeneralOrderV1::decode(&order_bytes).expect("order");
        let mut signed_terms =
            vec![0_u8; general_signed_order_terms_len_v1(1).expect("signed width")];
        order
            .encode_signed_terms_into(&mut signed_terms)
            .expect("signed terms");
        (root, batch, order_bytes, signed_terms)
    }

    struct VerifyRequestFixture {
        root: GeneralRootV2,
        batch: GeneralBatchV1,
        orders: Vec<Vec<u8>>,
        candidate: Vec<u8>,
        page: Vec<u8>,
        submission: GeneralCandidateV1,
    }

    fn verify_request_fixture() -> VerifyRequestFixture {
        const WIDTH: u32 = 3;
        const PRICE_SCALE: u64 = 100;
        let identity = |low: u8| {
            let mut value = [0_u8; 32];
            value[0] = low;
            value
        };
        let market = identity(1);
        let config_id = identity(2);
        let product_id = identity(3);
        let mut root = GeneralRootV2::active(market, config_id, 7).expect("active root");
        let revision = root.revision();
        let mut batch = GeneralBatchV1::open(
            &mut root,
            GeneralBatchOpeningV1 {
                outcome_count: WIDTH,
                sequence: 0,
                generation: 7,
                market,
                product_id,
                config_id,
                price_scale: PRICE_SCALE,
                collection_close_slot: 1_000,
                settlement_close_slot: 2_000,
                max_orders: 4,
            },
            revision,
            10,
        )
        .expect("open batch");
        let mut place = |owner: u8, nonce: u64, receive: &[u64], deliver: &[u64]| {
            let mut bytes = vec![0_u8; general_order_len_v1(WIDTH).expect("order width")];
            GeneralOrderV1::encode_into(
                GeneralOrderHeaderV1 {
                    outcome_count: WIDTH,
                    nonce,
                    owner_id: identity(owner),
                    market,
                    batch_id: batch.batch_id(),
                    generation: 7,
                    max_lots: 10,
                    max_quote_debit_per_lot: 5,
                    valid_until_slot: 2_000,
                },
                receive,
                deliver,
                GeneralOrderStateV1 {
                    phase: GeneralOrderPhaseV1::Placed,
                    admitted_slot: 10,
                    released_slot: 0,
                },
                &mut bytes,
            )
            .expect("order record");
            let order = GeneralOrderV1::decode(&bytes).expect("order");
            let claims = (0..WIDTH)
                .map(|index| order.claim_reserve(index).expect("claim reserve"))
                .collect::<Vec<_>>();
            batch
                .admit(
                    order,
                    MakerFundingV1 {
                        owner_id: identity(owner),
                        available_quote: 1_000,
                        available_claims: &claims,
                    },
                    10,
                )
                .expect("escrow order");
            bytes
        };
        let first = place(9, 1, &[1, 0, 0], &[0, 1, 0]);
        let second = place(8, 2, &[0, 1, 0], &[1, 0, 0]);
        let revision = root.revision();
        batch.close(&mut root, revision).expect("close batch");

        let mut candidate = vec![0_u8; candidate_len(WIDTH).expect("candidate width")];
        let candidate_header = CandidateHeaderV2 {
            outcome_count: WIDTH,
            page_count: 1,
            candidate_coordinate: 1,
            price_scale: PRICE_SCALE,
            candidate_id: identity(0xff),
            product_id,
            batch_id: batch.batch_id(),
        };
        CandidateV2::encode_into(candidate_header, &[40, 60, 0], &mut candidate)
            .expect("draft candidate");
        let candidate_id = general_candidate_identity_v1(&candidate).expect("candidate identity");
        CandidateV2::encode_into(
            CandidateHeaderV2 {
                candidate_id,
                ..candidate_header
            },
            &[40, 60, 0],
            &mut candidate,
        )
        .expect("addressed candidate");

        let mut orders = vec![first, second];
        orders.sort_by(|left, right| {
            let left = GeneralOrderV1::decode(left).expect("left order").order_id();
            let right = GeneralOrderV1::decode(right).expect("right order").order_id();
            if left == right {
                core::cmp::Ordering::Equal
            } else if dclutch_general_adapter_contract::runtime_verify::runtime_identity_precedes_v2(
                &left, &right,
            ) {
                core::cmp::Ordering::Less
            } else {
                core::cmp::Ordering::Greater
            }
        });
        let rows = orders
            .iter()
            .enumerate()
            .map(|(index, bytes)| {
                let order = GeneralOrderV1::decode(bytes).expect("order");
                let header = order.header();
                let receive = (0..WIDTH)
                    .map(|outcome| order.receive_per_lot(outcome).expect("receive"))
                    .collect::<Vec<_>>();
                let deliver = (0..WIDTH)
                    .map(|outcome| order.deliver_per_lot(outcome).expect("deliver"))
                    .collect::<Vec<_>>();
                let mut row = vec![0_u8; execution_len(WIDTH).expect("execution width")];
                ExecutionV2::encode_into(
                    ExecutionHeaderV2 {
                        outcome_count: WIDTH,
                        page_coordinate: 1,
                        execution_coordinate: u32::try_from(index).expect("row index") + 1,
                        nonce: header.nonce,
                        order_id: order.order_id(),
                        owner_id: header.owner_id,
                        max_lots: header.max_lots,
                        lots: 4,
                    },
                    &receive,
                    &deliver,
                    &mut row,
                )
                .expect("execution row");
                row
            })
            .collect::<Vec<_>>();
        let row_refs = rows.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let mut page = vec![0_u8; page_len(WIDTH, 2).expect("page width")];
        PageV2::encode_into(
            PageHeaderV2 {
                outcome_count: WIDTH,
                page_coordinate: 1,
                page_count: 1,
                revision: 9,
                candidate_id,
            },
            &row_refs,
            &mut page,
        )
        .expect("candidate page");
        let submission = GeneralCandidateV1::submit(
            batch,
            CandidateV2::decode(&candidate).expect("candidate"),
            9,
            2,
            5,
            identity(40),
            20,
            1_100,
        )
        .expect("submit candidate");
        VerifyRequestFixture {
            root,
            batch,
            orders,
            candidate,
            page,
            submission,
        }
    }

    fn verify_local_state(
        kind: GeneralLocalStateKindV3,
        outcome_count: u32,
        body: &[u8],
    ) -> Vec<u8> {
        let width = general_local_state_len_v3(kind, outcome_count).expect("local-state width");
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0_u8; width];
        encode_general_local_state_v3_atomic(
            GeneralLocalStateHeaderV3 {
                kind,
                bump: 7,
                rent_principal: 1_000,
                beneficiary: [0x71; 32],
            },
            body,
            &mut scratch,
            &mut output,
        )
        .expect("canonical local state");
        output
    }

    fn verify_request_state(
        fixture: &VerifyRequestFixture,
        submission: GeneralCandidateV1,
        cursor_before: &[u8],
        row_index: u32,
    ) -> (GeneralHotStateV3, GeneralCandidateV1, Vec<u8>) {
        const WIDTH: u32 = 3;
        let cursor_len = candidate_verifier_len_v1(submission).expect("cursor width");
        let certificate_len = candidate_certificate_len_v1(submission).expect("result width");
        let vacant_cursor = vec![0_u8; cursor_len];
        let cursor_input = if cursor_before.is_empty() {
            vacant_cursor.as_slice()
        } else {
            cursor_before
        };
        let verified_before = vec![0_u8; certificate_len];
        let view = CandidateVerifyRowViewV1 {
            batch: fixture.batch,
            submission,
            candidate: &fixture.candidate,
            page: &fixture.page,
            order: &fixture.orders[usize::try_from(row_index).expect("row index")],
            cursor_before: cursor_input,
            verified_before: &verified_before,
            expected_page_index: 0,
            expected_row_index: row_index,
            expected_revision: u64::from(row_index),
        };
        let manifest_orders = candidate_verify_manifest_orders_v1(&view).expect("manifest count");
        let manifest_len = settlement_manifest_len_v2(WIDTH, manifest_orders).expect("manifest");
        let mut cursor_scratch = vec![0_u8; cursor_len];
        let mut cursor_output = vec![0_u8; cursor_len];
        let mut verified_scratch = vec![0_u8; certificate_len];
        let mut verified_output = vec![0_u8; certificate_len];
        let mut manifest_scratch = vec![0_u8; manifest_len];
        let mut manifest_output = vec![0_u8; manifest_len];
        let summary = verify_candidate_row_v1(
            view,
            CandidateVerifyRowBuffersV1 {
                cursor_scratch: &mut cursor_scratch,
                cursor_output: &mut cursor_output,
                verified_scratch: &mut verified_scratch,
                verified_output: &mut verified_output,
                manifest_scratch: &mut manifest_scratch,
                manifest_output: &mut manifest_output,
            },
        )
        .expect("verification step");
        let observed = |value: u8, data: Vec<u8>| GeneralObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key: key(value),
                owner: key(200),
                lamports: 1_000_000,
                executable: false,
                data,
            },
            is_signer: false,
            is_writable: false,
        };
        let mut runtime_suffix_accounts = vec![
            observed(
                0x80,
                verify_local_state(
                    GeneralLocalStateKindV3::Candidate,
                    WIDTH,
                    &submission.to_bytes(),
                ),
            ),
            observed(
                0x81,
                if cursor_before.is_empty() {
                    Vec::new()
                } else {
                    verify_local_state(GeneralLocalStateKindV3::Verifier, WIDTH, cursor_before)
                },
            ),
            observed(0x82, Vec::new()),
            observed(0x83, Vec::new()),
            observed(0x84, Vec::new()),
            observed(
                0x85,
                verify_local_state(
                    GeneralLocalStateKindV3::Batch,
                    WIDTH,
                    &fixture.batch.to_bytes(),
                ),
            ),
            observed(0x86, fixture.candidate.clone()),
            observed(0x87, fixture.page.clone()),
            observed(
                0x88,
                fixture.orders[usize::try_from(row_index).expect("row index")].clone(),
            ),
            observed(0x89, manifest_output),
        ];
        for account in runtime_suffix_accounts.iter_mut().take(5) {
            account.is_writable = true;
        }
        runtime_suffix_accounts[3].is_signer = true;
        (
            GeneralHotStateV3 {
                fixed_accounts: Vec::new(),
                strategy_accounts: Vec::new(),
                runtime_suffix_accounts,
                release_set: [1; 32],
                generation: 7,
                minimum_finalized_slot: observation().slot,
                checked_release: None,
            },
            summary.submission,
            cursor_output,
        )
    }

    fn submit_request_config() -> GeneralConfigV3 {
        submit_request_config_with_max_orders(4)
    }

    fn submit_request_config_with_max_orders(max_orders_per_candidate: u32) -> GeneralConfigV3 {
        GeneralConfigV3::new(GeneralConfigV3Input {
            capacity_profile_id: [0x51; 32],
            claim_basis_id: [0x52; 32],
            program_set_id: [0x53; 32],
            generation: 7,
            price_scale: 100,
            collection_slots: 1_000,
            selection_slots: 400,
            settlement_slots: 600,
            max_orders_per_candidate,
            max_pages_per_candidate: 10,
            continuation_reward_lamports: 5,
            selection_policy_id: [0x54; 32],
            quote_surplus_beneficiary: [0x55; 32],
        })
        .expect("Submit config")
    }

    fn submit_request_state(fixture: &VerifyRequestFixture) -> GeneralHotStateV3 {
        const WIDTH: u32 = 3;
        let observed = |account_key: Pubkey, data: Vec<u8>| GeneralObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key: account_key,
                owner: key(200),
                lamports: 1_000_000,
                executable: false,
                data,
            },
            is_signer: false,
            is_writable: false,
        };
        let mut runtime_suffix_accounts = vec![
            observed(key(0x80), Vec::new()),
            observed(
                Pubkey::new_from_array(fixture.submission.opening().solver_id),
                Vec::new(),
            ),
            observed(key(0x82), Vec::new()),
            observed(
                key(0x83),
                verify_local_state(
                    GeneralLocalStateKindV3::Batch,
                    WIDTH,
                    &fixture.batch.to_bytes(),
                ),
            ),
            observed(key(0x84), fixture.candidate.clone()),
            observed(key(0x85), fixture.submission.to_bytes().to_vec()),
        ];
        runtime_suffix_accounts[0].is_writable = true;
        runtime_suffix_accounts[1].is_writable = true;
        runtime_suffix_accounts[1].is_signer = true;
        runtime_suffix_accounts[2].is_writable = true;
        GeneralHotStateV3 {
            fixed_accounts: Vec::new(),
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts,
            release_set: [1; 32],
            generation: 7,
            minimum_finalized_slot: observation().slot,
            checked_release: None,
        }
    }

    fn front_config(generation: u64, price_scale: u64, max_orders: u32) -> GeneralConfigV3 {
        GeneralConfigV3::new(GeneralConfigV3Input {
            capacity_profile_id: [0x51; 32],
            claim_basis_id: [0x52; 32],
            program_set_id: [0x53; 32],
            generation,
            price_scale,
            collection_slots: 40,
            selection_slots: 50,
            settlement_slots: 60,
            max_orders_per_candidate: max_orders,
            max_pages_per_candidate: 10,
            continuation_reward_lamports: 1,
            selection_policy_id: [0x54; 32],
            quote_surplus_beneficiary: [0x55; 32],
        })
        .expect("front config")
    }

    #[test]
    fn open_request_derives_the_slot_independent_batch_occurrence() {
        let market = [0x41; 32];
        let config_id = [0x42; 32];
        let product_id = [0x43; 32];
        let root = GeneralRootV2::active(market, config_id, 7).expect("active root");
        let config = front_config(7, 11, 19);
        let request = derive_open_request_from_root_v5(258, config_id, config, product_id, root)
            .expect("pre-executable open request");
        let expected = GeneralBatchOccurrenceTermsV1::new(GeneralBatchOpeningV1 {
            outcome_count: 258,
            sequence: root.next_batch_sequence(),
            generation: root.generation(),
            market,
            product_id,
            config_id,
            price_scale: 11,
            collection_close_slot: 900,
            settlement_close_slot: 1_200,
            max_orders: 19,
        })
        .expect("occurrence terms")
        .occurrence_id();

        assert_eq!(request.wire, GeneralRequestWireV3::V3);
        assert_eq!(request.action, Action::OpenBatch);
        assert_eq!(request.candidate_id, Some(expected));
        assert_eq!(request.expected_revision, root.revision());
        assert_eq!(
            decode_general_request_v3(&request.to_bytes().expect("V3 request"))
                .expect("hostile decode"),
            request
        );

        assert_eq!(
            derive_open_request_from_root_v5(
                258,
                config_id,
                front_config(8, 11, 19),
                product_id,
                root,
            ),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );
        assert_eq!(
            derive_open_request_from_root_v5(258, [0x99; 32], config, product_id, root),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );
    }

    #[test]
    fn front_requests_are_chain_derived_v3_wires_for_every_executable_action() {
        let (root, batch, order_bytes, signed_terms) = front_records();
        let config_id = root.config_id();
        let order_id = GeneralOrderV1::decode(&order_bytes)
            .expect("order")
            .order_id();
        let cases = [
            (
                Action::CloseBatch,
                front_state(
                    front_local_state(GeneralLocalStateKindV3::Batch, &batch.to_bytes()),
                    None,
                    None,
                ),
                batch.batch_id(),
                root.revision(),
            ),
            (
                Action::PlaceOrder,
                front_state(
                    front_local_state(GeneralLocalStateKindV3::Batch, &batch.to_bytes()),
                    None,
                    Some(signed_terms),
                ),
                order_id,
                0,
            ),
            (
                Action::CancelOrder,
                front_state(
                    front_local_state(GeneralLocalStateKindV3::Batch, &batch.to_bytes()),
                    Some(front_local_state(
                        GeneralLocalStateKindV3::Order,
                        &order_bytes,
                    )),
                    None,
                ),
                order_id,
                0,
            ),
            (
                Action::ReleaseOrder,
                front_state(
                    front_local_state(GeneralLocalStateKindV3::Order, &order_bytes),
                    None,
                    None,
                ),
                order_id,
                0,
            ),
        ];
        for (action, state, subject, expected_revision) in cases {
            let request = derive_front_request_from_root_v5(&state, action, 1, config_id, root)
                .expect("chain-derived front request");
            assert_eq!(request.wire, GeneralRequestWireV3::V3);
            assert_eq!(request.action, action);
            assert_eq!(request.candidate_id, Some(subject));
            assert_eq!(request.expected_revision, expected_revision);
            let bytes = request.to_bytes().expect("V3 wire");
            assert_eq!(
                decode_general_request_v3(&bytes).expect("hostile decode"),
                request
            );
            assert!(ControllerRequestV2::decode(&bytes).is_err());

            let mut substituted_generation = bytes;
            substituted_generation[7] = b'2';
            assert!(decode_general_request_v3(&substituted_generation).is_err());
            let mut unauthorized_result_state = bytes;
            unauthorized_result_state[63] = 1;
            assert!(decode_general_request_v3(&unauthorized_result_state).is_err());
        }
    }

    #[test]
    fn close_candidate_request_is_chain_derived_from_candidate_and_closed_batch() {
        let fixture = verify_request_fixture();
        let solver = Pubkey::new_from_array(fixture.submission.opening().solver_id);
        let observed = |account_key: Pubkey, data: Vec<u8>| GeneralObservedAccountMetaV3 {
            account: ObservedAccount {
                observation: observation(),
                key: account_key,
                owner: key(200),
                lamports: 1_000_000,
                executable: false,
                data,
            },
            is_signer: false,
            is_writable: false,
        };
        let mut runtime_suffix_accounts = vec![
            observed(
                key(0x80),
                verify_local_state(
                    GeneralLocalStateKindV3::Candidate,
                    3,
                    &fixture.submission.to_bytes(),
                ),
            ),
            observed(key(0x81), Vec::new()),
            observed(solver, Vec::new()),
            observed(
                key(0x83),
                verify_local_state(GeneralLocalStateKindV3::Batch, 3, &fixture.batch.to_bytes()),
            ),
        ];
        runtime_suffix_accounts[0].is_writable = true;
        runtime_suffix_accounts[1].is_signer = true;
        runtime_suffix_accounts[1].is_writable = true;
        runtime_suffix_accounts[2].is_writable = true;
        let state = GeneralHotStateV3 {
            fixed_accounts: Vec::new(),
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts,
            release_set: [1; 32],
            generation: 7,
            minimum_finalized_slot: observation().slot,
            checked_release: None,
        };
        let request = derive_front_request_from_root_v5(
            &state,
            Action::CloseCandidate,
            3,
            fixture.root.config_id(),
            fixture.root,
        )
        .expect("chain-derived CloseCandidate request");
        assert_eq!(request.wire, GeneralRequestWireV3::V3);
        assert_eq!(request.action, Action::CloseCandidate);
        assert_eq!(
            request.candidate_id,
            Some(fixture.submission.opening().candidate_id),
        );
        assert_eq!(request.expected_revision, 0);
        assert_eq!(request.page_index, 0);
        assert_eq!(request.execution_index, 0);
        assert_eq!(request.manifest_order_index, 0);
        assert_eq!(request.state_bump, 0);
        assert_eq!(request.terminal_record_bump, 0);
        assert_eq!(request.result_state_bump, 0);

        let mut unsigned = state.clone();
        unsigned.runtime_suffix_accounts[1].is_signer = false;
        assert_eq!(
            derive_front_request_from_root_v5(
                &unsigned,
                Action::CloseCandidate,
                3,
                fixture.root.config_id(),
                fixture.root,
            ),
            Err(GeneralHotOperatorErrorV3::ChainState),
        );

        let mut substituted_solver = state;
        substituted_solver.runtime_suffix_accounts[2].account.key = key(0xee);
        assert_eq!(
            derive_front_request_from_root_v5(
                &substituted_solver,
                Action::CloseCandidate,
                3,
                fixture.root.config_id(),
                fixture.root,
            ),
            Err(GeneralHotOperatorErrorV3::ChainState),
        );
    }

    #[test]
    fn front_request_derivation_refuses_cross_batch_signed_terms() {
        let (root, batch, _, _) = front_records();
        let mut hostile_order = vec![0_u8; general_order_len_v1(1).expect("order width")];
        GeneralOrderV1::encode_rows_into(
            GeneralOrderHeaderV1 {
                outcome_count: 1,
                nonce: 10,
                owner_id: [0x44; 32],
                market: root.market(),
                batch_id: [0x99; 32],
                generation: root.generation(),
                max_lots: 5,
                max_quote_debit_per_lot: 3,
                valid_until_slot: 100,
            },
            GeneralOrderStateV1 {
                phase: GeneralOrderPhaseV1::Placed,
                admitted_slot: 10,
                released_slot: 0,
            },
            |_| Ok((1, 2)),
            &mut hostile_order,
        )
        .expect("hostile canonical order");
        let hostile = GeneralOrderV1::decode(&hostile_order).expect("hostile order");
        let mut signed_terms =
            vec![0_u8; general_signed_order_terms_len_v1(1).expect("signed width")];
        hostile
            .encode_signed_terms_into(&mut signed_terms)
            .expect("hostile signed terms");
        let state = front_state(
            front_local_state(GeneralLocalStateKindV3::Batch, &batch.to_bytes()),
            None,
            Some(signed_terms),
        );
        assert_eq!(
            derive_front_request_from_root_v5(
                &state,
                Action::PlaceOrder,
                1,
                root.config_id(),
                root,
            ),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );
    }

    #[test]
    fn submit_request_is_derived_from_the_exact_signed_candidate_evidence() {
        let fixture = verify_request_fixture();
        let state = submit_request_state(&fixture);
        let request = derive_submit_request_from_root_v5(
            &state,
            3,
            fixture.root.config_id(),
            submit_request_config(),
            fixture.batch.opening().product_id,
            fixture.root,
        )
        .expect("chain-derived SubmitCandidate request");
        assert_eq!(request.wire, GeneralRequestWireV3::V3);
        assert_eq!(request.action, Action::SubmitCandidate);
        assert_eq!(
            request.candidate_id,
            Some(fixture.submission.opening().candidate_id)
        );
        assert_eq!(request.expected_revision, 0);
        assert_eq!(request.page_index, 0);
        assert_eq!(request.execution_index, 0);
        assert_eq!(request.manifest_order_index, 0);
        assert_eq!(request.state_bump, 0);
        assert_eq!(request.terminal_record_bump, 0);
        assert_eq!(request.result_state_bump, 0);
        let bytes = request.to_bytes().expect("canonical V3 request");
        assert_eq!(
            decode_general_request_v3(&bytes).expect("hostile decode"),
            request
        );
        assert!(ControllerRequestV2::decode(&bytes).is_err());
    }

    #[test]
    fn submit_request_refuses_materialized_state_and_substituted_evidence() {
        let fixture = verify_request_fixture();
        let config = submit_request_config();
        let derive = |state: &GeneralHotStateV3, config: GeneralConfigV3| {
            derive_submit_request_from_root_v5(
                state,
                3,
                fixture.root.config_id(),
                config,
                fixture.batch.opening().product_id,
                fixture.root,
            )
        };
        let state = submit_request_state(&fixture);

        let mut materialized = state.clone();
        materialized.runtime_suffix_accounts[0].account.data = verify_local_state(
            GeneralLocalStateKindV3::Candidate,
            3,
            &fixture.submission.to_bytes(),
        );
        assert_eq!(
            derive(&materialized, config),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );

        let mut wrong_solver = state.clone();
        wrong_solver.runtime_suffix_accounts[1].account.key = key(0xee);
        assert_eq!(
            derive(&wrong_solver, config),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );

        let mut writable_candidate = state.clone();
        writable_candidate.runtime_suffix_accounts[4].is_writable = true;
        assert_eq!(
            derive(&writable_candidate, config),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );

        let mut unwrapped_batch = state.clone();
        unwrapped_batch.runtime_suffix_accounts[3].account.data = fixture.batch.to_bytes().to_vec();
        assert_eq!(
            derive(&unwrapped_batch, config),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );

        let mut substituted_candidate = state.clone();
        *substituted_candidate.runtime_suffix_accounts[4]
            .account
            .data
            .last_mut()
            .expect("candidate byte") ^= 1;
        assert_eq!(
            derive(&substituted_candidate, config),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );

        assert_eq!(
            derive(&state, submit_request_config_with_max_orders(5)),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );
    }

    #[test]
    fn verify_request_is_derived_by_replaying_the_exact_next_row() {
        let fixture = verify_request_fixture();
        let (first_state, successor_submission, successor_cursor) =
            verify_request_state(&fixture, fixture.submission, &[], 0);
        let first = derive_verify_request_from_root_v5(
            &first_state,
            3,
            fixture.root.config_id(),
            fixture.batch.opening().product_id,
            fixture.root,
        )
        .expect("first chain-derived Verify request");
        assert_eq!(first.wire, GeneralRequestWireV3::V3);
        assert_eq!(first.action, Action::VerifyCandidateRow);
        assert_eq!(
            first.candidate_id,
            Some(fixture.submission.opening().candidate_id)
        );
        assert_eq!(first.expected_revision, 0);
        assert_eq!(first.page_index, 0);
        assert_eq!(first.execution_index, 0);
        assert_eq!(first.manifest_order_index, 0);
        assert_eq!(first.state_bump, 0);
        assert_eq!(first.terminal_record_bump, 0);
        assert_eq!(first.result_state_bump, 0);
        let bytes = first.to_bytes().expect("V3 request bytes");
        assert_eq!(
            decode_general_request_v3(&bytes).expect("hostile decode"),
            first
        );
        assert!(ControllerRequestV2::decode(&bytes).is_err());

        let (second_state, _, _) =
            verify_request_state(&fixture, successor_submission, &successor_cursor, 1);
        let second = derive_verify_request_from_root_v5(
            &second_state,
            3,
            fixture.root.config_id(),
            fixture.batch.opening().product_id,
            fixture.root,
        )
        .expect("successor chain-derived Verify request");
        assert_eq!(second.expected_revision, 1);
        assert_eq!(second.page_index, 0);
        assert_eq!(second.execution_index, 1);
        assert_eq!(second.candidate_id, first.candidate_id);
    }

    #[test]
    fn verify_request_refuses_substituted_evidence_before_emitting_a_wire() {
        let fixture = verify_request_fixture();
        let (state, _, _) = verify_request_state(&fixture, fixture.submission, &[], 0);
        let derive = |state: &GeneralHotStateV3| {
            derive_verify_request_from_root_v5(
                state,
                3,
                fixture.root.config_id(),
                fixture.batch.opening().product_id,
                fixture.root,
            )
        };

        let mut substituted_manifest = state.clone();
        *substituted_manifest.runtime_suffix_accounts[9]
            .account
            .data
            .last_mut()
            .expect("manifest byte") ^= 1;
        assert_eq!(
            derive(&substituted_manifest),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );

        let mut substituted_page = state.clone();
        substituted_page.runtime_suffix_accounts[7].account.data[24] ^= 1;
        assert_eq!(
            derive(&substituted_page),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );

        assert_eq!(
            derive_verify_request_from_root_v5(
                &state,
                3,
                fixture.root.config_id(),
                [0x99; 32],
                fixture.root,
            ),
            Err(GeneralHotOperatorErrorV3::ChainState)
        );
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
        output
            .get_mut(0..8)
            .expect("manifest offset in bounds")
            .copy_from_slice(b"DCGMAN02");
        output
            .get_mut(8..10)
            .expect("manifest offset in bounds")
            .copy_from_slice(&2_u16.to_le_bytes());
        *output.get_mut(10).expect("manifest offset in bounds") = 11;
        output
            .get_mut(12..16)
            .expect("manifest offset in bounds")
            .copy_from_slice(&width.to_le_bytes());
        output
            .get_mut(16..20)
            .expect("manifest offset in bounds")
            .copy_from_slice(&order_count.to_le_bytes());
        output
            .get_mut(20..24)
            .expect("manifest offset in bounds")
            .copy_from_slice(&7_u32.to_le_bytes());
        output
            .get_mut(24..32)
            .expect("manifest offset in bounds")
            .copy_from_slice(&2_u64.to_le_bytes());
        output
            .get_mut(32..64)
            .expect("manifest offset in bounds")
            .copy_from_slice(&[51; 32]);
        let row_bytes =
            dclutch_general_adapter_contract::runtime_manifest::settlement_order_len_v2(width)
                .expect("order width");
        for (ordinal, (order_coordinate, source_page_index, source_execution_index)) in
            rows.iter().copied().enumerate()
        {
            let row = 64 + ordinal * row_bytes;
            output
                .get_mut(row..row + 8)
                .expect("manifest offset in bounds")
                .copy_from_slice(b"DCGORD02");
            output
                .get_mut(row + 8..row + 10)
                .expect("manifest offset in bounds")
                .copy_from_slice(&2_u16.to_le_bytes());
            *output.get_mut(row + 10).expect("manifest offset in bounds") = 12;
            output
                .get_mut(row + 12..row + 16)
                .expect("manifest offset in bounds")
                .copy_from_slice(&width.to_le_bytes());
            output
                .get_mut(row + 16..row + 20)
                .expect("manifest offset in bounds")
                .copy_from_slice(&order_coordinate.to_le_bytes());
            output
                .get_mut(row + 20..row + 24)
                .expect("manifest offset in bounds")
                .copy_from_slice(&source_page_index.to_le_bytes());
            output
                .get_mut(row + 24..row + 32)
                .expect("manifest offset in bounds")
                .copy_from_slice(&9_u64.to_le_bytes());
            output
                .get_mut(row + 32..row + 64)
                .expect("manifest offset in bounds")
                .copy_from_slice(&[51; 32]);
            let order_byte = u8::try_from(order_coordinate).expect("order identity");
            output
                .get_mut(row + 64..row + 96)
                .expect("manifest offset in bounds")
                .copy_from_slice(&[order_byte; 32]);
            output
                .get_mut(row + 96..row + 128)
                .expect("manifest offset in bounds")
                .copy_from_slice(&[72; 32]);
            output
                .get_mut(row + 128..row + 136)
                .expect("manifest offset in bounds")
                .copy_from_slice(&3_u64.to_le_bytes());
            output
                .get_mut(row + 136..row + 144)
                .expect("manifest offset in bounds")
                .copy_from_slice(&3_u64.to_le_bytes());
            output
                .get_mut(row + 152..row + 156)
                .expect("manifest offset in bounds")
                .copy_from_slice(&source_execution_index.to_le_bytes());
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
            assert_eq!(plan.heap_frame_bytes, GENERAL_HOT_HEAP_FRAME_BYTES_V3);
            let solana_message::VersionedMessage::V0(message) = &plan.message.message else {
                panic!("General compiles only v0 messages");
            };
            assert_eq!(message.instructions.len(), 2);
            let heap =
                ComputeBudgetInstruction::request_heap_frame(GENERAL_HOT_HEAP_FRAME_BYTES_V3);
            let heap_compiled = message.instructions.first().expect("leading heap frame");
            assert_eq!(
                message
                    .account_keys
                    .get(usize::from(heap_compiled.program_id_index)),
                Some(&heap.program_id)
            );
            assert_eq!(heap_compiled.data, heap.data);
            assert!(heap_compiled.accounts.is_empty());
            let hot_compiled = message.instructions.get(1).expect("trailing Trading Hot");
            assert_eq!(
                message
                    .account_keys
                    .get(usize::from(hot_compiled.program_id_index)),
                Some(&report.instruction.program_id)
            );
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
    ///
    /// The GEN-SEVEN register-bank widening moved the common bank from 90/40
    /// to 151/45 scalar/identity coordinates. The counts and wires below were
    /// re-measured against the widened real-ELF campaign.
    ///
    /// MINUS EIGHTEEN ACCOUNTS AND THIRTY-SIX WIRE BYTES ON EVERY ROW, from the
    /// input bank going inline. At N=258 the bank was eighteen scratch pages;
    /// it is now eighteen fewer accounts and, at two wire bytes for each
    /// ALT-backed readonly account, thirty-six fewer bytes. The caller-authority
    /// span did not move -- it counts accelerator invocations, which the output
    /// still chunks -- so the delta is uniform across all seven actions, and
    /// that uniformity is what says this removed a count and nothing else.
    #[test]
    fn every_action_is_alt_packet_safe_at_the_canonical_runtime_width() {
        let payer = key(250);
        let blockhash = Hash::new_from_array([16; 32]);
        // +1 ACCOUNT AND +2 WIRE BYTES ON EVERY ROW, from `e3298c9a` appending
        // the System program to General's account profile. The account cost is
        // one coordinate; the wire cost is two bytes because an ALT-backed v0
        // message names a readonly account TWICE -- once in the table's
        // readonly-index array and once in the instruction's own account-index
        // array -- and both are one byte. Uniform across all seven, which is
        // what says the append moved a count and nothing else.
        // FREEZE MOVED ALONE on 2026-09-04 and the other six did not, which is
        // the opposite of the uniformity above and says the same kind of thing:
        // `9653ef363` gave `Freeze` the closed Batch as its ONE readonly
        // evidence account, so exactly the action that gained an account gained
        // a coordinate and two wire bytes. Nothing else in the seven declares a
        // new record.
        for (action, accounts, wire) in [
            (Action::Consider, 71, 674),
            (Action::Freeze, 70, 672),
            (Action::InitializeSettlement, 105, 932),
            (Action::Collect, 99, 825),
            (Action::Materialize, 97, 821),
            (Action::Distribute, 99, 825),
            (Action::Close, 98, 823),
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
    ///
    /// If you are tempted to edit a number below to make this green, re-run the
    /// campaign instead and move the evidence with it. RE-RUN 2026-09-02
    /// against the real ELF at HEAD -- `real_sbf` in
    /// `programs/dclutch-general-accelerator-sbf/program-test/tests/lifecycle.rs`,
    /// which derives the same frame and asserts it against the ELF -- and every
    /// one of the seven moved by exactly +31 from the `b92b2cee` measurement:
    ///
    /// - +30 from `68f7c849`, which found that `admitted_v3.rs` described an
    ///   eighteen-account CPI frame nothing had ever produced and corrected
    ///   `ADMITTED_RUNTIME_ACCOUNTS_START_V3` from 18 to 48. The campaign
    ///   numbers were pinned before that correction, so this control has been
    ///   asserting a frame the code stopped building on 2026-09-01.
    /// - +1 from `e3298c9a`, which appended the System program to General's
    ///   account profile.
    ///
    /// The uniformity is the evidence that those are the only two causes: a
    /// third would not have moved all seven by the same amount.
    #[test]
    fn the_derived_geometry_reproduces_the_executed_campaign_frame() {
        for (action, campaign_accounts) in [
            (Action::Consider, 61),
            // +1 on 2026-09-04 from `9653ef363`, and on this row alone: the
            // freeze deadline needed a batch to read a collection close out of,
            // so `Freeze` went from zero readonly evidence accounts to one.
            // The real-ELF authority for it is `real_sbf` in the accelerator's
            // `lifecycle.rs`, which derives this frame and asserts it against
            // the ELF; it moved with the profile and this copy did not.
            (Action::Freeze, 60),
            (Action::InitializeSettlement, 118),
            (Action::Collect, 98),
            (Action::Materialize, 96),
            (Action::Distribute, 98),
            (Action::Close, 115),
        ] {
            // THE FIXED FRAME AND NOTHING ELSE. This used to add the input
            // page span, and every row above is eighteen smaller because that
            // span is gone at N = 258. The caller-authority span is not a
            // logical account -- it lives in the strategy extras, which
            // `ADMITTED_RUNTIME_ACCOUNTS_START_V3` already covers.
            let logical =
                usize::from(general_account_profile_fixed_count_v3(action).expect("logical count"));
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
                usize::from(general_account_profile_fixed_count_v3(action).expect("count")),
                geometry.physical_runtime,
                "{action:?}"
            );
        }
    }

    /// N=1 and N=258 differ only by the caller-authority span, and both fit.
    ///
    /// It used to be twice that span, because each accelerator invocation cost
    /// a caller authority AND an input scratch page and the two counts were the
    /// same number. The input bank is inline now, so the width moves by exactly
    /// one account per invocation.
    #[test]
    fn the_runtime_width_moves_only_the_caller_authority_span() {
        for action in GENERAL_ACTIONS_V3 {
            let narrow = general_frame_geometry_v3(action, 1);
            let wide = general_frame_geometry_v3(action, 258);
            assert_eq!(
                wide.accounts - narrow.accounts,
                wide.admitted_invocations - narrow.admitted_invocations,
                "{action:?} account width follows only the output invocation count"
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
        *substituted
            .get_mut(32)
            .expect("verified candidate byte exists") ^= 1;
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

    /// The two live geometry conjuncts, and the witness for why the one they
    /// replaced had to go.
    ///
    /// The stale form compared `general_child_account_start_v3` against literal
    /// 8/9 -- `general_readonly_evidence_start_v3`'s own table, copied into this
    /// operator. Children begin AFTER evidence, so that equality can hold only
    /// when an action declares no readonly evidence: SIX of the seven actions
    /// were refused outright, and nobody noticed because
    /// `build_general_hot_instruction_v3` had no caller to run it.
    ///
    /// SEVEN OF SEVEN SINCE 2026-09-04. `Freeze` was the one action left that
    /// declared none, and `9653ef363` gave it the closed Batch so its transition
    /// could read a collection close. The witness is stronger for it -- there is
    /// now no action the stale conjunct would have admitted -- and it is also
    /// the reason this count is asserted against the action list rather than
    /// written as a literal 6.
    ///
    /// Both replacements compare quantities with independent authors. (a) pins
    /// the literals to the thing they were always describing -- evidence begins
    /// at the fixed-prefix boundary -- so a drifted evidence table still fires
    /// them. (b) pins the EffectProgram's own route table against the evidence
    /// arithmetic, which is a different author, rather than restating
    /// `general_child_account_start_v3`'s definition back at itself.
    #[test]
    fn the_geometry_conjuncts_hold_for_every_action_and_the_stale_one_could_not() {
        let mut refused_by_the_stale_form = 0_usize;
        for action in GENERAL_ACTIONS_V3 {
            let expected_evidence_start = if action == Action::Close { 9 } else { 8 };
            assert_eq!(
                general_readonly_evidence_start_v3(action),
                expected_evidence_start,
                "{action:?} moved the fixed-prefix boundary",
            );

            let child_start = general_child_account_start_v3(action);
            if general_effect_route_count_v3(action) != 0 {
                assert_eq!(
                    general_effect_route_frame_v3(action, 0)
                        .expect("first route frame")
                        .account_start,
                    child_start,
                    "{action:?} put a gap or an overlap between evidence and children",
                );
            }

            // The reversion witness: the stale conjunct, evaluated.
            if child_start != expected_evidence_start {
                refused_by_the_stale_form += 1;
            }
        }
        assert_eq!(
            refused_by_the_stale_form,
            GENERAL_ACTIONS_V3.len(),
            "the stale check admits an action, so one of the seven declares no readonly evidence",
        );
    }

    /// The corpus this builder mines from reaches this frame's Market, root and
    /// Custody deployment, and not some other coordinate.
    ///
    /// The DERIVATION is `dclutch-hot-bump-miner-v1`'s and has its own tests;
    /// what is per-family, and what nothing tested before 2026-09-03, is which
    /// coordinate of THIS frame each fact is read from. Every other fixture in
    /// this file fills its Market and root accounts with constant bytes, so
    /// both decodes fail, every slot degrades to zero, and a corpus reading the
    /// wrong coordinate would emit exactly the same all-zero block as one
    /// reading the right one -- a disconnected instrument logging as silence.
    ///
    /// `hot_bump_corpus_fixture_v1` stages bodies that DO decode, and derives
    /// the three bumps from the seeds it built them from. Two authors: this
    /// side decodes those bodies and re-derives.
    #[test]
    fn the_mined_corpus_reads_this_frames_market_root_and_custody_deployment() {
        use crate::hot_bump_corpus_fixture_v1 as corpus;
        let state = GeneralHotStateV3 {
            fixed_accounts: corpus::fixed_frame()
                .into_iter()
                .map(|value| GeneralObservedAccountMetaV3 {
                    account: value.account,
                    is_signer: value.is_signer,
                    is_writable: value.is_writable,
                })
                .collect(),
            strategy_accounts: Vec::new(),
            runtime_suffix_accounts: Vec::new(),
            release_set: corpus::release_set_id(),
            generation: corpus::GENERATION,
            minimum_finalized_slot: 0,
            checked_release: None,
        };
        assert_eq!(
            general_hot_bump_hints_v3(&state, &corpus::trading_program())
                .expect("staged corpus mines"),
            corpus::expected_hints()
        );
    }
}
