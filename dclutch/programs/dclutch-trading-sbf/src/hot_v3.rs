//! Family-neutral Trading V3 hot execution boundary.
//!
//! This module owns the common physical interpreter path. It authenticates the
//! Market, immutable root selection, finalized artifact graph, and current
//! release programs before projecting any mutation. The first executable cut
//! accepts interpreted programs with local effects and no fixed-role route;
//! child routes remain fail-closed until their canonical producer receipts are
//! consumed here.

extern crate alloc;

use alloc::{boxed::Box, vec, vec::Vec};

use dclutch_vm::account_profile::{
    AccountObservationV1,
    lifecycle_v3::{
        AuthenticatedRentCreditV3, AuthenticatedRentMinimumV3, AuthenticatedRentQuoteV5,
        CloseStatePlanV3, CoordinateScopeV3, LifecycleContextV3, LifecycleOperationV3,
        LifecycleProtectedRegisterBuffersV3, LifecycleRefundSourceV3, LifecycleRegisterKindV3,
        LifecycleRegisterTargetV3, LifecycleRegistersV3, LifecycleRentQuoteBuffersV5,
        LifecycleSeedInputValueV3, PlannedObservationsV3, StateLifecyclePlanV3,
        StateLifecyclePolicyV5, ValidatedProfileJoinV3,
        plan_lifecycle_with_protected_outputs_atomic,
    },
    v2::{
        AccountPrestateV2, AccountProfileV2, ProjectionRegistersV2, RouteAccountPrivilegesV2,
        SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2, TrustedEnvironmentV2,
        project_atomic as project_accounts_atomic, project_dynamic_fixed_spans_atomic,
    },
    v3::{AccountProfileV3, SCHEMA_RELEASE_ID_V3 as ACCOUNT_PROFILE_SCHEMA_ID_V3},
};
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::capability_manifest::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_market::capability_program::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, Error as CapabilityProgramError,
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_CONFIG_STAGING_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3,
        HOT_CORE_PROGRAMDATA_ACCOUNT_V3, HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_DESCRIPTOR_STAGING_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
        HOT_EFFECT_STAGING_ACCOUNT_V3, HOT_EXECUTION_MAGIC_V3, HOT_FIXED_ACCOUNT_COUNT_V3,
        HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3, HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_LIFECYCLE_STAGING_ACCOUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        HOT_LINKED_BASIS_STAGING_ACCOUNT_V3, HOT_MANIFEST_RAW_ACCOUNT_V3,
        HOT_MANIFEST_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_STAGING_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PRODUCT_STAGING_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_STAGING_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
        HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
        HOT_RUNTIME_PRODUCT_COORDINATE_V3, HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3,
        HOT_STRATEGY_RAW_ACCOUNT_V3, HOT_STRATEGY_STAGING_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
        HOT_TRANSITION_RAW_ACCOUNT_V3, HOT_TRANSITION_STAGING_ACCOUNT_V3, HotExecutionEnvelopeV3,
        hot_bump_hint_v1, hot_frame_uses_sealed_execution_aliases_v3,
    },
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
    v4::{
        CAPABILITY_PROGRAM_V4_BYTES, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as PROGRAM_SCHEMA_ID_V4, SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    },
};
use dclutch_vm::capability_seal::{
    SealedArtifactV1, SealedDescriptorClosureV1, SealedRoleV1, SealedStaticOwnershipV1,
};
use dclutch_claims::frame_spec_v1::{
    ClaimsFrameRoleV1, SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1, SparseNativeTransferFrameSpecV1,
};
use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CUSTODY_BUMP_RELAY_BYTES_V1, CUSTODY_REPLAY_BYTES_V1, CustodyFrameRoleV1, CustodyFrameSpecV1,
    OperationV1, TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_trading::{
    direct_finalization_v3::{
        DIRECT_INLINE_POSTSTATE_COUNT_V3, DirectInlineAccountPrestateV3,
        DirectInlineAccountPrestatesV3, DirectInlineFinalizationInputV3,
        DirectInlineFinalizationProgramsV3, DirectInlineFinalizationWorkspaceV3,
        DirectInlinePoststateCommitmentV3,
        HOT_CHILD_EXECUTION_DIGEST_DOMAIN_V3 as CHILD_EXECUTION_DIGEST_DOMAIN_V3,
        HotExecutionAckInputV3, HotExecutionArtifactFactsV3,
        prepare_direct_inline_finalization_into_v3, project_hot_execution_ack_v3,
    },
    execution_v3::{
        DIRECT_SUCCESSOR_KIND_ID_V3, DirectExecutionActionV3, DirectExecutionRequestV3,
    },
    inline_candidate_v2::{
        DIRECT_INLINE_CUSTODY_EFFECT_CAPACITY_V2, DIRECT_INLINE_CUSTODY_ROUTE_SLOTS_V2,
        DirectExternalCollateralV2, DirectExternalDebitV2, DirectInlineCandidateContextV2,
        DirectInlineCollateralFrameV2, DirectInlineEffectDispatchV2,
    },
    ordinary_v3::{
        IDENTITY_BUYER_MAKER_ROOT_V3, IDENTITY_BUYER_TOKEN_ACCOUNT_V3,
        IDENTITY_CUSTODY_AUTHORITY_V3, IDENTITY_FEE_TOKEN_ACCOUNT_V3,
        IDENTITY_LINKED_BASIS_RECORD_V3, IDENTITY_MARKET_V3, IDENTITY_MINT_V3,
        IDENTITY_PRODUCT_RECORD_DIGEST_V3, IDENTITY_REALM_V3, IDENTITY_RELEASE_SET_V3,
        IDENTITY_SELLER_TOKEN_ACCOUNT_V3, IDENTITY_SEMANTIC_BASIS_V3, IDENTITY_TOKEN_PROGRAM_V3,
        IDENTITY_TRADING_PROGRAM_V3, SCALAR_BUYER_POSITION_REVISION_V3,
        SCALAR_CLAIMS_MARKET_REVISION_V3, SCALAR_CUSTODY_REVISION_V3, SCALAR_MARKET_GENERATION_V3,
        SCALAR_SELLER_POSITION_REVISION_V3, SCALAR_SLOT_V3,
    },
    successor::{
        AuthenticatedCompactIntentV2, DIRECT_ROOT_STATE_BYTES_V1, DirectExecutionConfigV1,
        DirectRootStateV1, InlineExecutionV2, InlineOrdinaryInputV2, InlineParticipantV2,
        MakerReplayFirstUseV1, MakerReplayObservationV1, MakerReplayRootV1, MakerReplayVacancyV1,
        RegisteredRecordFirstUseV2, register_intent_v2,
    },
};
use dclutch_vm::effect::{
    v2::{AccountInput, AccountPermission, FixedRole},
    v3::{ProgramV3 as EffectProgramV3, ProjectionV3, ResolvedEffectV3, RouteKindV3},
    v4::{
        ErrorV4 as EffectKernelErrorV4, ProgramV4 as EffectProgramV4, ResolvedWriteRangeV4,
        SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4,
        project_atomic_visiting as project_effects_v4_atomic_visiting,
    },
    v5::{
        FundingOperationV5, FundingSeedInputV5, MAX_ACTION_SEEDS_V5, ProgramV5 as EffectProgramV5,
        SCHEMA_RELEASE_ID_V5 as EFFECT_SCHEMA_ID_V5,
    },
};
use dclutch_market::execution_strategy::{
    admitted_v3::{
        AdmittedInvocationContextV3, AdmittedPreludeWitnessV1,
        admitted_invocation_context_digest_v3, admitted_prelude_witness_bytes_v1,
    },
    shadow_digest_v3::{
        AcceleratorCallerKindV1, BorrowedRuntimeObservationV3, ShadowEffectProjectionV3,
        ShadowInvocationContextV3, ShadowReceiptDependencyV3, ShadowResolvedRouteV3,
        ShadowRouteKindV3, ShadowRouteRoleV3, ShadowRuntimeObservationV3,
        accelerator_caller_authority_digest_v1, borrowed_runtime_observations_digest_in_v3,
        candidate_digest_v3, effect_digest_v3, family_request_digest_v3,
        invocation_context_digest_v3, receipt_dependencies_digest_v4,
        runtime_observations_digest_v3, runtime_observations_scratch_bytes_v3,
        runtime_observations_scratch_slices_v3,
    },
    shadow_v3::{
        ShadowArtifactTupleV3, ShadowExecutionDigestsV3, ShadowRequestV3, ShadowRuntimeShapeV3,
    },
    v2::{
        AcceleratorTransportProfileV2, AdmittedAcceleratorRequestV2, AuthenticatedScratchPageV2,
        BankTransportV2, EXECUTION_STRATEGY_PROGRAM_BYTES_V2, ExecutionStrategyProgramV2,
        RequestTransportV2, StrategyDispositionV2, classify_bank_transport_v2,
        register_bank_bytes_v2,
    },
};
use dclutch_market::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity,
    SERIES_UNALLOCATED_PERMIT_EXPIRY_REQUEST_BYTES_V1, STATE_BYTES, SeriesFoundingPermitSeedsV1,
    SeriesUnallocatedPermitExpiryRequestV1,
};
use dclutch_product::ContentId as ProductContentId;
use dclutch_product::svm_reader::{
    AuthenticatedProductRuntimeV3, FinalizedRecordFrameV2 as ProductRecordFrameV2,
    ProductRecordBumpsV3, ProductRuntimeFrameV3, authenticate_product_runtime_v3_hinted,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::activation_auth_v1::{
    authenticate_activated_role_in_frame_v1, authenticate_activation_cache_identity_v1,
    require_cache_account,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_CACHE_BUMP_OFFSET_V1,
    ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1,
};
use dclutch_registry::svm::{
    AuthenticatedRoleReceiptV1, ProgramDataMetadataV3View,
    continuation_v1::{RegistryContinuationAdmissionSeedsV1, RegistryContinuationRequestV1},
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_market::rent::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_vm::request_profile::{
    ProjectionRegisterKindV1, ProjectionRegisterSpaceV1, ProjectionRegistersV1, ProjectionTargetV1,
    RequestProfileV1, SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1,
    project_atomic as project_request_atomic,
    v2::{NativeSignatureRegistersV1, REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, RequestProfileV2},
    v3::{
        BorrowedWitnessPolicyV3, BorrowedWitnessRoleV3, REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID,
        RequestProfileV3,
    },
    v4::{ProjectionRegistersV4, REQUEST_PROFILE_V4_SCHEMA_RELEASE_ID, RequestProfileV4},
};
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3, admit_occurrence_bytes, future_market_projection,
    plan::{ReplayCandidateV3, SeriesReplayActionV3, evaluate_replay_v3},
    replay::{SERIES_TICKET_STATE_BYTES_V3, SeriesStateV3, TicketStateSeedsV3, TicketStateV3},
    request::admit_series_action_v3,
    ticket_admission_v1::SERIES_TICKET_PREPARED_ADMISSIBLE_STATES_V1,
};
use dclutch_custody::token_svm::{COption as TokenCOption, TokenAccount};
use dclutch_vm::v3::{
    ProgramV3 as TransitionProgramV3, RegisterInput, RegisterKindV3, RegisterOutput,
    RegisterSpaceV3, RegisterWriteTargetV3, SCHEMA_RELEASE_ID as TRANSITION_SCHEMA_ID_V3,
    execute_fold_atomic,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::{hash, hashv},
    instruction::AccountMeta,
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, transfer as system_transfer};

use crate::{
    TradingSbfError,
    admitted_composition_v3::{
        ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4, ADMITTED_ACCELERATOR_HOT_FIXED_START_V4,
        ADMITTED_ACCELERATOR_OUTPUT_PAGE_ACCOUNT_V4,
        ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4,
        ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4, AdmittedCpiFrameV3,
        admitted_caller_authority_count_v3, admitted_runtime_accounts_start_v4,
        execute_admitted_aot_v3,
    },
    child_authority_v4::PreflightedCallerBumpV4,
    child_receipt_v3::{
        ChildReceiptBankV3, ExpectedReceiptProvenanceV4, receipt_dependency_width_v3,
        require_chain_receipt_width_v3,
    },
    claims_composition_v3::{SparsePostResourceVerificationV3, claims_child_wire_capacity_v3},
    core_composition_v3::{
        CoreCompositionParentV3, execute_core_route_v3,
        is_series_permit_expiry_precommit_observation_v1, preflight_core_route_v3,
    },
    dispatch::TradingFamilyContextV1,
    dynamic_accounts_v4::{
        PhysicalAccountsV4, dynamic_declared_privileges_v4, dynamic_logical_account_count_v4,
        expand_dynamic_physical_accounts_v4,
    },
    entrypoint_adapter::{HeapScratchRegionV1, ScratchVecV1},
    execution_strategy_v2::{
        ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2, AuthenticatedExecutionStrategyV2,
        INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2, SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2,
        authenticate_activated_current_deployment,
        authenticate_execution_strategy_from_sealed_capability_v2,
    },
    native_signature::{
        SysvarInstructionV1, borrow_authenticated_instructions_v1,
        seed_native_signatures_at_authenticated_instruction,
    },
    shadow_composition_v3::{ShadowCpiFrameV3, execute_shadow_aot_v3},
};

/// One authenticated Effect artifact selected by the schema-bound V4
/// capability descriptor. V3 remains an explicit migration input; successor
/// descriptors execute through V4 resolution so account spans and local
/// effects share one coordinate authority.
#[derive(Clone, Copy)]
struct SelectedEffectProgramV4<'a> {
    base: EffectProgramV3<'a>,
    successor: EffectProgramV4<'a>,
    funding: Option<EffectProgramV5<'a>>,
}

/// One route's ordered borrowed ranges from the authenticated Effect V4.
///
/// This view carries no family tag and invents no range. Every byte appended by
/// [`Self::append_to`] is selected by the sealed Effect, resolved from the
/// transition's authenticated output registers, and borrowed from the exact
/// top-level request. The range table remains the sole topology authority.
#[derive(Clone, Copy)]
pub(crate) struct BorrowedRouteRangesV4<'effect, 'registers, 'request> {
    effect: EffectProgramV4<'effect>,
    route: u16,
    tail_count: u32,
    scalars: &'registers [u64],
    family_request: &'request [u8],
}

impl<'effect, 'registers, 'request> BorrowedRouteRangesV4<'effect, 'registers, 'request> {
    const fn new(
        effect: EffectProgramV4<'effect>,
        route: u16,
        tail_count: u32,
        scalars: &'registers [u64],
        family_request: &'request [u8],
    ) -> Self {
        Self {
            effect,
            route,
            tail_count,
            scalars,
            family_request,
        }
    }

    /// Exact number of route-local ranges in source-table order.
    pub(crate) fn count(self) -> Result<u16, ProgramError> {
        self.effect
            .borrowed_range_count_for_route(self.route)
            .map_err(|_| TradingSbfError::Content.into())
    }

    /// Exact concatenated width, with every request bound checked first.
    pub(crate) fn byte_len(self) -> Result<usize, ProgramError> {
        let count = self.count()?;
        let mut total = 0_usize;
        let mut ordinal = 0_u16;
        while ordinal < count {
            let bytes = self.range(ordinal)?;
            total = total
                .checked_add(bytes.len())
                .ok_or(TradingSbfError::Content)?;
            ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        Ok(total)
    }

    /// Append every route-local range atomically in authenticated table order.
    pub(crate) fn append_to(self, output: &mut Vec<u8>) -> Result<(), ProgramError> {
        let count = self.count()?;
        let additional = self.byte_len()?;
        output
            .len()
            .checked_add(additional)
            .ok_or(TradingSbfError::Content)?;
        output
            .try_reserve_exact(additional)
            .map_err(|_| TradingSbfError::HeapExhausted)?;
        let mut ordinal = 0_u16;
        while ordinal < count {
            output.extend_from_slice(self.range(ordinal)?);
            ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        Ok(())
    }

    /// Exact family request used only by the zero-range legacy compatibility
    /// branch. A V4 range never derives its bytes through this accessor.
    pub(crate) const fn family_request(self) -> &'request [u8] {
        self.family_request
    }

    /// Resolve one route-local range's coordinates without slicing it.
    ///
    /// A consumer that must bind the range to the request BEFORE it -- the
    /// SignedDelta plan's `request_id` commits to exactly that prefix --
    /// needs the offset, which the byte slice alone cannot carry.
    pub(crate) fn resolved_range(
        self,
        ordinal: u16,
    ) -> Result<dclutch_vm::effect::v4::ResolvedBorrowedRangeV4, ProgramError> {
        self.effect
            .resolved_borrowed_range_for_tail(self.route, ordinal, self.tail_count, self.scalars)
            .map_err(|_| TradingSbfError::Content.into())
    }

    pub(crate) fn range(self, ordinal: u16) -> Result<&'request [u8], ProgramError> {
        self.resolved_range(ordinal)?
            .slice(self.family_request)
            .map_err(|_| TradingSbfError::Content.into())
    }
}

impl<'a> SelectedEffectProgramV4<'a> {
    const fn base(self) -> EffectProgramV3<'a> {
        self.base
    }

    const fn funding(self) -> Option<EffectProgramV5<'a>> {
        self.funding
    }

    fn resolved_invocation(
        self,
        route: u16,
        invocation: u32,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<dclutch_vm::effect::v3::ResolvedInvocationV3, TradingSbfError> {
        self.successor
            .resolved_invocation(route, invocation, tail_count, scalars, identities)
            .map(|resolved| resolved.invocation)
            .map_err(|_| TradingSbfError::Content)
    }

    fn resolved_fixed_effect(
        self,
        index: u16,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<ResolvedEffectV3, TradingSbfError> {
        self.successor
            .resolved_fixed_effect(index, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)
    }

    fn resolved_item_effect(
        self,
        item: u32,
        index: u16,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<ResolvedEffectV3, TradingSbfError> {
        self.successor
            .resolved_item_effect(item, index, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)
    }
}

impl<'a> core::ops::Deref for SelectedEffectProgramV4<'a> {
    type Target = EffectProgramV3<'a>;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

#[cfg(feature = "families")]
use crate::resolution_composition_v3::{
    ResolutionCompositionParentV3, execute_resolution_route_v3, preflight_resolution_route_v3,
};

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
use crate::{
    claims_composition_v3::{ClaimsRouteReceiptV3, execute_claims_route_v3},
    custody_composition_v3::{
        CustodyCompositionParentV3, execute_custody_route_v3, preflight_custody_route_v3,
    },
};
#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
use dclutch_claims::composition_v3::{
    ClaimsCompositionErrorV3, ClaimsCompositionParentV3, ClaimsCompositionV3, ClaimsExternalOnceV3,
};
// These are SBF-heap profile bounds, not semantic/product limits. The lifting
// path is scratch-page transport under authenticated ExecutionStrategy V2.
const MAX_HOT_RUNTIME_ACCOUNTS_V3: usize = 256;
const MAX_HOT_SCALARS_V3: usize = 512;
const MAX_HOT_IDENTITIES_V3: usize = 128;
const MAX_HOT_REQUEST_BYTES_V3: usize = 8_192;
const HOT_SELECTED_CONFIG_LOGICAL_ACCOUNT_V3: usize = 1;
const HOT_LINKED_BASIS_LOGICAL_ACCOUNT_V3: usize = 4;

const CHILD_RECEIPT_CONTEXT_DOMAIN_V4: &[u8] = b"dclutch:hot-child-receipt-context:v4";
const CHILD_REQUEST_DIGEST_DOMAIN_V4: &[u8] = b"dclutch:hot-child-request:v4";
const CHILD_REQUEST_DIGEST_DOMAIN_V5: &[u8] = b"dclutch:hot-child-request-ranges:v5";
const VACANT_ROOT_POSTSTATE_DOMAIN_V3: &[u8] = b"dclutch:hot-vacant-root-poststate:v3";

/// Canonical digest of a base child request and the legacy optional witness.
///
/// This is the exact historical V4 framing and remains the zero-range path, so
/// existing General and legacy bundles retain byte-for-byte receipt provenance.
pub fn child_request_digest_v4(
    child_request: &[u8],
    borrowed_witness: Option<&[u8]>,
) -> Result<[u8; 32], ProgramError> {
    let presence = [0_u8, u8::from(borrowed_witness.is_some())];
    let child_request_len = u32::try_from(child_request.len())
        .map_err(|_| TradingSbfError::Content)?
        .to_le_bytes();
    let witness_len = u32::try_from(borrowed_witness.map_or(0, <[u8]>::len))
        .map_err(|_| TradingSbfError::Content)?
        .to_le_bytes();
    Ok(hashv(&[
        CHILD_REQUEST_DIGEST_DOMAIN_V4,
        &presence,
        &child_request_len,
        &witness_len,
        child_request,
        borrowed_witness.unwrap_or(&[]),
    ])
    .to_bytes())
}

/// Canonical digest of one base child request and ordered authenticated ranges.
///
/// Count and every component width precede the bytes. Neither a different
/// range split nor a base/range boundary substitution can therefore share a
/// digest merely because the concatenated child wire matches. `range_at` is
/// evaluated once per ordinal and its results are retained for both framing
/// passes, so a stateful caller cannot substitute between length and data.
pub fn child_request_digest_v5<'a>(
    child_request: &[u8],
    range_count: u16,
    mut range_at: impl FnMut(u16) -> Option<&'a [u8]>,
) -> Result<[u8; 32], ProgramError> {
    if range_count == 0 {
        return Err(TradingSbfError::Content.into());
    }
    let mut ranges = Vec::new();
    ranges
        .try_reserve_exact(usize::from(range_count))
        .map_err(|_| TradingSbfError::HeapExhausted)?;
    let mut range_bytes = 0_usize;
    let mut ordinal = 0_u16;
    while ordinal < range_count {
        let range = range_at(ordinal).ok_or(TradingSbfError::Content)?;
        range_bytes = range_bytes
            .checked_add(range.len())
            .ok_or(TradingSbfError::Content)?;
        ranges.push(range);
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    let child_request_len = u32::try_from(child_request.len())
        .map_err(|_| TradingSbfError::Content)?
        .to_le_bytes();
    let length_table_bytes = usize::from(range_count)
        .checked_mul(4)
        .ok_or(TradingSbfError::Content)?;
    let capacity = CHILD_REQUEST_DIGEST_DOMAIN_V5
        .len()
        .checked_add(2 + 4)
        .and_then(|width| width.checked_add(length_table_bytes))
        .and_then(|width| width.checked_add(child_request.len()))
        .and_then(|width| width.checked_add(range_bytes))
        .ok_or(TradingSbfError::Content)?;
    let mut framed = Vec::new();
    framed
        .try_reserve_exact(capacity)
        .map_err(|_| TradingSbfError::HeapExhausted)?;
    framed.extend_from_slice(CHILD_REQUEST_DIGEST_DOMAIN_V5);
    framed.extend_from_slice(&range_count.to_le_bytes());
    framed.extend_from_slice(&child_request_len);
    for range in &ranges {
        let len = u32::try_from(range.len()).map_err(|_| TradingSbfError::Content)?;
        framed.extend_from_slice(&len.to_le_bytes());
    }
    framed.extend_from_slice(child_request);
    for range in ranges {
        framed.extend_from_slice(range);
    }
    if framed.len() != capacity {
        return Err(TradingSbfError::Content.into());
    }
    Ok(hash(&framed).to_bytes())
}

/// Diagnostic-only phase checkpoint: phase name, remaining compute units, and
/// the SBF bump allocator's total-ever-allocated position.
///
/// The default SBF allocator never frees and returns the new bump position
/// itself, so the address of one fresh single-byte allocation reads the exact
/// running total. The whole checkpoint is one out-of-line call taking a single
/// `&str`, because `process_hot_execution_v3` is already near the 4KB SBF
/// frame limit: expanding three separate syscalls inline at ten phases spills
/// enough of its frame to make the profiled executable overwrite its own
/// caller frame, which silently invalidates every number it prints.
///
/// Reports the total outstanding first, then the two ends it is the sum of, so
/// a deliberately oversized diagnostic heap can be read from the same log.
///
/// **The first number is the one to read.** The upward position alone stopped
/// being the heap requirement when the allocator grew a scratch end (W2p): a
/// bank in the scratch region is outstanding heap that the upward position
/// does not see. Every figure in a W2p phase table is
/// `upward + scratch`.
#[cfg(feature = "hot-cu-profile")]
#[inline(never)]
pub(crate) fn hot_checkpoint(phase: &str) {
    solana_program::log::sol_log(phase);
    solana_program::log::sol_log_compute_units();
    let (position, scratch) = hot_heap_outstanding();
    solana_program::log::sol_log_64(position.saturating_add(scratch), position, scratch, 0, 0);
}

/// Diagnostic-only allocation mark: label and heap outstanding, no compute
/// read.
///
/// A phase checkpoint answers "how much heap had this phase spent"; it cannot
/// say which bank inside a phase spent it. Attributing the wall needs marks
/// between individual allocations, and at that density the two extra syscalls
/// `hot_checkpoint` makes are themselves the measurement's biggest cost. This
/// logs one line, in the same three-number shape as a checkpoint.
///
/// The subtraction between two consecutive marks is the exact charge of what
/// lies between them at the upward end, and the exact release of a scratch
/// region at the other.
#[cfg(feature = "hot-cu-profile")]
#[inline(never)]
pub(crate) fn hot_heap_mark(label: &str) {
    let (position, scratch) = hot_heap_outstanding();
    solana_program::log::sol_log(label);
    solana_program::log::sol_log_64(
        position.saturating_add(scratch),
        position,
        scratch,
        hot_heap_capacity(),
        0,
    );
}

/// The ceiling the allocator is actually enforcing, as the fourth logged word.
///
/// A mark that prints only what has been HANDED OUT cannot say whether the next
/// allocation will fit, because the ceiling is not a constant: `admit_heap_frame_v1`
/// raises it mid-invocation for a route that declared the extended profile. Read
/// without it, 30,896 outstanding looks like exhaustion of a 32,768 default and
/// is in fact 47% of a 65,536 grant -- opposite conclusions from the same three
/// numbers. That reading cost a one-line throwaway probe on 2026-09-01; the
/// probe is this line now.
#[cfg(feature = "hot-cu-profile")]
fn hot_heap_capacity() -> u64 {
    #[cfg(all(
        target_os = "solana",
        not(feature = "custom-heap"),
        not(feature = "no-entrypoint")
    ))]
    {
        u64::try_from(crate::entrypoint_adapter::program_heap_capacity_v1()).unwrap_or(u64::MAX)
    }
    #[cfg(not(all(
        target_os = "solana",
        not(feature = "custom-heap"),
        not(feature = "no-entrypoint")
    )))]
    {
        0
    }
}

/// The bump position and the scratch bytes outstanding, both as offsets from
/// the heap floor.
///
/// On chain both come from the allocator's own header, so a mark costs no
/// allocation of its own. Off chain there is no program heap: a probe
/// allocation still reads a monotone position, which is all the host build
/// ever used this for.
#[cfg(feature = "hot-cu-profile")]
fn hot_heap_outstanding() -> (u64, u64) {
    #[cfg(all(
        target_os = "solana",
        not(feature = "custom-heap"),
        not(feature = "no-entrypoint")
    ))]
    {
        (
            u64::try_from(crate::entrypoint_adapter::program_heap_bytes_used_v1())
                .unwrap_or(u64::MAX),
            u64::try_from(crate::entrypoint_adapter::program_heap_scratch_bytes_v1())
                .unwrap_or(u64::MAX),
        )
    }
    #[cfg(not(all(
        target_os = "solana",
        not(feature = "custom-heap"),
        not(feature = "no-entrypoint")
    )))]
    {
        let probe = Vec::<u8>::with_capacity(1);
        let floor = usize::try_from(solana_program::entrypoint::HEAP_START_ADDRESS).unwrap_or(0);
        (
            u64::try_from((probe.as_ptr() as usize).saturating_sub(floor)).unwrap_or(u64::MAX),
            0,
        )
    }
}

#[cfg(feature = "hot-cu-profile")]
macro_rules! hot_cu_checkpoint {
    ($phase:literal) => {
        crate::hot_v3::hot_checkpoint(concat!("dclutch-hot-cu:", $phase))
    };
}

#[cfg(not(feature = "hot-cu-profile"))]
macro_rules! hot_cu_checkpoint {
    ($phase:literal) => {};
}

#[cfg(feature = "hot-cu-profile")]
macro_rules! hot_heap_mark {
    ($label:literal) => {
        crate::hot_v3::hot_heap_mark(concat!("dclutch-hot-heap:", $label))
    };
}

#[cfg(not(feature = "hot-cu-profile"))]
macro_rules! hot_heap_mark {
    ($label:literal) => {};
}

/// Name, under the diagnostic feature, a cause this route's wire cannot carry.
///
/// `TradingSbfError::Content` covers thousands of sites, and the callees that
/// reach it have already computed WHICH conjunct refused -- an
/// `account_profile_contract::v2::Error` distinguishing forty causes, say. A
/// `map_err(|_| Content)` throws that away and converts a located defect into a
/// bisection, which AGENTS.md prices at hours, three times over, in one day.
///
/// This keeps the cause where a reader looks first without paying for `Debug`
/// formatting in the production ELF: under `hot-cu-profile` it logs beside the
/// CU checkpoints, and without it the error is dropped exactly as before.
/// Log the first coordinate whose observed width disagrees with its rule.
///
/// `Error::DataLengthMismatch` names the walk, not the account, and the walk is
/// eighty-one accounts wide on the Series Expire profile. The rule accessors are
/// public and the arithmetic is `data_length + data_item_stride * tail_count`,
/// so the coordinate is one loop away from the refusal that already happened.
/// Diagnostic-only: nothing here runs in a production ELF.
#[cfg(feature = "hot-cu-profile")]
#[inline(never)]
fn log_first_data_length_disagreement_v1(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    observations: &[AccountObservationV1<'_>],
) {
    for (coordinate, account) in observations.iter().enumerate() {
        let Ok(index) = u16::try_from(coordinate) else {
            return;
        };
        let Ok(rule) = profile.rule(false, index) else {
            return;
        };
        if rule.prestate() != AccountPrestateV2::Exact {
            continue;
        }
        let expected = u64::from(rule.data_length()).saturating_add(
            u64::from(rule.data_item_stride()).saturating_mul(u64::from(tail_count)),
        );
        let observed = account.data().len() as u64;
        if observed != expected {
            solana_program::log::sol_log(
                "dclutch-hot-why:data-length coordinate/expected/observed",
            );
            solana_program::log::sol_log_64(coordinate as u64, expected, observed, 0, 0);
            return;
        }
    }
    solana_program::log::sol_log("dclutch-hot-why:data-length no fixed-rule coordinate disagrees");
}

#[cfg(feature = "hot-cu-profile")]
macro_rules! hot_cu_data_length_disagreement {
    ($profile:expr, $tail:expr, $observations:expr) => {
        crate::hot_v3::log_first_data_length_disagreement_v1($profile, $tail, $observations)
    };
}

#[cfg(not(feature = "hot-cu-profile"))]
macro_rules! hot_cu_data_length_disagreement {
    ($profile:expr, $tail:expr, $observations:expr) => {};
}

/// Log one of the static-ownership verdict's four ranges if it strayed.
///
/// `Error::TokenRangeMismatch` is one code over four artifacts, and the
/// distinction is pointer identity, which no equality test recovers after the
/// fact. Diagnostic-only.
///
/// ONE RANGE PER CALL, NOT AN ARRAY OF FOUR. The closure this is logged from is
/// inlined into `execute_authenticated_hot_v3`, so a `[&[u8]; 4]` built for the
/// call is sixty-four bytes of that function's frame -- exactly the sixty-four
/// that put the profiled link over the bound (3,840 measured plain, 3,904 under
/// the profile, five "overwrites values in the frame" diagnostics) while the
/// shipped link sat at zero. Four calls of five words each ride in registers
/// and the measurement stops moving the frame it measures.
#[cfg(feature = "hot-cu-profile")]
#[inline(never)]
fn log_sealed_ownership_range_v1(
    verdict: &dclutch_vm::capability_seal::SealedStaticOwnershipV1<'_>,
    action: u32,
    index: usize,
    seen: &[u8],
) {
    if index == 0 {
        solana_program::log::sol_log("dclutch-hot-why:sealed-ownership action proved/observed");
        solana_program::log::sol_log_64(u64::from(verdict.action()), u64::from(action), 0, 0, 0);
    }
    let proved_ranges = verdict.proved_ranges();
    let Some(proved) = proved_ranges.get(index) else {
        return;
    };
    if core::ptr::eq(proved.as_ptr(), seen.as_ptr()) && proved.len() == seen.len() {
        return;
    }
    solana_program::log::sol_log("dclutch-hot-why:sealed-ownership role/proved-len/seen-len");
    solana_program::log::sol_log_64(
        index as u64,
        proved.len() as u64,
        seen.len() as u64,
        proved.as_ptr() as u64,
        seen.as_ptr() as u64,
    );
}

#[cfg(feature = "hot-cu-profile")]
macro_rules! hot_cu_sealed_ownership_ranges {
    ($verdict:expr, $action:expr, [$($seen:expr),+ $(,)?]) => {{
        let mut index = 0usize;
        $(
            crate::hot_v3::log_sealed_ownership_range_v1(&$verdict, $action, index, $seen);
            index += 1;
        )+
        let _ = index;
    }};
}

#[cfg(not(feature = "hot-cu-profile"))]
macro_rules! hot_cu_sealed_ownership_ranges {
    ($verdict:expr, $action:expr, $observed:expr) => {};
}

/// Name which conjunct of the role-carrier resolution refused.
///
/// `resolve_carrier_by_representative_v3` has SIX `TradingSbfError::Release`
/// exits and the wire carries one code for all of them, so a role that fails to
/// resolve is a five-way guess: the tables disagreed in length, a carrier
/// arrived signing or writable or non-executable, two distinct physical
/// accounts carried the key, or the key is not in the frame at all. The first
/// logged word is the case; the rest are its operands. Diagnostic-only.
#[cfg(feature = "hot-cu-profile")]
pub(crate) fn log_role_carrier_refusal_v1(case: u64, first: u64, second: u64, third: u64) {
    solana_program::log::sol_log("dclutch-hot-why:role-carrier case/first/second/third");
    solana_program::log::sol_log_64(case, first, second, third, 0);
}

#[cfg(feature = "hot-cu-profile")]
macro_rules! hot_cu_role_carrier {
    ($case:expr, $first:expr, $second:expr, $third:expr) => {
        crate::hot_v3::log_role_carrier_refusal_v1($case, $first, $second, $third)
    };
}

#[cfg(not(feature = "hot-cu-profile"))]
macro_rules! hot_cu_role_carrier {
    ($case:expr, $first:expr, $second:expr, $third:expr) => {};
}

/// Name which conjunct of the Custody child preparation refused.
///
/// `custody_composition_v3::prepare` and the frame-shape guard behind it have
/// EIGHT `TradingSbfError::Content` exits and the wire carries one code for all
/// of them -- the same code 2,124 other sites in this program publish. A route
/// that refuses here is therefore an eight-way guess: the parent facts were
/// zero or foreign, the callee arrived signing/writable/non-executable or the
/// successor width disagreed, the invocation was not a Custody one or carried a
/// borrowed witness, the request slice fell outside the bank, the request bytes
/// did not decode or named the External compartment, one of the six parent
/// bindings inside the request disagreed, the frame smuggled the callee into a
/// coordinate, or the frame was shorter than the replay coordinate. The first
/// logged word is the case; the rest are its operands. Diagnostic-only.
#[cfg(feature = "hot-cu-profile")]
pub(crate) fn log_custody_prepare_refusal_v1(case: u64, first: u64, second: u64, third: u64) {
    solana_program::log::sol_log("dclutch-hot-why:custody-prepare case/first/second/third");
    solana_program::log::sol_log_64(case, first, second, third, 0);
}

#[cfg(feature = "hot-cu-profile")]
macro_rules! hot_cu_custody_prepare {
    ($case:expr, $first:expr, $second:expr, $third:expr) => {
        crate::hot_v3::log_custody_prepare_refusal_v1($case, $first, $second, $third)
    };
}

#[cfg(not(feature = "hot-cu-profile"))]
macro_rules! hot_cu_custody_prepare {
    ($case:expr, $first:expr, $second:expr, $third:expr) => {};
}

pub(crate) use hot_cu_custody_prepare as hot_cu_custody_prepare_macro;

/// The `Debug` formatting of a refused cause, kept out of the caller's frame.
///
/// `msg!` with a `{:?}` argument expands to `format!` at the call site, and a
/// closure passed to `map_err` is inlined into the function it sits in -- so
/// the formatting machinery landed in `execute_authenticated_hot_v3`'s own
/// frame and pushed it past the 4,096-byte bound under the profile feature,
/// five diagnostics the strict release gate refuses. Generic and
/// `#[inline(never)]` for the same reason `hot_checkpoint` is: the
/// measurement must not change the frame it measures.
#[cfg(feature = "hot-cu-profile")]
#[inline(never)]
pub(crate) fn hot_reason<E: core::fmt::Debug>(label: &str, error: &E) {
    solana_program::log::sol_log(label);
    solana_program::msg!("{:?}", error);
}

#[cfg(feature = "hot-cu-profile")]
macro_rules! hot_cu_reason {
    ($label:literal, $error:expr) => {{
        crate::hot_v3::hot_reason(concat!("dclutch-hot-why:", $label), &$error)
    }};
}

#[cfg(not(feature = "hot-cu-profile"))]
macro_rules! hot_cu_reason {
    ($label:literal, $error:expr) => {{
        let _ = &$error;
    }};
}

// The admitted CPI loop lives in `admitted_composition_v3`, and until this
// export the whole loop was ONE heap span in this module -- which is how
// twelve kilobytes of it went unattributed when the input transport changed.
pub(crate) use hot_heap_mark as hot_heap_mark_macro;
// The child-route CPI legs live in the four `*_composition_v3` modules, and
// until this export the whole span from one child's return to the next
// child's entry was ONE unattributed number in this module -- 25,247,
// 41,492 and 48,871 CU on the partial equity Remove, larger than any
// authentication the note prices.
pub(crate) use hot_cu_checkpoint as hot_cu_checkpoint_macro;

/// Shadow caller-authority PDA after six authenticated strategy extras.
pub const HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3: usize = HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3 + 6;
/// First profile-defined runtime account for Shadow-AOT execution.
pub const HOT_SHADOW_RUNTIME_ACCOUNTS_START_V3: usize = HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3 + 1;
/// First admitted-AOT caller authority after eight authenticated strategy extras.
pub const HOT_ADMITTED_CALLER_AUTHORITIES_START_V3: usize =
    HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3 + 8;

/// Return the first profile-defined runtime account for admitted AOT.
///
/// One release-pinned Trading authority is supplied for each canonical output
/// acknowledgement chunk; the width follows only authenticated register
/// geometry and the chain return-data limit.
pub fn hot_admitted_runtime_accounts_start_v3(
    profile: AcceleratorTransportProfileV2,
    scalar_count: u32,
    identity_count: u32,
) -> Result<usize, ProgramError> {
    HOT_ADMITTED_CALLER_AUTHORITIES_START_V3
        .checked_add(admitted_caller_authority_count_v3(
            profile,
            scalar_count,
            identity_count,
        )?)
        .and_then(|start| start.checked_add(hot_admitted_output_page_accounts_v3(profile)))
        .ok_or_else(|| TradingSbfError::Content.into())
}

/// Accounts the output page occupies in the TOP-LEVEL Hot frame: one, or none.
///
/// The page sits in the same relative place in both frames -- immediately
/// before the AccountProfile-ordered runtime slice -- so the two shapes agree
/// by construction rather than by two authors happening to write the same
/// thing. In the CPI frame that place is named by
/// `ADMITTED_OUTPUT_PAGE_ACCOUNT_V3`; here it is one account past the
/// caller-authority span, which is one account long under this profile.
pub const fn hot_admitted_output_page_accounts_v3(profile: AcceleratorTransportProfileV2) -> usize {
    match profile {
        AcceleratorTransportProfileV2::OutputPageV3 => 1,
        AcceleratorTransportProfileV2::ChunkedBankV2
        | AcceleratorTransportProfileV2::ShadowTranscriptV3 => 0,
    }
}

mod accelerator;
mod execute;
mod series_expiry;
mod direct;
mod frame;
mod strategy;
mod accounts;
mod lifecycle;
mod children;
mod commit;
#[cfg(test)]
mod tests;

use accelerator::*;
use execute::*;
use series_expiry::*;
use direct::*;
use frame::*;
use strategy::*;
use accounts::*;
use lifecycle::*;
use children::*;
use commit::*;

pub use accelerator::{AcceleratorArtifactClassV4, AuthenticatedAcceleratorInvocationV4, authenticate_accelerator_invocation_v4};
pub(crate) use accelerator::{AuthenticatedAcceleratorCallerV4};
pub use accounts::{ChildInvocationBuffersV3, DowngradedEffectAccountsV3};
pub(crate) use accounts::{child_route_privileges_v3, downgraded_effect_accounts_v3};
pub(crate) use children::{invocation_accounts_contain_program};

/// Execute one complete common V3 hot action.
#[inline(never)]
pub fn process_hot_execution_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    hot_cu_checkpoint!("start");
    let (envelope, family_request) = HotExecutionEnvelopeV3::split_instruction(instruction_data)
        .map_err(|_| TradingSbfError::Content)?;
    let invocation =
        authenticate_hot_invocation_v3(program_id, accounts, instruction_data, envelope)?;
    let frame =
        parse_hot_frame_boxed_v3(program_id, accounts, invocation.permits_fixed_market_union)?;
    let request_digest = hash(family_request).to_bytes();
    let root_prestate = {
        let bytes = frame
            .root
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Root)?;
        hash(&bytes).to_bytes()
    };
    if root_prestate != envelope.root_prestate_digest() {
        return Err(TradingSbfError::Root.into());
    }

    // The fixed Market is always the live Series controller. Exceptional
    // Expire authority is earned only after this ordinary Hot prelude has
    // authenticated the controller Market, its persistent root, and Product
    // graph. The occurrence's distinct future Market is a route-local account,
    // never a substitute for the fixed controller coordinate.
    let market = authenticate_market_boxed_v3(&frame, envelope)?;
    let root = authenticate_root_boxed_v3(
        program_id,
        &frame,
        envelope,
        &market,
        invocation.role_authentication,
    )?;
    let rent = Rent::from_account_info(frame.rent).map_err(|_| TradingSbfError::Content)?;
    let product_runtime_v3 = authenticate_product_runtime_boxed_v3(&frame, &market)?;
    let authenticated_series_expiry_rent_credit = try_authenticate_series_expiry_premarket_v1(
        program_id,
        accounts,
        family_request,
        invocation,
        &frame,
        &root,
        &product_runtime_v3,
    )?;
    let market = AuthenticatedLogicalMarketV3::from_live(&market);

    authenticate_and_execute_hot_v3(&AuthenticatedHotPreludeV3 {
        program_id,
        accounts,
        instruction_data,
        family_request,
        envelope,
        invocation,
        frame,
        request_digest,
        root_prestate,
        market,
        root,
        rent,
        product_runtime_v3,
        authenticated_series_expiry_replay: authenticated_series_expiry_rent_credit.is_some(),
        authenticated_series_expiry_rent_credit: authenticated_series_expiry_rent_credit
            .unwrap_or([0; 32]),
    })
}

mod seal;

pub use seal::{
    CLOSE_SEAL_ACCOUNT_COUNT_V1, CLOSE_SEAL_ACCOUNT_V1, CLOSE_SEAL_ACTIVATION_CACHE_ACCOUNT_V1,
    CLOSE_SEAL_CLOSER_ACCOUNT_V1, CLOSE_SEAL_REGISTRY_ACCOUNT_V1, CLOSE_SEAL_RENT_ACCOUNT_V1,
    CLOSE_SEAL_TRADING_PROGRAM_ACCOUNT_V1, CLOSE_SEAL_TRADING_PROGRAMDATA_ACCOUNT_V1,
    SEAL_ACCOUNT_COUNT_V1, SEAL_PAYER_ACCOUNT_V1, SEAL_SYSTEM_PROGRAM_ACCOUNT_V1,
    process_capability_seal_close_v1, process_capability_seal_v1,
};
use seal::{authenticate_capability_seal_v3, borrow_sealed_record, sealed_token};

/// Return whether instruction data selects the common V3 hot outer.
pub fn is_hot_execution_v3(instruction_data: &[u8]) -> bool {
    instruction_data.get(..8) == Some(HOT_EXECUTION_MAGIC_V3.as_slice())
}
