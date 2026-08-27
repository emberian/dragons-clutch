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

use dclutch_account_profile_contract::{
    AccountObservationV1,
    lifecycle_v3::{
        AuthenticatedRentCreditV3, AuthenticatedRentMinimumV3, AuthenticatedRentQuoteV5,
        CoordinateScopeV3, LifecycleContextV3, LifecycleOperationV3,
        LifecycleProtectedRegisterBuffersV3, LifecycleRegisterKindV3, LifecycleRegisterTargetV3,
        LifecycleRegistersV3, LifecycleRentQuoteBuffersV5, LifecycleSeedInputValueV3,
        StateLifecyclePlanV3, StateLifecyclePolicyV5, plan_lifecycle_with_protected_outputs_atomic,
    },
    v2::{
        AccountPrestateV2, AccountProfileV2, ProjectionRegistersV2, RouteAccountPrivilegesV2,
        SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2, TrustedEnvironmentV2,
        derive_effect_permissions, derive_effect_permissions_with_dynamic_spans,
        project_atomic as project_accounts_atomic, project_dynamic_fixed_spans_atomic,
    },
};
use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CONFIG_STAGING_ACCOUNT_V3,
        HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
        HOT_EFFECT_RAW_ACCOUNT_V3, HOT_EFFECT_STAGING_ACCOUNT_V3, HOT_EXECUTION_MAGIC_V3,
        HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        HOT_LIFECYCLE_RAW_ACCOUNT_V3, HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_LINKED_BASIS_STAGING_ACCOUNT_V3,
        HOT_MANIFEST_RAW_ACCOUNT_V3, HOT_MANIFEST_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_PORTFOLIO_STAGING_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PRODUCT_STAGING_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_STAGING_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
        HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
        HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_STRATEGY_STAGING_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3,
        HOT_TRANSITION_STAGING_ACCOUNT_V3, HotExecutionAckV3, HotExecutionEnvelopeV3,
    },
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
    v4::{
        CAPABILITY_PROGRAM_V4_BYTES, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as PROGRAM_SCHEMA_ID_V4, SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5,
    },
};
use dclutch_core_contract::ContentId;
use dclutch_effect_kernel::{
    v2::{AccountInput, AccountPermission, FixedRole},
    v3::{ProgramV3 as EffectProgramV3, ProjectionV3, ResolvedEffectV3},
    v4::{
        ProgramV4 as EffectProgramV4, SCHEMA_RELEASE_ID_V4 as EFFECT_SCHEMA_ID_V4,
        project_atomic as project_effects_v4_atomic,
    },
};
use dclutch_execution_strategy_contract::{
    admitted_v3::{AdmittedInvocationContextV3, admitted_invocation_context_digest_v3},
    shadow_digest_v3::{
        ShadowEffectProjectionV3, ShadowInvocationContextV3, ShadowReceiptDependencyV3,
        ShadowResolvedRouteV3, ShadowRouteKindV3, ShadowRouteRoleV3, ShadowRuntimeObservationV3,
        candidate_digest_v3, effect_digest_v3, family_request_digest_v3,
        invocation_context_digest_v3, receipt_dependencies_digest_v4,
        runtime_observations_digest_v3,
    },
    shadow_v3::{
        ShadowArtifactTupleV3, ShadowExecutionDigestsV3, ShadowRequestV3, ShadowRuntimeShapeV3,
    },
    v2::{
        AcceleratorRequestV2, AcceleratorTransportProfileV2, AuthenticatedScratchPageV2,
        BankTransportV2, EXECUTION_STRATEGY_PROGRAM_BYTES_V2, ExecutionStrategyProgramV2,
        RequestTransportV2, StrategyDispositionV2, classify_bank_transport_v2,
        register_bank_bytes_v2,
    },
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_product_runtime_v2::ContentId as ProductContentId;
use dclutch_product_runtime_v2_svm_reader::{
    AuthenticatedProductRuntimeV3, FinalizedRecordFrameV2 as ProductRecordFrameV2,
    ProductRuntimeFrameV3, authenticate_product_runtime_v3,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{ACTIVATION_PDA_DOMAIN_V1, ActivatedExecutionReleaseSetViewV1};
use dclutch_registry_svm::{
    AuthenticatedRoleReceiptV1, RegistryInstructionV1,
    continuation_v1::{RegistryContinuationAdmissionSeedsV1, RegistryContinuationRequestV1},
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_rent_contract::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
use dclutch_request_profile_contract::{
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
use dclutch_transition_vm::v3::{
    ProgramV3 as TransitionProgramV3, RegisterInput, RegisterKindV3, RegisterOutput,
    RegisterSpaceV3, RegisterWriteTargetV3, SCHEMA_RELEASE_ID as TRANSITION_SCHEMA_ID_V3,
    execute_fold_atomic,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
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
        ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4,
        ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4,
        ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4, AdmittedCpiFrameV3,
        admitted_caller_authority_count_v3, execute_admitted_aot_v3,
    },
    child_receipt_v3::{
        ChildReceiptBankV3, ExpectedReceiptProvenanceV4, require_chain_receipt_width_v3,
    },
    core_composition_v3::{
        CoreCompositionParentV3, execute_core_route_v3, preflight_core_route_v3,
    },
    dispatch::TradingFamilyContextV1,
    dynamic_accounts_v4::{
        downgrade_dynamic_child_accounts_v4, expand_dynamic_physical_accounts_v4,
    },
    execution_strategy_v2::{
        ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2, AuthenticatedExecutionStrategyV2,
        INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2, SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2,
        authenticate_activated_current_deployment, authenticate_execution_strategy_v2,
    },
    native_signature::{
        load_current_top_level_instruction, seed_native_signatures_at_authenticated_instruction,
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
}

impl<'a> SelectedEffectProgramV4<'a> {
    const fn base(self) -> EffectProgramV3<'a> {
        self.base
    }

    fn resolved_invocation(
        self,
        route: u16,
        invocation: u32,
        tail_count: u32,
        scalars: &[u64],
        identities: &[[u8; 32]],
    ) -> Result<dclutch_effect_kernel::v3::ResolvedInvocationV3, TradingSbfError> {
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
use dclutch_claims_svm::composition_v3::{ClaimsCompositionParentV3, ClaimsCompositionV3};
// These are SBF-heap profile bounds, not semantic/product limits. The lifting
// path is scratch-page transport under authenticated ExecutionStrategy V2.
const MAX_HOT_RUNTIME_ACCOUNTS_V3: usize = 256;
const MAX_HOT_SCALARS_V3: usize = 512;
const MAX_HOT_IDENTITIES_V3: usize = 128;
const MAX_HOT_REQUEST_BYTES_V3: usize = 8_192;
const HOT_SELECTED_CONFIG_LOGICAL_ACCOUNT_V3: usize = 1;
const HOT_LINKED_BASIS_LOGICAL_ACCOUNT_V3: usize = 4;

const EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-execution:v3";
const CHILD_EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-child-execution:v3";
const CHILD_RECEIPT_CONTEXT_DOMAIN_V4: &[u8] = b"dclutch:hot-child-receipt-context:v4";
const CHILD_REQUEST_DIGEST_DOMAIN_V4: &[u8] = b"dclutch:hot-child-request:v4";

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
/// Reports both bytes used against the protocol 32KB heap and the raw bump
/// offset from the heap floor, so a deliberately oversized diagnostic heap can
/// be read from the same log.
#[cfg(feature = "hot-cu-profile")]
#[inline(never)]
fn hot_checkpoint(phase: &str) {
    solana_program::log::sol_log(phase);
    solana_program::log::sol_log_compute_units();
    let probe = Vec::<u8>::with_capacity(1);
    let position = probe.as_ptr() as usize;
    let floor = solana_program::entrypoint::HEAP_START_ADDRESS as usize;
    let ceiling = floor.saturating_add(solana_program::entrypoint::HEAP_LENGTH);
    solana_program::log::sol_log_64(
        ceiling.saturating_sub(position) as u64,
        position.saturating_sub(floor) as u64,
        0,
        0,
        0,
    );
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
    scalar_count: u32,
    identity_count: u32,
) -> Result<usize, ProgramError> {
    HOT_ADMITTED_CALLER_AUTHORITIES_START_V3
        .checked_add(admitted_caller_authority_count_v3(
            scalar_count,
            identity_count,
        )?)
        .ok_or_else(|| TradingSbfError::Content.into())
}

/// Descriptor artifact class exposed by one authenticated accelerator view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceleratorArtifactClassV4 {
    /// AccountProfile selected by CapabilityProgramV4.
    AccountProfile,
    /// RequestProfile selected by CapabilityProgramV4.
    RequestProfile,
    /// LifecycleV5 policy selected by CapabilityProgramV4.
    Lifecycle,
    /// ExecutionStrategy selected by CapabilityProgramV4.
    Strategy,
    /// Transition program selected by CapabilityProgramV4/Strategy.
    Transition,
    /// EffectV4 program selected by CapabilityProgramV4.
    Effect,
}

/// Public read-only facts authenticated for one admitted accelerator callback.
///
/// This is an ephemeral adapter view, not a persisted DTO and not write/CPI
/// authority. The complete family request remains owned by the current
/// top-level Hot instruction; the view owns that loaded instruction only so an
/// external accelerator can borrow its exact request slice after this helper
/// has rejoined the caller PDA, activation, records, Product, runtime digest,
/// and AcceleratorRequestV2 invocation-context digest.
pub struct AuthenticatedAcceleratorInvocationV4<'request, 'accounts, 'info> {
    request: AcceleratorRequestV2<'request>,
    envelope: HotExecutionEnvelopeV3,
    hot_instruction: Vec<u8>,
    strategy: Box<AuthenticatedExecutionStrategyV2>,
    selected_action: u32,
    context: Box<AdmittedInvocationContextV3>,
    product_runtime: Box<AuthenticatedProductRuntimeV3<'accounts, 'info>>,
    claims_program: ContentId,
    custody_program: ContentId,
    span_widths: Vec<u32>,
    input_bank: Vec<u8>,
    scalars: Vec<u64>,
    identities: Vec<[u8; 32]>,
    artifact_raw_accounts: [&'accounts AccountInfo<'info>; 6],
    runtime_accounts: &'accounts [AccountInfo<'info>],
}

impl<'request, 'accounts, 'info> AuthenticatedAcceleratorInvocationV4<'request, 'accounts, 'info> {
    /// Exact canonical AcceleratorRequestV2 supplied by Trading.
    pub const fn request(&self) -> AcceleratorRequestV2<'request> {
        self.request
    }

    /// Exact authenticated common Hot envelope.
    pub const fn envelope(&self) -> HotExecutionEnvelopeV3 {
        self.envelope
    }

    /// Borrow the complete family request from the authenticated top-level instruction.
    pub fn family_request(&self) -> &[u8] {
        self.hot_instruction
            .get(dclutch_capability_program_contract::hot_v3::HOT_FAMILY_REQUEST_OFFSET_V3..)
            .unwrap_or(&[])
    }

    /// Action selector returned by the authenticated CapabilityProgramSetV2.
    pub const fn selected_action(&self) -> u32 {
        self.selected_action
    }

    /// Exact hostile-decoded CapabilityProgramV4 descriptor.
    pub const fn descriptor(&self) -> CapabilityProgramV4 {
        self.strategy.capability_program()
    }

    /// Exact admitted strategy/certificate/admission/deployment witness.
    pub const fn strategy(&self) -> AuthenticatedExecutionStrategyV2 {
        *self.strategy
    }

    /// Complete invocation-context preimage whose digest is in AcceleratorRequestV2.
    pub const fn context(&self) -> AdmittedInvocationContextV3 {
        *self.context
    }

    /// Product-authenticated runtime facts.
    pub const fn product_runtime(&self) -> &AuthenticatedProductRuntimeV3<'accounts, 'info> {
        &self.product_runtime
    }

    /// Independently authenticated Product-linked basis record coordinate.
    pub const fn linked_basis_record(
        &self,
    ) -> dclutch_product_runtime_v2_svm_reader::AuthenticatedRecordV2 {
        self.product_runtime.linked_basis_record
    }

    /// Current Registry-selected Claims program identity.
    pub const fn claims_program(&self) -> ContentId {
        self.claims_program
    }

    /// Current Registry-selected Custody program identity.
    pub const fn custody_program(&self) -> ContentId {
        self.custody_program
    }

    /// Exact protected Profile13 span widths in descriptor order.
    pub fn span_widths(&self) -> &[u32] {
        &self.span_widths
    }

    /// Exact complete pre-Transition register bank committed by the request.
    pub fn input_bank(&self) -> &[u8] {
        &self.input_bank
    }

    /// Scalar prefix decoded without narrowing from the complete input bank.
    pub fn scalars(&self) -> &[u64] {
        &self.scalars
    }

    /// Identity suffix decoded from the complete input bank.
    pub fn identities(&self) -> &[[u8; 32]] {
        &self.identities
    }

    /// Exact finalized raw account for one descriptor artifact class.
    pub const fn artifact_raw_account(
        &self,
        class: AcceleratorArtifactClassV4,
    ) -> &'accounts AccountInfo<'info> {
        let [
            account_profile,
            request_profile,
            lifecycle,
            strategy,
            transition,
            effect,
        ] = self.artifact_raw_accounts;
        match class {
            AcceleratorArtifactClassV4::AccountProfile => account_profile,
            AcceleratorArtifactClassV4::RequestProfile => request_profile,
            AcceleratorArtifactClassV4::Lifecycle => lifecycle,
            AcceleratorArtifactClassV4::Strategy => strategy,
            AcceleratorArtifactClassV4::Transition => transition,
            AcceleratorArtifactClassV4::Effect => effect,
        }
    }

    /// Expanded logical AccountInfo sequence, downgraded read-only for the callback.
    pub const fn runtime_accounts(&self) -> &'accounts [AccountInfo<'info>] {
        self.runtime_accounts
    }
}

/// Authenticate one external admitted-accelerator invocation without lending
/// mutation or child-CPI authority.
#[inline(never)]
pub fn authenticate_accelerator_invocation_v4<'request, 'accounts, 'info>(
    accelerator_program: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    request_bytes: &'request [u8],
) -> Result<Box<AuthenticatedAcceleratorInvocationV4<'request, 'accounts, 'info>>, ProgramError> {
    let request =
        AcceleratorRequestV2::decode(request_bytes).map_err(|_| TradingSbfError::Content)?;
    let caller_authority = account(accounts, 0)?;
    let fixed = accounts
        .get(
            ADMITTED_ACCELERATOR_HOT_FIXED_START_V4
                ..ADMITTED_ACCELERATOR_HOT_FIXED_START_V4 + ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4,
        )
        .ok_or(TradingSbfError::Content)?;
    let strategy_evidence = accounts
        .get(
            ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4
                ..ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4
                    + ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4,
        )
        .ok_or(TradingSbfError::Content)?;
    let runtime_accounts = accounts
        .get(ADMITTED_ACCELERATOR_RUNTIME_ACCOUNTS_START_V4..)
        .ok_or(TradingSbfError::Content)?;
    let trading_program = account(fixed, HOT_TRADING_PROGRAM_ACCOUNT_V3)?;
    let frame = HotFrameV3::parse_accelerator_readonly(trading_program.key, fixed)?;
    let hot_instruction =
        authenticate_accelerator_top_level_v4(frame, strategy_evidence, caller_authority, request)?;
    let (envelope, family_request) = HotExecutionEnvelopeV3::split_instruction(&hot_instruction)
        .map_err(|_| TradingSbfError::Content)?;
    let root_prestate = {
        let data = frame
            .root
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Root)?;
        hash(&data).to_bytes()
    };
    if root_prestate != envelope.root_prestate_digest() {
        return Err(TradingSbfError::Root.into());
    }
    let (trading_receipt, claims_program, custody_program) =
        authenticate_accelerator_activation_v4(frame, envelope)?;
    let market = authenticate_market_boxed_v3(&frame, envelope)?;
    let root_data = frame
        .root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    let family_context = TradingFamilyContextV1::authenticate(
        frame.trading_program.key,
        frame.root.key,
        frame.root.owner,
        &root_data,
        trading_receipt,
    )?;
    drop(root_data);
    if family_context.market() != envelope.market()
        || family_context.release_set().to_bytes() != envelope.release_set()
        || family_context.generation() != envelope.generation()
    {
        return Err(TradingSbfError::Root.into());
    }
    let rent = Rent::from_account_info(frame.rent).map_err(|_| TradingSbfError::Content)?;
    let product_runtime = authenticate_product_runtime_boxed_v3(&frame, &rent, &market)?;

    let manifest_data = borrow_finalized_record(
        frame,
        frame.manifest_raw,
        frame.manifest_staging,
        &rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        family_context.selection().manifest().to_bytes(),
    )?;
    let entry = authenticate_manifest_entry_boxed_v3(&manifest_data, &family_context)?;
    drop(manifest_data);
    let program_set_data = borrow_finalized_record(
        frame,
        frame.program_set_raw,
        frame.program_set_staging,
        &rent,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        family_context.selection().capability_release().to_bytes(),
    )?;
    let program_set = CapabilityProgramSetV2::decode_selected(
        family_context.selection().capability_release().to_bytes(),
        hash(&program_set_data).to_bytes(),
        &program_set_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let selected_entry = program_set
        .select_entry(family_request)
        .map_err(|_| TradingSbfError::Content)?;
    let selected_action = selected_entry.selector();
    let selected_descriptor = selected_entry.descriptor();
    drop(program_set_data);
    if selected_descriptor.schema().to_bytes() != PROGRAM_SCHEMA_ID_V4
        || selected_descriptor.program() != request.capability_program()
    {
        return Err(TradingSbfError::Content.into());
    }
    let descriptor_data = borrow_finalized_record(
        frame,
        frame.descriptor_raw,
        frame.descriptor_staging,
        &rent,
        selected_descriptor.schema().to_bytes(),
        selected_descriptor.program().to_bytes(),
    )?;
    let descriptor = decode_capability_program_boxed_v3(&descriptor_data)?;
    drop(descriptor_data);
    authenticate_descriptor_root_selection(&descriptor, &family_context, &entry)?;
    drop(entry);

    let config_data = borrow_finalized_record(
        frame,
        frame.config_raw,
        frame.config_staging,
        &rent,
        descriptor.config_schema().to_bytes(),
        family_context.selection().config().to_bytes(),
    )?;
    // As on the canonical path: `borrow_finalized_record` already refused
    // unless `hash(config_data)` is the selected config identity.
    drop(config_data);
    require_common_projection_bindings_v3(CommonProjectionBindingsV3 {
        selected_config: family_context.selection().config().to_bytes(),
        selected_product_record: market.identity.product_record.to_bytes(),
        authenticated_product_record: product_runtime
            .runtime
            .product_record
            .content_digest
            .to_bytes(),
        market_product: market.identity.product_id.to_bytes(),
        runtime_product: product_runtime.runtime.product_id.to_bytes(),
        product_semantic_basis: product_runtime.runtime.liability_basis_id.to_bytes(),
        authenticated_semantic_basis: product_runtime.semantic_basis_id.to_bytes(),
        authenticated_linked_basis: product_runtime
            .linked_basis_record
            .content_digest
            .to_bytes(),
    })?;
    drop(market);
    let (strategy, strategy_end) = authenticate_strategy_boxed_v3(
        &frame,
        strategy_evidence,
        family_context,
        selected_descriptor.schema(),
        selected_descriptor.program(),
        0,
    )?;
    if strategy_end != strategy_evidence.len()
        || strategy.strategy().disposition() != StrategyDispositionV2::AdmittedAot
        || strategy.strategy_program_id() != request.strategy_program()
        || strategy.certificate_program_id() != Some(request.certificate_program())
        || strategy
            .artifact_release()
            .ok_or(TradingSbfError::Content)?
            .program()
            .to_bytes()
            != accelerator_program.to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }

    let input_bank = authenticate_accelerator_input_bank_v4(
        request,
        runtime_accounts,
        frame.trading_program.key,
    )?;
    let (scalars, identities) = decode_accelerator_register_bank_v4(request, &input_bank)?;
    if request.tail_count() != product_runtime.runtime.outcome_count {
        return Err(TradingSbfError::Content.into());
    }
    let span_widths = authenticate_accelerator_artifacts_v4(
        frame,
        &rent,
        &descriptor,
        &strategy,
        request,
        family_request,
        runtime_accounts.len(),
        &scalars,
    )?;

    let context = authenticate_accelerator_context_v4(
        accelerator_program,
        frame,
        envelope,
        family_context,
        selected_action,
        &descriptor,
        &strategy,
        &product_runtime,
        request,
        family_request,
        runtime_accounts,
        root_prestate,
    )?;
    authenticate_accelerator_caller_authority_v4(
        frame.trading_program.key,
        caller_authority,
        envelope,
        frame.root.key,
        request_bytes,
    )?;

    Ok(Box::new(AuthenticatedAcceleratorInvocationV4 {
        request,
        envelope,
        hot_instruction,
        strategy,
        selected_action,
        context,
        product_runtime,
        claims_program,
        custody_program,
        span_widths,
        input_bank,
        scalars,
        identities,
        artifact_raw_accounts: [
            frame.account_profile_raw,
            frame.request_profile_raw,
            frame.lifecycle_raw,
            frame.strategy_raw,
            frame.transition_raw,
            frame.effect_raw,
        ],
        runtime_accounts,
    }))
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_accelerator_artifacts_v4(
    frame: HotFrameV3<'_, '_>,
    rent: &Rent,
    descriptor: &CapabilityProgramV4,
    strategy: &AuthenticatedExecutionStrategyV2,
    request: AcceleratorRequestV2<'_>,
    family_request: &[u8],
    runtime_account_count: usize,
    scalars: &[u64],
) -> Result<Vec<u32>, ProgramError> {
    let account_profile_data = borrow_finalized_record(
        frame,
        frame.account_profile_raw,
        frame.account_profile_staging,
        rent,
        descriptor.account_profile().schema().to_bytes(),
        descriptor.account_profile().program().to_bytes(),
    )?;
    if descriptor.account_profile().schema().to_bytes() != ACCOUNT_PROFILE_SCHEMA_ID_V2 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let account_profile =
        AccountProfileV2::decode(&account_profile_data).map_err(|_| TradingSbfError::Content)?;
    let request_profile_data = borrow_finalized_record(
        frame,
        frame.request_profile_raw,
        frame.request_profile_staging,
        rent,
        descriptor.request_profile().schema().to_bytes(),
        descriptor.request_profile().program().to_bytes(),
    )?;
    let request_profile = decode_request_profile(*descriptor, &request_profile_data)?;
    let transition_data = borrow_finalized_record(
        frame,
        frame.transition_raw,
        frame.transition_staging,
        rent,
        descriptor.transition().schema().to_bytes(),
        descriptor.transition().program().to_bytes(),
    )?;
    if descriptor.transition().schema().to_bytes() != TRANSITION_SCHEMA_ID_V3
        || strategy.strategy().transition_schema() != descriptor.transition().schema()
        || strategy.strategy().transition_program() != descriptor.transition().program()
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let transition =
        TransitionProgramV3::decode(&transition_data).map_err(|_| TradingSbfError::Content)?;
    let effect_data = borrow_finalized_record(
        frame,
        frame.effect_raw,
        frame.effect_staging,
        rent,
        descriptor.effect().schema().to_bytes(),
        descriptor.effect().program().to_bytes(),
    )?;
    let effect = decode_selected_effect_v4(descriptor.effect().schema().to_bytes(), &effect_data)?;
    let lifecycle_data = borrow_finalized_record(
        frame,
        frame.lifecycle_raw,
        frame.lifecycle_staging,
        rent,
        descriptor.lifecycle().schema().to_bytes(),
        descriptor.lifecycle().program().to_bytes(),
    )?;
    if descriptor.lifecycle().schema().to_bytes() != SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5
        || descriptor.derivation_policy() != descriptor.lifecycle().program()
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    StateLifecyclePolicyV5::decode_selected(
        descriptor.lifecycle().program().to_bytes(),
        hash(&lifecycle_data).to_bytes(),
        &lifecycle_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let mut span_widths = if account_profile.uses_dynamic_fixed_spans() {
        vec![0_u32; usize::from(account_profile.dynamic_fixed_span_count())]
    } else {
        Vec::new()
    };
    if account_profile.uses_dynamic_fixed_spans() {
        account_profile
            .dynamic_span_widths_from_scalars(scalars, &mut span_widths)
            .map_err(|_| TradingSbfError::Content)?;
    }
    let logical_count = if account_profile.uses_dynamic_fixed_spans() {
        account_profile
            .logical_account_count_with_dynamic_spans(request.tail_count(), &span_widths)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        account_profile
            .logical_account_count(request.tail_count())
            .map_err(|_| TradingSbfError::Content)?
    };
    if logical_count != runtime_account_count {
        return Err(TradingSbfError::Content.into());
    }
    require_geometry(
        account_profile,
        request_profile,
        transition,
        effect,
        request.tail_count(),
        family_request,
        runtime_account_count,
        &span_widths,
        scalars,
    )?;
    Ok(span_widths)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_accelerator_context_v4<'accounts, 'info>(
    accelerator_program: &Pubkey,
    frame: HotFrameV3<'accounts, 'info>,
    envelope: HotExecutionEnvelopeV3,
    family_context: TradingFamilyContextV1,
    selected_action: u32,
    descriptor: &CapabilityProgramV4,
    strategy: &AuthenticatedExecutionStrategyV2,
    product_runtime: &AuthenticatedProductRuntimeV3<'accounts, 'info>,
    request: AcceleratorRequestV2<'_>,
    family_request: &[u8],
    runtime_accounts: &[AccountInfo<'info>],
    root_prestate: [u8; 32],
) -> Result<Box<AdmittedInvocationContextV3>, ProgramError> {
    let runtime_observations_digest = accelerator_runtime_observations_digest_v4(
        runtime_accounts,
        family_context.selection().config().to_bytes(),
        product_runtime
            .runtime
            .product_record
            .content_digest
            .to_bytes(),
        product_runtime
            .runtime
            .portfolio_record
            .content_digest
            .to_bytes(),
        product_runtime
            .linked_basis_record
            .content_digest
            .to_bytes(),
    )?;
    let context = Box::new(AdmittedInvocationContextV3 {
        release_set: family_context.release_set(),
        market: ContentId::new(envelope.market()).map_err(|_| TradingSbfError::Content)?,
        root: ContentId::new(frame.root.key.to_bytes()).map_err(|_| TradingSbfError::Content)?,
        registry_program: ContentId::new(frame.registry.key.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        trading_program: ContentId::new(frame.trading_program.key.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        accelerator_program: ContentId::new(accelerator_program.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        capability_program: strategy.capability_program_id(),
        account_profile: descriptor.account_profile().program(),
        request_profile: descriptor.request_profile().program(),
        transition: strategy.strategy().transition_program(),
        effect: descriptor.effect().program(),
        lifecycle: descriptor.derivation_policy(),
        strategy: strategy.strategy_program_id(),
        certificate: strategy
            .certificate_program_id()
            .ok_or(TradingSbfError::Content)?,
        admission: strategy
            .admission_program_id()
            .ok_or(TradingSbfError::Content)?,
        artifact_release: strategy
            .artifact_release_id()
            .ok_or(TradingSbfError::Content)?,
        config: family_context.selection().config(),
        product: ContentId::new(
            product_runtime
                .runtime
                .product_record
                .content_digest
                .to_bytes(),
        )
        .map_err(|_| TradingSbfError::Content)?,
        portfolio: ContentId::new(
            product_runtime
                .runtime
                .portfolio_record
                .content_digest
                .to_bytes(),
        )
        .map_err(|_| TradingSbfError::Content)?,
        linked_basis: ContentId::new(
            product_runtime
                .linked_basis_record
                .content_digest
                .to_bytes(),
        )
        .map_err(|_| TradingSbfError::Content)?,
        family_request_digest: ContentId::new(hash(family_request).to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        runtime_observations_digest,
        root_prestate_digest: ContentId::new(root_prestate)
            .map_err(|_| TradingSbfError::Content)?,
        selected_action,
        tail_count: request.tail_count(),
        account_count: u32::try_from(runtime_accounts.len())
            .map_err(|_| TradingSbfError::Content)?,
        scalar_count: request.scalar_count(),
        identity_count: request.identity_count(),
    });
    if admitted_invocation_context_digest_v3(*context).map_err(|_| TradingSbfError::Content)?
        != request.invocation_context()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(context)
}

fn authenticate_accelerator_top_level_v4(
    frame: HotFrameV3<'_, '_>,
    strategy_evidence: &[AccountInfo<'_>],
    caller_authority: &AccountInfo<'_>,
    request: AcceleratorRequestV2<'_>,
) -> Result<Vec<u8>, ProgramError> {
    let (_, observed) = load_current_top_level_instruction(frame.instructions)?;
    let (hot_instruction, fixed_start, strategy_start, caller_start, registry_mode) = if observed
        .program_id
        == *frame.trading_program.key
    {
        (
            observed.data.clone(),
            0_usize,
            HOT_FIXED_ACCOUNT_COUNT_V3,
            HOT_FIXED_ACCOUNT_COUNT_V3
                .checked_add(ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4)
                .ok_or(TradingSbfError::Content)?,
            false,
        )
    } else if observed.program_id == *frame.registry.key {
        let (envelope, _) = HotExecutionEnvelopeV3::split_instruction(&observed.data)
            .map_err(|_| TradingSbfError::NativeSignature)?;
        let activation_digest = {
            let data = frame
                .activation_cache
                .try_borrow_data()
                .map_err(|_| TradingSbfError::NativeSignature)?;
            ContentId::new(hash(&data).to_bytes()).map_err(|_| TradingSbfError::NativeSignature)?
        };
        let continuation = RegistryContinuationRequestV1::new_core_trading_hot(
            ContentId::new(envelope.release_set()).map_err(|_| TradingSbfError::NativeSignature)?,
            activation_digest,
            ContentId::new(hash(&observed.data).to_bytes())
                .map_err(|_| TradingSbfError::NativeSignature)?,
            u32::try_from(observed.data.len()).map_err(|_| TradingSbfError::NativeSignature)?,
        )
        .map_err(|_| TradingSbfError::NativeSignature)?;
        let outer = observed
            .accounts
            .get(..REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1)
            .ok_or(TradingSbfError::NativeSignature)?;
        let expected_outer = [
            frame.activation_cache.key,
            frame.core_program.key,
            frame.core_programdata.key,
            frame.trading_program.key,
            frame.trading_programdata.key,
        ];
        if outer
            .iter()
            .take(expected_outer.len())
            .zip(expected_outer)
            .any(|(meta, key)| meta.pubkey != *key || meta.is_signer || meta.is_writable)
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
        let batch = continuation
            .role_batch_request()
            .map_err(|_| TradingSbfError::NativeSignature)?;
        let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
            .map_err(|_| TradingSbfError::NativeSignature)?;
        let seeds = RegistryContinuationAdmissionSeedsV1::new(
            continuation,
            frame.activation_cache.key.to_bytes(),
            batch_digest,
        )
        .map_err(|_| TradingSbfError::NativeSignature)?;
        let release = seeds.release_set();
        let cache = seeds.activation_cache();
        let batch = seeds.batch_request_digest();
        let mask = seeds.role_mask();
        let role = seeds.continuation_role();
        let digest = seeds.continuation_digest();
        let expected_admission = Pubkey::find_program_address(
            &[
                seeds.domain(),
                release.as_slice(),
                cache.as_slice(),
                batch.as_slice(),
                mask.as_slice(),
                role.as_slice(),
                digest.as_slice(),
            ],
            frame.registry.key,
        )
        .0;
        let admission_meta = outer.get(5).ok_or(TradingSbfError::NativeSignature)?;
        if admission_meta.pubkey != expected_admission
            || admission_meta.is_signer
            || admission_meta.is_writable
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
        let fixed_start = REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1;
        let strategy_start = fixed_start
            .checked_add(HOT_FIXED_ACCOUNT_COUNT_V3)
            .and_then(|value| value.checked_add(1))
            .ok_or(TradingSbfError::Content)?;
        let caller_start = strategy_start
            .checked_add(ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4)
            .ok_or(TradingSbfError::Content)?;
        (
            observed.data.clone(),
            fixed_start,
            strategy_start,
            caller_start,
            true,
        )
    } else {
        return Err(TradingSbfError::NativeSignature.into());
    };
    let (envelope, _) = HotExecutionEnvelopeV3::split_instruction(&hot_instruction)
        .map_err(|_| TradingSbfError::NativeSignature)?;
    let fixed_metas = observed
        .accounts
        .get(fixed_start..fixed_start + HOT_FIXED_ACCOUNT_COUNT_V3)
        .ok_or(TradingSbfError::NativeSignature)?;
    let fixed_accounts = [
        frame.market,
        frame.root,
        frame.manifest_raw,
        frame.manifest_staging,
        frame.program_set_raw,
        frame.program_set_staging,
        frame.descriptor_raw,
        frame.descriptor_staging,
        frame.config_raw,
        frame.config_staging,
        frame.account_profile_raw,
        frame.account_profile_staging,
        frame.request_profile_raw,
        frame.request_profile_staging,
        frame.transition_raw,
        frame.transition_staging,
        frame.effect_raw,
        frame.effect_staging,
        frame.lifecycle_raw,
        frame.lifecycle_staging,
        frame.strategy_raw,
        frame.strategy_staging,
        frame.activation_cache,
        frame.core_program,
        frame.core_programdata,
        frame.trading_program,
        frame.trading_programdata,
        frame.registry,
        frame.rent,
        frame.instructions,
        frame.product_raw,
        frame.product_staging,
        frame.result_domain_raw,
        frame.result_domain_staging,
        frame.portfolio_raw,
        frame.portfolio_staging,
        frame.linked_basis_raw,
        frame.linked_basis_staging,
    ];
    for (index, (meta, info)) in fixed_metas.iter().zip(fixed_accounts).enumerate() {
        let expected_writable =
            index == HOT_ROOT_ACCOUNT_V3 || (registry_mode && index == HOT_MARKET_ACCOUNT_V3);
        if meta.pubkey != *info.key || meta.is_signer || meta.is_writable != expected_writable {
            return Err(TradingSbfError::NativeSignature.into());
        }
    }
    let strategy_metas = observed
        .accounts
        .get(
            strategy_start
                ..strategy_start
                    .checked_add(ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4)
                    .ok_or(TradingSbfError::Content)?,
        )
        .ok_or(TradingSbfError::NativeSignature)?;
    if strategy_metas
        .iter()
        .zip(strategy_evidence)
        .any(|(meta, info)| meta.pubkey != *info.key || meta.is_signer || meta.is_writable)
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let caller_count =
        admitted_caller_authority_count_v3(request.scalar_count(), request.identity_count())?;
    if caller_count
        != usize::try_from(request.chunk_count()).map_err(|_| TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    let caller_index = caller_start
        .checked_add(usize::try_from(request.chunk_index()).map_err(|_| TradingSbfError::Content)?)
        .ok_or(TradingSbfError::Content)?;
    let caller_meta = observed
        .accounts
        .get(caller_index)
        .ok_or(TradingSbfError::NativeSignature)?;
    if caller_meta.pubkey != *caller_authority.key
        || caller_meta.is_signer
        || caller_meta.is_writable
        || caller_start
            .checked_add(caller_count)
            .ok_or(TradingSbfError::Content)?
            > observed.accounts.len()
        || envelope.market() == [0; 32]
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    Ok(hot_instruction)
}

fn authenticate_accelerator_activation_v4(
    frame: HotFrameV3<'_, '_>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<(AuthenticatedRoleReceiptV1, ContentId, ContentId), ProgramError> {
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &envelope.release_set()],
        frame.registry.key,
    )
    .0;
    if frame.activation_cache.key != &expected_cache
        || frame.activation_cache.owner != frame.registry.key
        || frame.activation_cache.is_signer
        || frame.activation_cache.is_writable
        || frame.activation_cache.executable
    {
        return Err(TradingSbfError::Release.into());
    }
    let data = frame
        .activation_cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| TradingSbfError::Release)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| TradingSbfError::Release)?
        .to_bytes()
        != envelope.release_set()
    {
        return Err(TradingSbfError::Release.into());
    }
    let core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|_| TradingSbfError::Release)?;
    let trading = activated
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| TradingSbfError::Release)?;
    let claims = activated
        .role(ExecutionRoleV1::Claims)
        .map_err(|_| TradingSbfError::Release)?;
    let custody = activated
        .role(ExecutionRoleV1::Custody)
        .map_err(|_| TradingSbfError::Release)?;
    drop(data);
    // Both releases come from the Registry activation cache, whose activation
    // already authenticated a chain-observed complete-ELF digest for each role.
    authenticate_activated_current_deployment(
        core.release(),
        frame.core_program,
        frame.core_programdata,
    )
    .map_err(ProgramError::from)?;
    authenticate_activated_current_deployment(
        trading.release(),
        frame.trading_program,
        frame.trading_programdata,
    )
    .map_err(ProgramError::from)?;
    Ok((
        AuthenticatedRoleReceiptV1::new(
            ExecutionRoleV1::Trading,
            ContentId::new(envelope.release_set()).map_err(|_| TradingSbfError::Release)?,
            trading.release().program(),
            trading.artifact_release_id(),
            trading.release().semantic_release_id(),
        ),
        ContentId::new(claims.release().program().to_bytes())
            .map_err(|_| TradingSbfError::Release)?,
        ContentId::new(custody.release().program().to_bytes())
            .map_err(|_| TradingSbfError::Release)?,
    ))
}

fn authenticate_accelerator_input_bank_v4(
    request: AcceleratorRequestV2<'_>,
    runtime_accounts: &[AccountInfo<'_>],
    trading_program: &Pubkey,
) -> Result<Vec<u8>, ProgramError> {
    let bank = match request.transport() {
        RequestTransportV2::Inline => request.inline_bank().to_vec(),
        RequestTransportV2::ScratchPages => {
            let page_count =
                usize::try_from(request.chunk_count()).map_err(|_| TradingSbfError::Content)?;
            let mut pages = vec![None; page_count];
            for account in runtime_accounts {
                if account.owner != trading_program
                    || account.is_signer
                    || account.is_writable
                    || account.executable
                    || runtime_accounts
                        .iter()
                        .filter(|runtime| runtime.key == account.key)
                        .count()
                        != 1
                {
                    continue;
                }
                let data = account
                    .try_borrow_data()
                    .map_err(|_| TradingSbfError::Content)?;
                let Ok(page) = AuthenticatedScratchPageV2::decode(&data) else {
                    continue;
                };
                page.validate_request_input(
                    ContentId::new(trading_program.to_bytes())
                        .map_err(|_| TradingSbfError::Content)?,
                    request,
                )
                .map_err(|_| TradingSbfError::Content)?;
                let index =
                    usize::try_from(page.chunk_index()).map_err(|_| TradingSbfError::Content)?;
                let slot = pages.get_mut(index).ok_or(TradingSbfError::Content)?;
                if slot.is_some() {
                    return Err(TradingSbfError::Content.into());
                }
                *slot = Some((page.chunk_offset(), page.payload().to_vec()));
            }
            let mut bank = Vec::with_capacity(
                usize::try_from(request.total_bank_bytes())
                    .map_err(|_| TradingSbfError::Content)?,
            );
            for (index, page) in pages.into_iter().enumerate() {
                let (offset, payload) = page.ok_or(TradingSbfError::Content)?;
                if usize::try_from(offset).map_err(|_| TradingSbfError::Content)? != bank.len()
                    || index >= page_count
                {
                    return Err(TradingSbfError::Content.into());
                }
                bank.extend_from_slice(&payload);
            }
            bank
        }
    };
    if u64::try_from(bank.len()).map_err(|_| TradingSbfError::Content)?
        != request.total_bank_bytes()
        || ContentId::new(hash(&bank).to_bytes()).map_err(|_| TradingSbfError::Content)?
            != request.input_bank_digest()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(bank)
}

fn decode_accelerator_register_bank_v4(
    request: AcceleratorRequestV2<'_>,
    bank: &[u8],
) -> Result<(Vec<u64>, Vec<[u8; 32]>), ProgramError> {
    let expected = register_bank_bytes_v2(request.scalar_count(), request.identity_count())
        .map_err(|_| TradingSbfError::Content)?;
    if usize::try_from(expected).map_err(|_| TradingSbfError::Content)? != bank.len() {
        return Err(TradingSbfError::Content.into());
    }
    let scalar_bytes = usize::try_from(request.scalar_count())
        .map_err(|_| TradingSbfError::Content)?
        .checked_mul(8)
        .ok_or(TradingSbfError::Content)?;
    let mut scalars = Vec::with_capacity(
        usize::try_from(request.scalar_count()).map_err(|_| TradingSbfError::Content)?,
    );
    for bytes in bank
        .get(..scalar_bytes)
        .ok_or(TradingSbfError::Content)?
        .chunks_exact(8)
    {
        scalars.push(u64::from_le_bytes(
            bytes.try_into().map_err(|_| TradingSbfError::Content)?,
        ));
    }
    let identities = bank
        .get(scalar_bytes..)
        .ok_or(TradingSbfError::Content)?
        .chunks_exact(32)
        .map(|bytes| bytes.try_into().map_err(|_| TradingSbfError::Content))
        .collect::<Result<Vec<[u8; 32]>, _>>()?;
    if scalars.len()
        != usize::try_from(request.scalar_count()).map_err(|_| TradingSbfError::Content)?
        || identities.len()
            != usize::try_from(request.identity_count()).map_err(|_| TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok((scalars, identities))
}

fn accelerator_runtime_observations_digest_v4(
    runtime_accounts: &[AccountInfo<'_>],
    selected_config: [u8; 32],
    product_root: [u8; 32],
    portfolio: [u8; 32],
    linked_basis: [u8; 32],
) -> Result<ContentId, ProgramError> {
    let projected = LogicalProjectionKeysV3 {
        selected_config,
        product_root,
        portfolio,
        linked_basis,
    };
    // Exact capacity, not `collect::<Result<Vec<_>, _>>()`. A fallible collect
    // reports a zero lower bound, so the SBF bump allocator - which never frees
    // - is walked through the whole doubling ladder and charges several times
    // the live width for every fallible bank on this path.
    let mut runtime_data = Vec::with_capacity(runtime_accounts.len());
    for account in runtime_accounts.iter() {
        runtime_data.push(
            account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?,
        );
    }
    let observations = runtime_accounts
        .iter()
        .zip(&runtime_data)
        .enumerate()
        .map(|(coordinate, (account, data))| ShadowRuntimeObservationV3 {
            key: *logical_projection_key_v3(coordinate, account.key, &projected),
            owner: account.owner.to_bytes(),
            lamports: account.lamports(),
            data: data.as_ref(),
            signer: false,
            writable: false,
            executable: account.executable,
        })
        .collect::<Vec<_>>();
    runtime_observations_digest_v3(&observations).map_err(|_| TradingSbfError::Content.into())
}

fn authenticate_accelerator_caller_authority_v4(
    trading_program: &Pubkey,
    caller_authority: &AccountInfo<'_>,
    envelope: HotExecutionEnvelopeV3,
    root: &Pubkey,
    request_bytes: &[u8],
) -> Result<(), ProgramError> {
    let request_digest =
        ContentId::new(hash(request_bytes).to_bytes()).map_err(|_| TradingSbfError::Release)?;
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(envelope.release_set()).map_err(|_| TradingSbfError::Release)?,
        envelope.market(),
        ExecutionRoleV1::Trading,
        root.to_bytes(),
        request_digest.to_bytes(),
    )
    .map_err(|_| TradingSbfError::Release)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), trading_program).0;
    if caller_authority.key != &expected
        || !caller_authority.is_signer
        || caller_authority.is_writable
        || caller_authority.executable
    {
        Err(TradingSbfError::Release.into())
    } else {
        Ok(())
    }
}

const REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1: usize = 6;

/// Invocation facts authenticated from the current top-level instruction.
///
/// Registry continuation mode inserts one ephemeral admission signer before
/// strategy extras. It also permits physical privilege union on the fixed
/// Market observation; AccountProfile still owns the exact logical downgrade.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AuthenticatedHotInvocationV3 {
    current_instruction: u16,
    native_message_offset_bias: u16,
    strategy_extras_start: usize,
    permits_fixed_market_union: bool,
    role_authentication: HotRoleAuthenticationV3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HotRoleAuthenticationV3 {
    ReauthenticateRegistry,
    AuthenticatedContinuation,
}

#[inline(never)]
fn authenticate_hot_invocation_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    envelope: HotExecutionEnvelopeV3,
) -> Result<AuthenticatedHotInvocationV3, ProgramError> {
    let instructions = account(accounts, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)?;
    let (current_instruction, observed) = load_current_top_level_instruction(instructions)?;
    if observed.program_id == *program_id {
        if observed.data.as_slice() != instruction_data
            || observed.accounts.len() != accounts.len()
            || observed.accounts.iter().zip(accounts).any(|(meta, info)| {
                meta.pubkey != *info.key
                    || meta.is_signer != info.is_signer
                    || meta.is_writable != info.is_writable
            })
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
        return Ok(AuthenticatedHotInvocationV3 {
            current_instruction,
            native_message_offset_bias: 0,
            strategy_extras_start: HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3,
            permits_fixed_market_union: false,
            role_authentication: HotRoleAuthenticationV3::ReauthenticateRegistry,
        });
    }

    let registry = account(accounts, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?;
    if observed.program_id != *registry.key {
        return Err(TradingSbfError::NativeSignature.into());
    }
    if observed.data.as_slice() != instruction_data {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let activation = account(accounts, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?;
    let activation_digest = {
        let bytes = activation
            .try_borrow_data()
            .map_err(|_| TradingSbfError::NativeSignature)?;
        ContentId::new(hash(&bytes).to_bytes()).map_err(|_| TradingSbfError::NativeSignature)?
    };
    let hot_digest =
        ContentId::new(hash(instruction_data).to_bytes()).map_err(|_| TradingSbfError::Content)?;
    let request = RegistryContinuationRequestV1::new_core_trading_hot(
        ContentId::new(envelope.release_set()).map_err(|_| TradingSbfError::Content)?,
        activation_digest,
        hot_digest,
        u32::try_from(instruction_data.len()).map_err(|_| TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::NativeSignature)?;

    let admission = account(accounts, HOT_FIXED_ACCOUNT_COUNT_V3)?;
    if !admission.is_signer
        || admission.is_writable
        || admission.executable
        || admission.owner != &system_program::ID
        || !admission.data_is_empty()
        || admission.lamports() != 0
        || accounts
            .iter()
            .filter(|info| info.key == admission.key)
            .count()
            != 1
    {
        return Err(TradingSbfError::Release.into());
    }
    let batch = request
        .role_batch_request()
        .map_err(|_| TradingSbfError::NativeSignature)?;
    let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
        .map_err(|_| TradingSbfError::NativeSignature)?;
    let seeds =
        RegistryContinuationAdmissionSeedsV1::new(request, activation.key.to_bytes(), batch_digest)
            .map_err(|_| TradingSbfError::NativeSignature)?;
    let release = seeds.release_set();
    let cache = seeds.activation_cache();
    let batch = seeds.batch_request_digest();
    let mask = seeds.role_mask();
    let role = seeds.continuation_role();
    let digest = seeds.continuation_digest();
    let expected_admission = Pubkey::find_program_address(
        &[
            seeds.domain(),
            release.as_slice(),
            cache.as_slice(),
            batch.as_slice(),
            mask.as_slice(),
            role.as_slice(),
            digest.as_slice(),
        ],
        registry.key,
    )
    .0;
    if expected_admission != *admission.key {
        return Err(TradingSbfError::Release.into());
    }

    let outer = observed
        .accounts
        .get(..REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1)
        .ok_or(TradingSbfError::NativeSignature)?;
    let expected_outer = [
        account(accounts, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?.key,
        account(accounts, HOT_CORE_PROGRAM_ACCOUNT_V3)?.key,
        account(accounts, HOT_CORE_PROGRAMDATA_ACCOUNT_V3)?.key,
        account(accounts, HOT_TRADING_PROGRAM_ACCOUNT_V3)?.key,
        account(accounts, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3)?.key,
        admission.key,
    ];
    if outer
        .iter()
        .zip(expected_outer)
        .any(|(meta, key)| meta.pubkey != *key || meta.is_signer || meta.is_writable)
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let observed_nested = observed
        .accounts
        .get(REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1..)
        .ok_or(TradingSbfError::NativeSignature)?;
    if observed_nested.len() != accounts.len()
        || observed_nested
            .iter()
            .zip(accounts)
            .enumerate()
            .any(|(index, (meta, info))| {
                meta.pubkey != *info.key
                    || meta.is_writable != info.is_writable
                    || if index == HOT_FIXED_ACCOUNT_COUNT_V3 {
                        meta.is_signer
                    } else {
                        meta.is_signer != info.is_signer
                    }
            })
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    Ok(AuthenticatedHotInvocationV3 {
        current_instruction,
        native_message_offset_bias: 0,
        strategy_extras_start: HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3
            .checked_add(1)
            .ok_or(TradingSbfError::Content)?,
        permits_fixed_market_union: true,
        role_authentication: HotRoleAuthenticationV3::AuthenticatedContinuation,
    })
}

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

    let market = authenticate_market_boxed_v3(&frame, envelope)?;
    let root = authenticate_root_boxed_v3(
        program_id,
        &frame,
        envelope,
        &market,
        invocation.role_authentication,
    )?;
    let context = &root.context;
    let immutable_root_header = &root.immutable_header;

    let rent = Rent::from_account_info(frame.rent).map_err(|_| TradingSbfError::Content)?;
    let product_runtime_v3 = authenticate_product_runtime_boxed_v3(&frame, &rent, &market)?;
    let product_runtime = product_runtime_v3.runtime;
    let product_outcome_count = product_runtime.outcome_count;
    hot_cu_checkpoint!("root-product");
    let manifest_data = borrow_finalized_record(
        *frame,
        frame.manifest_raw,
        frame.manifest_staging,
        &rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        context.selection().manifest().to_bytes(),
    )?;
    let entry = authenticate_manifest_entry_boxed_v3(&manifest_data, context)?;

    let capability_release = context.selection().capability_release().to_bytes();
    let program_set_data = borrow_finalized_record(
        *frame,
        frame.program_set_raw,
        frame.program_set_staging,
        &rent,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        capability_release,
    )?;
    // The record's own authentication is the single owner of its content
    // digest: `borrow_finalized_record` refuses unless `hash(program_set_data)`
    // is exactly `capability_release`, so the selected identity and the
    // authenticated digest are one value and hashing the record again here only
    // recomputed it.
    let program_set = CapabilityProgramSetV2::decode_selected(
        capability_release,
        capability_release,
        &program_set_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let selected_entry = program_set
        .select_entry(family_request)
        .map_err(|_| TradingSbfError::Content)?;
    let selected_descriptor = selected_entry.descriptor();
    if selected_descriptor.schema().to_bytes() != PROGRAM_SCHEMA_ID_V4 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let selected_program = selected_descriptor.program();
    let selected_action = selected_entry.selector();

    let descriptor_data = borrow_finalized_record(
        *frame,
        frame.descriptor_raw,
        frame.descriptor_staging,
        &rent,
        selected_descriptor.schema().to_bytes(),
        selected_program.to_bytes(),
    )?;
    if descriptor_data.len() != CAPABILITY_PROGRAM_V4_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let descriptor = decode_capability_program_boxed_v3(&descriptor_data)?;
    authenticate_descriptor_root_selection(&descriptor, context, &entry)?;

    let config_data = borrow_finalized_record(
        *frame,
        frame.config_raw,
        frame.config_staging,
        &rent,
        descriptor.config_schema().to_bytes(),
        context.selection().config().to_bytes(),
    )?;
    // The config record needs no digest of its own here either: the borrow
    // above refuses unless `hash(config_data)` is the selected config identity,
    // so re-hashing it and comparing the result against that identity could
    // only ever agree.
    drop(config_data);
    require_common_projection_bindings_v3(CommonProjectionBindingsV3 {
        selected_config: context.selection().config().to_bytes(),
        selected_product_record: market.identity.product_record.to_bytes(),
        authenticated_product_record: product_runtime.product_record.content_digest.to_bytes(),
        market_product: market.identity.product_id.to_bytes(),
        runtime_product: product_runtime.product_id.to_bytes(),
        product_semantic_basis: product_runtime.liability_basis_id.to_bytes(),
        authenticated_semantic_basis: product_runtime_v3.semantic_basis_id.to_bytes(),
        authenticated_linked_basis: product_runtime_v3
            .linked_basis_record
            .content_digest
            .to_bytes(),
    })?;
    let lifecycle_data = borrow_finalized_record(
        *frame,
        frame.lifecycle_raw,
        frame.lifecycle_staging,
        &rent,
        descriptor.lifecycle().schema().to_bytes(),
        descriptor.lifecycle().program().to_bytes(),
    )?;
    if descriptor.lifecycle().schema().to_bytes() != SELECTED_LIFECYCLE_SCHEMA_RELEASE_ID_V5
        || descriptor.derivation_policy() != descriptor.lifecycle().program()
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    // Same single owner as the program set above: the borrow already proved
    // `hash(lifecycle_data)` is the selected lifecycle program identity.
    let selected_lifecycle = descriptor.lifecycle().program().to_bytes();
    let lifecycle = StateLifecyclePolicyV5::decode_selected(
        selected_lifecycle,
        selected_lifecycle,
        &lifecycle_data,
    )
    .map_err(|_| TradingSbfError::Content)?;

    let account_profile_data = borrow_finalized_record(
        *frame,
        frame.account_profile_raw,
        frame.account_profile_staging,
        &rent,
        descriptor.account_profile().schema().to_bytes(),
        descriptor.account_profile().program().to_bytes(),
    )?;
    if descriptor.account_profile().schema().to_bytes() != ACCOUNT_PROFILE_SCHEMA_ID_V2 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let account_profile =
        AccountProfileV2::decode(&account_profile_data).map_err(|_| TradingSbfError::Content)?;
    lifecycle
        .validate_account_profile(account_profile)
        .map_err(|_| TradingSbfError::Content)?;

    let request_profile_data = borrow_finalized_record(
        *frame,
        frame.request_profile_raw,
        frame.request_profile_staging,
        &rent,
        descriptor.request_profile().schema().to_bytes(),
        descriptor.request_profile().program().to_bytes(),
    )?;
    let request_profile = decode_request_profile(*descriptor, &request_profile_data)?;

    let (strategy, strategy_extras_end) = authenticate_strategy_boxed_v3(
        &frame,
        accounts,
        *context,
        selected_descriptor.schema(),
        selected_program,
        invocation.strategy_extras_start,
    )?;
    let strategy_extras = accounts
        .get(invocation.strategy_extras_start..strategy_extras_end)
        .ok_or(TradingSbfError::Content)?;

    let transition_data = borrow_finalized_record(
        *frame,
        frame.transition_raw,
        frame.transition_staging,
        &rent,
        descriptor.transition().schema().to_bytes(),
        descriptor.transition().program().to_bytes(),
    )?;
    if descriptor.transition().schema().to_bytes() != TRANSITION_SCHEMA_ID_V3
        || strategy.strategy().transition_schema() != descriptor.transition().schema()
        || strategy.strategy().transition_program() != descriptor.transition().program()
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let transition =
        TransitionProgramV3::decode(&transition_data).map_err(|_| TradingSbfError::Content)?;

    let effect_data = borrow_finalized_record(
        *frame,
        frame.effect_raw,
        frame.effect_staging,
        &rent,
        descriptor.effect().schema().to_bytes(),
        descriptor.effect().program().to_bytes(),
    )?;
    let effect = decode_selected_effect_v4(descriptor.effect().schema().to_bytes(), &effect_data)?;
    hot_cu_checkpoint!("artifacts-strategy-effect");

    let provisional_scalar_count = effect
        .scalar_count(product_outcome_count)
        .map_err(|_| TradingSbfError::Content)?;
    let provisional_identity_count = effect
        .identity_count(product_outcome_count)
        .map_err(|_| TradingSbfError::Content)?;
    let (shadow_caller_authority, admitted_caller_authorities, runtime_start) =
        match strategy.strategy().disposition() {
            StrategyDispositionV2::Interpreted => (None, None, strategy_extras_end),
            StrategyDispositionV2::ShadowAot => {
                let expected = HOT_SHADOW_CALLER_AUTHORITY_ACCOUNT_V3
                    .checked_add(
                        invocation
                            .strategy_extras_start
                            .checked_sub(HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3)
                            .ok_or(TradingSbfError::Content)?,
                    )
                    .ok_or(TradingSbfError::Content)?;
                if strategy_extras_end != expected {
                    return Err(TradingSbfError::Content.into());
                }
                let caller = accounts
                    .get(strategy_extras_end)
                    .ok_or(TradingSbfError::Content)?;
                (
                    Some(caller),
                    None,
                    strategy_extras_end
                        .checked_add(1)
                        .ok_or(TradingSbfError::Content)?,
                )
            }
            StrategyDispositionV2::AdmittedAot => {
                let admitted_start = HOT_ADMITTED_CALLER_AUTHORITIES_START_V3
                    .checked_add(
                        invocation
                            .strategy_extras_start
                            .checked_sub(HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3)
                            .ok_or(TradingSbfError::Content)?,
                    )
                    .ok_or(TradingSbfError::Content)?;
                if strategy_extras_end != admitted_start {
                    return Err(TradingSbfError::Content.into());
                }
                let runtime_start = admitted_start
                    .checked_add(admitted_caller_authority_count_v3(
                        u32::try_from(provisional_scalar_count)
                            .map_err(|_| TradingSbfError::Content)?,
                        u32::try_from(provisional_identity_count)
                            .map_err(|_| TradingSbfError::Content)?,
                    )?)
                    .ok_or(TradingSbfError::Content)?;
                let callers = accounts
                    .get(admitted_start..runtime_start)
                    .ok_or(TradingSbfError::Content)?;
                (None, Some(callers), runtime_start)
            }
        };
    let expected_shadow_runtime = HOT_SHADOW_RUNTIME_ACCOUNTS_START_V3
        .checked_add(
            invocation
                .strategy_extras_start
                .checked_sub(HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3)
                .ok_or(TradingSbfError::Content)?,
        )
        .ok_or(TradingSbfError::Content)?;
    if shadow_caller_authority.is_some() && runtime_start != expected_shadow_runtime {
        return Err(TradingSbfError::Content.into());
    }

    let trusted_environment = observe_trusted_environment_v3(account_profile, program_id)?;
    let dynamic_spans = authenticate_dynamic_span_widths_v3(
        account_profile,
        request_profile,
        effect,
        strategy.strategy().disposition(),
        product_outcome_count,
        family_request,
        request_digest,
        trusted_environment,
        provisional_scalar_count,
        provisional_identity_count,
    )?;

    let runtime_accounts = expand_runtime_accounts_v3(
        account_profile,
        product_outcome_count,
        &dynamic_spans.widths,
        [
            frame.root,
            frame.config_raw,
            frame.product_raw,
            frame.portfolio_raw,
            frame.linked_basis_raw,
        ],
        accounts
            .get(runtime_start..)
            .ok_or(TradingSbfError::Content)?,
    )?;
    let input_scratch_pages = authenticated_input_scratch_pages_v3(
        account_profile,
        &dynamic_spans.widths,
        dynamic_spans.transport_span,
        &runtime_accounts,
    )?;
    if runtime_accounts.len() > MAX_HOT_RUNTIME_ACCOUNTS_V3 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    // Exact capacity, not `collect::<Result<Vec<_>, _>>()`. A fallible collect
    // reports a zero lower bound, so the SBF bump allocator - which never frees
    // - is walked through the whole doubling ladder and charges several times
    // the live width for every fallible bank on this path.
    let mut runtime_data = Vec::with_capacity(runtime_accounts.len());
    for account in &runtime_accounts {
        runtime_data.push(
            account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?,
        );
    }
    let tail_count = project_tail_count(account_profile, product_outcome_count)?;
    require_tail_count_agreement_v3(product_outcome_count, tail_count)?;
    // Representatives are resolved before the observation bank because the
    // logical projection key of an aliased coordinate is its representative's,
    // not its own.
    let aliases = representative_coordinates_v3(
        account_profile,
        tail_count,
        &dynamic_spans.widths,
        runtime_accounts.len(),
    )?;
    let projected_keys = Box::new(LogicalProjectionKeysV3 {
        selected_config: context.selection().config().to_bytes(),
        product_root: product_runtime.product_record.content_digest.to_bytes(),
        portfolio: product_runtime.portfolio_record.content_digest.to_bytes(),
        linked_basis: product_runtime_v3
            .linked_basis_record
            .content_digest
            .to_bytes(),
    });
    let selected_config_coordinate = u16::try_from(HOT_SELECTED_CONFIG_LOGICAL_ACCOUNT_V3)
        .map_err(|_| TradingSbfError::Content)?;
    let selected_config_is_variable = account_profile
        .rule(false, selected_config_coordinate)
        .map_err(|_| TradingSbfError::Content)?
        .prestate()
        == AccountPrestateV2::AdapterAuthenticatedVariableData;
    let observations = runtime_accounts
        .iter()
        .zip(&runtime_data)
        .enumerate()
        .map(|(coordinate, (account, data))| {
            let key = logical_projection_key_v3(
                *aliases.get(coordinate).unwrap_or(&coordinate),
                account.key,
                &projected_keys,
            );
            if coordinate == HOT_LINKED_BASIS_LOGICAL_ACCOUNT_V3
                || (coordinate == HOT_SELECTED_CONFIG_LOGICAL_ACCOUNT_V3
                    && selected_config_is_variable)
            {
                // The Product-runtime reader above authenticated Registry
                // finality, schema, content digest, and either the selected
                // immutable config or Product-owned semantic basis before
                // this observation is constructed.
                AccountObservationV1::new_adapter_authenticated_variable_data(
                    key,
                    account.owner.as_array(),
                    account.lamports(),
                    data.as_ref(),
                    account.is_signer,
                    account.is_writable,
                    account.executable,
                )
            } else {
                AccountObservationV1::new(
                    key,
                    account.owner.as_array(),
                    account.lamports(),
                    data.as_ref(),
                    account.is_signer,
                    account.is_writable,
                    account.executable,
                )
            }
        })
        .collect::<Vec<_>>();
    hot_cu_checkpoint!("runtime-observations");

    require_geometry(
        account_profile,
        request_profile,
        transition,
        effect,
        tail_count,
        family_request,
        runtime_accounts.len(),
        &dynamic_spans.widths,
        &dynamic_spans.request_projection_scalars,
    )?;
    let scalar_count = effect
        .scalar_count(tail_count)
        .map_err(|_| TradingSbfError::Content)?;
    let identity_count = effect
        .identity_count(tail_count)
        .map_err(|_| TradingSbfError::Content)?;
    let request_bytes = effect
        .request_bytes(tail_count)
        .map_err(|_| TradingSbfError::Content)?;
    if scalar_count > MAX_HOT_SCALARS_V3
        || identity_count > MAX_HOT_IDENTITIES_V3
        || request_bytes > MAX_HOT_REQUEST_BYTES_V3
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let current_rent_quotes = authenticate_current_rent_quotes_v5(lifecycle, &rent)?;

    let projected_request = project_account_and_request_registers_v3(
        invocation.current_instruction,
        invocation.native_message_offset_bias,
        instruction_data,
        *frame,
        account_profile,
        request_profile,
        lifecycle,
        &current_rent_quotes,
        &dynamic_spans.widths,
        tail_count,
        &observations,
        family_request,
        request_digest,
        trusted_environment,
        product_outcome_count,
        scalar_count,
        identity_count,
    )?;
    let request_output_scalars = projected_request.scalars;
    let request_output_identities = projected_request.identities;
    require_static_register_ownership_v5(StaticRegisterOwnershipV5 {
        account_profile,
        policy: lifecycle,
        action: selected_action,
        request: request_profile,
        transition,
    })?;
    let preplanned_lifecycle = prepare_lifecycle_v4(
        program_id,
        envelope.market(),
        envelope.release_set(),
        envelope.generation(),
        lifecycle,
        selected_action,
        account_profile,
        tail_count,
        &observations,
        &runtime_accounts,
        &request_output_scalars,
        &request_output_identities,
        &rent,
        &aliases,
    )?;
    hot_cu_checkpoint!("request-lifecycle-preplan");

    let candidate = if let Some(caller_authorities) = admitted_caller_authorities {
        execute_admitted_candidate_v3(AdmittedCandidateViewV3 {
            program_id,
            frame: &frame,
            hot_fixed_accounts: accounts
                .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
                .ok_or(TradingSbfError::Content)?,
            caller_authorities,
            strategy_extras,
            runtime_accounts: &runtime_accounts,
            input_scratch_pages,
            observations: &observations,
            envelope,
            context,
            descriptor: &descriptor,
            strategy: &strategy,
            product_runtime_v3: &product_runtime_v3,
            family_request,
            root_prestate,
            selected_program,
            selected_action,
            tail_count,
            scalars: &preplanned_lifecycle.scalars,
            identities: &preplanned_lifecycle.identities,
        })?
    } else {
        execute_interpreted_transition_v3(
            transition,
            tail_count,
            &preplanned_lifecycle.scalars,
            &preplanned_lifecycle.identities,
        )?
    };
    hot_cu_checkpoint!("candidate");
    // The request-profile banks have been copied into the independently
    // prepared lifecycle batch.  End their lifetime before the candidate and
    // Effect phases so the SBF caller frame cannot retain two register-bank
    // owners across the next noinline semantic boundary.
    drop(request_output_scalars);
    drop(request_output_identities);
    let transition_output_scalars = candidate.scalars;
    let transition_output_identities = candidate.identities;
    let admitted_execution_digest = candidate.transcript_digest;
    lifecycle
        .validate_projected_current_rent_quotes(
            account_profile,
            tail_count,
            &transition_output_scalars,
            &current_rent_quotes,
        )
        .map_err(|_| TradingSbfError::Content)?;
    require_trusted_environment_v3(
        trusted_environment,
        &transition_output_scalars,
        &transition_output_identities,
    )?;
    require_dynamic_span_values_v3(
        account_profile,
        &dynamic_spans.widths,
        &transition_output_scalars,
    )?;

    require_borrowed_witness_coverage_v3(
        request_profile,
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        family_request,
    )?;

    let projected_effects = project_hot_effects_v3(
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        &observations,
        &preplanned_lifecycle.plans,
        account_profile,
        &dynamic_spans.widths,
        &aliases,
        runtime_accounts.len(),
        request_bytes,
    )?;
    let output_lamports = projected_effects.lamports;
    let output_requests = projected_effects.requests;

    preflight_local_effects(
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        &aliases,
    )?;
    let revalidated_lifecycle = prepare_lifecycle_v4(
        program_id,
        envelope.market(),
        envelope.release_set(),
        envelope.generation(),
        lifecycle,
        selected_action,
        account_profile,
        tail_count,
        &observations,
        &runtime_accounts,
        &transition_output_scalars,
        &transition_output_identities,
        &rent,
        &aliases,
    )?;
    require_lifecycle_replan_agreement_v4(
        &preplanned_lifecycle,
        &revalidated_lifecycle,
        &transition_output_scalars,
        &transition_output_identities,
    )?;
    require_lifecycle_effect_bindings_v4(
        &revalidated_lifecycle.plans,
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        &aliases,
    )?;
    hot_cu_checkpoint!("effect-lifecycle-replan");
    let lifecycle_plans = revalidated_lifecycle.plans;
    let effect_accounts = downgraded_effect_accounts_v3(
        account_profile,
        tail_count,
        &dynamic_spans.widths,
        &runtime_accounts,
    )?;
    preflight_child_routes_v3(
        program_id,
        *frame,
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        &effect_accounts,
        &output_requests,
        family_request,
        request_digest,
        envelope,
        context.selection().capability_release().to_bytes(),
        selected_program.to_bytes(),
        &aliases,
    )?;
    let strategy_execution_digest = if let Some(caller_authority) = shadow_caller_authority {
        execute_shadow_candidate_v3(ShadowCandidateViewV3 {
            program_id,
            frame: &frame,
            caller_authority,
            strategy_extras,
            runtime_accounts: &runtime_accounts,
            observations: &observations,
            envelope,
            descriptor: &descriptor,
            strategy: &strategy,
            family_request,
            root_prestate,
            selected_program,
            selected_action,
            effect,
            tail_count,
            scalars: &transition_output_scalars,
            identities: &transition_output_identities,
            output_lamports: &output_lamports,
            request_bank: &output_requests,
        })?
    } else {
        admitted_execution_digest
    };
    hot_cu_checkpoint!("children-shadow");
    drop(observations);
    drop(runtime_data);
    hot_cu_checkpoint!("before-commit");
    let commit_status = commit_prepared_hot_v3(Box::new(PreparedHotCommitV3 {
        program_id,
        frame: &frame,
        request_profile,
        effect,
        tail_count,
        scalars: &transition_output_scalars,
        identities: &transition_output_identities,
        runtime_accounts: &runtime_accounts,
        effect_accounts: &effect_accounts,
        request_bank: &output_requests,
        family_request,
        request_digest,
        envelope,
        selected_program,
        lifecycle_plans: &lifecycle_plans,
        aliases: &aliases,
        output_lamports: &output_lamports,
        rent: &rent,
        immutable_root_header,
        root_prestate,
        strategy_execution_digest,
        descriptor: &descriptor,
        strategy: &strategy,
        context,
        market: &market,
        product_runtime_v3: &product_runtime_v3,
        product_outcome_count,
    }));
    hot_cu_checkpoint!("after-commit");
    if commit_status == 0 {
        Ok(())
    } else {
        Err(ProgramError::from(commit_status))
    }
}

struct PreparedHotCommitV3<'a, 'accounts, 'info, 'artifact> {
    program_id: &'a Pubkey,
    frame: &'a HotFrameV3<'accounts, 'info>,
    request_profile: RequestProfileKindV3<'artifact>,
    effect: SelectedEffectProgramV4<'artifact>,
    tail_count: u32,
    scalars: &'a [u64],
    identities: &'a [[u8; 32]],
    runtime_accounts: &'a [&'accounts AccountInfo<'info>],
    effect_accounts: &'a [AccountInfo<'info>],
    request_bank: &'a [u8],
    family_request: &'a [u8],
    request_digest: [u8; 32],
    envelope: HotExecutionEnvelopeV3,
    selected_program: ContentId,
    lifecycle_plans: &'a [PreparedLifecycleInvocationV3],
    aliases: &'a [usize],
    output_lamports: &'a [u64],
    rent: &'a Rent,
    immutable_root_header: &'a [u8; CAPABILITY_ROOT_HEADER_BYTES_V1],
    root_prestate: [u8; 32],
    strategy_execution_digest: [u8; 32],
    descriptor: &'a CapabilityProgramV4,
    strategy: &'a AuthenticatedExecutionStrategyV2,
    context: &'a TradingFamilyContextV1,
    market: &'a CoreState,
    product_runtime_v3: &'a AuthenticatedProductRuntimeV3<'accounts, 'info>,
    product_outcome_count: u32,
}

#[inline(never)]
fn commit_prepared_hot_v3(prepared: Box<PreparedHotCommitV3<'_, '_, '_, '_>>) -> u64 {
    match commit_prepared_hot_result_v3(&prepared) {
        Ok(()) => 0,
        Err(error) => error.into(),
    }
}

/// Keep the wide `ProgramError` return ABI inside the compact commit phase.
/// The outer verifier frame receives one scalar status register instead of an
/// indirect result slot that aliases its last live stack region.
#[inline(never)]
fn commit_prepared_hot_result_v3(
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    apply_lifecycle_creates_v3(
        prepared.program_id,
        prepared.lifecycle_plans,
        prepared.runtime_accounts,
    )?;
    let child_execution_digest = execute_prepared_child_routes_v3(prepared)?;
    commit_prepared_post_children_v3(prepared)?;
    let root_poststate = {
        let bytes = prepared
            .frame
            .root
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Commit)?;
        if bytes.get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            != Some(prepared.immutable_root_header.as_slice())
        {
            return Err(TradingSbfError::Commit.into());
        }
        hash(&bytes).to_bytes()
    };
    finalize_hot_ack_v3(prepared, child_execution_digest, root_poststate)
}

#[inline(never)]
fn execute_prepared_child_routes_v3(
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<[u8; 32], ProgramError> {
    execute_child_routes_v3(
        prepared.program_id,
        *prepared.frame,
        prepared.request_profile,
        prepared.effect,
        prepared.tail_count,
        prepared.scalars,
        prepared.identities,
        prepared.effect_accounts,
        prepared.request_bank,
        prepared.family_request,
        prepared.request_digest,
        prepared.envelope,
        prepared.context.selection().capability_release().to_bytes(),
        prepared.selected_program.to_bytes(),
    )
}

#[inline(never)]
fn commit_prepared_post_children_v3(
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
) -> Result<(), ProgramError> {
    apply_lifecycle_closes_v3(
        prepared.program_id,
        prepared.envelope.market(),
        prepared.envelope.release_set(),
        prepared.envelope.generation(),
        prepared.lifecycle_plans,
        prepared.runtime_accounts,
        prepared.rent,
    )?;
    commit_local_effects(
        prepared.effect,
        prepared.tail_count,
        prepared.scalars,
        prepared.identities,
        prepared.runtime_accounts,
        prepared.aliases,
        prepared.output_lamports,
        prepared.rent,
        false,
    )?;
    commit_local_effects(
        prepared.effect,
        prepared.tail_count,
        prepared.scalars,
        prepared.identities,
        prepared.runtime_accounts,
        prepared.aliases,
        prepared.output_lamports,
        prepared.rent,
        true,
    )?;
    Ok(())
}

#[inline(never)]
fn finalize_hot_ack_v3(
    prepared: &PreparedHotCommitV3<'_, '_, '_, '_>,
    child_execution_digest: [u8; 32],
    root_poststate: [u8; 32],
) -> Result<(), ProgramError> {
    let execution_digest = hashv(&[
        EXECUTION_DIGEST_DOMAIN_V3,
        &prepared.selected_program.to_bytes(),
        &prepared.descriptor.account_profile().program().to_bytes(),
        &prepared.descriptor.request_profile().program().to_bytes(),
        &prepared.strategy.strategy_program_id().to_bytes(),
        &prepared.strategy.strategy().transition_program().to_bytes(),
        &prepared.descriptor.effect().program().to_bytes(),
        &prepared.descriptor.derivation_policy().to_bytes(),
        &prepared.context.selection().config().to_bytes(),
        &prepared.market.identity.product_record.to_bytes(),
        &prepared
            .product_runtime_v3
            .linked_basis_record
            .content_digest
            .to_bytes(),
        &prepared.product_runtime_v3.semantic_basis_id.to_bytes(),
        &prepared.product_outcome_count.to_le_bytes(),
        &prepared.request_digest,
        &prepared.strategy_execution_digest,
        &child_execution_digest,
        &root_poststate,
    ])
    .to_bytes();
    let ack = HotExecutionAckV3::new(HotExecutionAckV3 {
        release_set: prepared.envelope.release_set(),
        market: prepared.envelope.market(),
        generation: prepared.envelope.generation(),
        root: prepared.frame.root.key.to_bytes(),
        request_digest: prepared.request_digest,
        selected_program: prepared.selected_program.to_bytes(),
        root_prestate_digest: prepared.root_prestate,
        root_poststate_digest: root_poststate,
        execution_digest,
    })
    .map_err(|_| TradingSbfError::Commit)?;
    set_return_data(&ack.to_bytes());
    Ok(())
}

struct AuthenticatedRootV3 {
    context: TradingFamilyContextV1,
    immutable_header: [u8; CAPABILITY_ROOT_HEADER_BYTES_V1],
}

#[inline(never)]
fn parse_hot_frame_boxed_v3<'accounts, 'info>(
    program_id: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    permits_fixed_market_union: bool,
) -> Result<Box<HotFrameV3<'accounts, 'info>>, ProgramError> {
    HotFrameV3::parse(program_id, accounts, permits_fixed_market_union).map(Box::new)
}

#[inline(never)]
fn authenticate_market_boxed_v3<'accounts, 'info>(
    frame: &HotFrameV3<'accounts, 'info>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<Box<CoreState>, ProgramError> {
    authenticate_market(*frame, envelope).map(Box::new)
}

#[inline(never)]
fn authenticate_root_boxed_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: &HotFrameV3<'accounts, 'info>,
    envelope: HotExecutionEnvelopeV3,
    market: &CoreState,
    role_authentication: HotRoleAuthenticationV3,
) -> Result<Box<AuthenticatedRootV3>, ProgramError> {
    let trading_receipt = match role_authentication {
        HotRoleAuthenticationV3::ReauthenticateRegistry => {
            let core_receipt = reauthenticate_role(
                *frame,
                ExecutionRoleV1::Core,
                frame.core_program,
                frame.core_programdata,
                envelope.release_set(),
            )?;
            if core_receipt.program().as_bytes() != &frame.core_program.key.to_bytes() {
                return Err(TradingSbfError::Release.into());
            }
            reauthenticate_role(
                *frame,
                ExecutionRoleV1::Trading,
                frame.trading_program,
                frame.trading_programdata,
                envelope.release_set(),
            )?
        }
        HotRoleAuthenticationV3::AuthenticatedContinuation => {
            authenticate_continuation_root_roles_v3(*frame, envelope)?
        }
    };
    let root_data = frame
        .root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    let context = TradingFamilyContextV1::authenticate(
        program_id,
        frame.root.key,
        frame.root.owner,
        &root_data,
        trading_receipt,
    )?;
    let root_header = CapabilityRootHeaderV1::decode(
        root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(TradingSbfError::Root)?,
    )
    .map_err(|_| TradingSbfError::Root)?;
    if context.market() != envelope.market()
        || context.release_set().to_bytes() != envelope.release_set()
        || context.generation() != envelope.generation()
        || market.identity.market_id.to_bytes() != envelope.market()
    {
        return Err(TradingSbfError::Root.into());
    }
    Ok(Box::new(AuthenticatedRootV3 {
        context,
        immutable_header: root_header.to_bytes(),
    }))
}

#[inline(never)]
fn authenticate_continuation_root_roles_v3(
    frame: HotFrameV3<'_, '_>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    let (trading_receipt, _, _) = authenticate_accelerator_activation_v4(frame, envelope)?;
    Ok(trading_receipt)
}

#[inline(never)]
fn authenticate_product_runtime_boxed_v3<'accounts, 'info>(
    frame: &HotFrameV3<'accounts, 'info>,
    rent: &Rent,
    market: &CoreState,
) -> Result<Box<AuthenticatedProductRuntimeV3<'accounts, 'info>>, ProgramError> {
    authenticate_product_runtime_v3(
        frame.registry.key,
        rent,
        ProductContentId::new(market.identity.product_record.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        ProductRuntimeFrameV3 {
            product: ProductRecordFrameV2 {
                raw: frame.product_raw,
                staging: frame.product_staging,
            },
            result_domain: ProductRecordFrameV2 {
                raw: frame.result_domain_raw,
                staging: frame.result_domain_staging,
            },
            portfolio: ProductRecordFrameV2 {
                raw: frame.portfolio_raw,
                staging: frame.portfolio_staging,
            },
            linked_basis: ProductRecordFrameV2 {
                raw: frame.linked_basis_raw,
                staging: frame.linked_basis_staging,
            },
        },
    )
    .map(Box::new)
    .map_err(|_| TradingSbfError::Content.into())
}

#[inline(never)]
fn decode_capability_program_boxed_v3(
    descriptor_data: &[u8],
) -> Result<Box<CapabilityProgramV4>, ProgramError> {
    CapabilityProgramV4::decode(descriptor_data)
        .map(Box::new)
        .map_err(|_| TradingSbfError::Content.into())
}

#[inline(never)]
fn authenticate_manifest_entry_boxed_v3(
    manifest_data: &[u8],
    context: &TradingFamilyContextV1,
) -> Result<Box<dclutch_capability_contract::CapabilityEntryV1>, ProgramError> {
    let manifest =
        CapabilityManifestV1::decode(manifest_data).map_err(|_| TradingSbfError::Content)?;
    let entry = manifest
        .entry(context.selection().entry_index())
        .map_err(|_| TradingSbfError::Content)?;
    if entry.kind_id() != context.selection().kind()
        || entry.release_id() != context.selection().capability_release()
        || entry.config_id() != context.selection().config()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(Box::new(entry))
}

#[inline(never)]
fn authenticate_strategy_boxed_v3<'accounts, 'info>(
    frame: &HotFrameV3<'accounts, 'info>,
    accounts: &'accounts [AccountInfo<'info>],
    context: TradingFamilyContextV1,
    selected_schema: ContentId,
    selected_program: ContentId,
    strategy_extras_start: usize,
) -> Result<(Box<AuthenticatedExecutionStrategyV2>, usize), ProgramError> {
    let strategy_data = frame
        .strategy_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    if strategy_data.len() != EXECUTION_STRATEGY_PROGRAM_BYTES_V2 {
        return Err(TradingSbfError::Content.into());
    }
    let preliminary_strategy =
        ExecutionStrategyProgramV2::decode(&strategy_data).map_err(|_| TradingSbfError::Content)?;
    drop(strategy_data);
    let strategy_account_count = match preliminary_strategy.disposition() {
        StrategyDispositionV2::Interpreted => INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2,
        StrategyDispositionV2::ShadowAot => SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2,
        StrategyDispositionV2::AdmittedAot => ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2,
    };
    let strategy_extra_count = strategy_account_count
        .checked_sub(INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2)
        .ok_or(TradingSbfError::Content)?;
    let strategy_extras_end = strategy_extras_start
        .checked_add(strategy_extra_count)
        .ok_or(TradingSbfError::Content)?;
    let strategy_extras = accounts
        .get(strategy_extras_start..strategy_extras_end)
        .ok_or(TradingSbfError::Content)?;
    let mut strategy_accounts = Vec::with_capacity(strategy_account_count);
    strategy_accounts.extend_from_slice(&[
        frame.descriptor_raw.clone(),
        frame.descriptor_staging.clone(),
        frame.strategy_raw.clone(),
        frame.strategy_staging.clone(),
    ]);
    strategy_accounts.extend_from_slice(strategy_extras);
    let strategy = authenticate_execution_strategy_v2(
        context,
        selected_schema,
        selected_program,
        frame.registry,
        frame.rent,
        &strategy_accounts,
    )?;
    if strategy.strategy().disposition() == StrategyDispositionV2::ShadowAot
        && strategy
            .strategy()
            .transport_profile()
            .map_err(|_| TradingSbfError::Content)?
            != AcceleratorTransportProfileV2::ShadowTranscriptV3
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    if strategy.strategy().disposition() == StrategyDispositionV2::AdmittedAot
        && strategy
            .strategy()
            .transport_profile()
            .map_err(|_| TradingSbfError::Content)?
            != AcceleratorTransportProfileV2::ChunkedBankV2
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok((Box::new(strategy), strategy_extras_end))
}

struct AdmittedCandidateViewV3<'a, 'data, 'accounts, 'info> {
    program_id: &'a Pubkey,
    frame: &'a HotFrameV3<'accounts, 'info>,
    hot_fixed_accounts: &'a [AccountInfo<'info>],
    caller_authorities: &'a [AccountInfo<'info>],
    strategy_extras: &'a [AccountInfo<'info>],
    runtime_accounts: &'a [&'accounts AccountInfo<'info>],
    input_scratch_pages: &'a [&'accounts AccountInfo<'info>],
    observations: &'a [AccountObservationV1<'data>],
    envelope: HotExecutionEnvelopeV3,
    context: &'a TradingFamilyContextV1,
    descriptor: &'a CapabilityProgramV4,
    strategy: &'a AuthenticatedExecutionStrategyV2,
    product_runtime_v3: &'a AuthenticatedProductRuntimeV3<'accounts, 'info>,
    family_request: &'a [u8],
    root_prestate: [u8; 32],
    selected_program: ContentId,
    selected_action: u32,
    tail_count: u32,
    scalars: &'a [u64],
    identities: &'a [[u8; 32]],
}

struct CandidateExecutionV3 {
    scalars: Vec<u64>,
    identities: Vec<[u8; 32]>,
    transcript_digest: [u8; 32],
}

#[inline(never)]
fn execute_interpreted_transition_v3(
    transition: TransitionProgramV3<'_>,
    tail_count: u32,
    input_scalars: &[u64],
    input_identities: &[[u8; 32]],
) -> Result<CandidateExecutionV3, ProgramError> {
    let mut scratch_scalars = input_scalars.to_vec();
    let mut scratch_identities = input_identities.to_vec();
    let mut output_scalars = input_scalars.to_vec();
    let mut output_identities = input_identities.to_vec();
    execute_fold_atomic(
        transition,
        tail_count,
        RegisterInput {
            scalars: input_scalars,
            identities: input_identities,
        },
        RegisterOutput {
            scalars: &mut scratch_scalars,
            identities: &mut scratch_identities,
        },
        RegisterOutput {
            scalars: &mut output_scalars,
            identities: &mut output_identities,
        },
    )
    .map_err(|_| TradingSbfError::Transition)?;
    Ok(CandidateExecutionV3 {
        scalars: output_scalars,
        identities: output_identities,
        transcript_digest: [0_u8; 32],
    })
}

struct ProjectedEffectsV3 {
    lamports: Vec<u64>,
    requests: Vec<u8>,
}

/// Account candidates and both Effect scratch banks are phase-local. Only the
/// exact lamport projection and child-request bank survive into preflight/CPI.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn project_hot_effects_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    observations: &[AccountObservationV1<'_>],
    lifecycle_plans: &[PreparedLifecycleInvocationV3],
    account_profile: AccountProfileV2<'_>,
    span_counts: &[u32],
    aliases: &[usize],
    runtime_account_count: usize,
    request_bytes: usize,
) -> Result<ProjectedEffectsV3, ProgramError> {
    let mut account_inputs = observations
        .iter()
        .map(|observation| AccountInput {
            lamports: observation.lamports(),
            data_len: observation.data().len(),
        })
        .collect::<Vec<_>>();
    apply_lifecycle_candidates_v3(lifecycle_plans, aliases, &mut account_inputs)?;
    let mut permissions = vec![AccountPermission::read_only(); runtime_account_count];
    if account_profile.uses_dynamic_fixed_spans() {
        derive_effect_permissions_with_dynamic_spans(
            account_profile,
            tail_count,
            span_counts,
            &mut permissions,
        )
        .map_err(|_| TradingSbfError::Content)?;
    } else {
        derive_effect_permissions(account_profile, tail_count, &mut permissions)
            .map_err(|_| TradingSbfError::Content)?;
    }
    require_common_projection_permissions_v3(&permissions)?;
    let effect_account_count = effect
        .successor
        .account_count(tail_count, scalars)
        .map_err(|_| TradingSbfError::Transition)?;
    if effect_account_count > runtime_account_count
        || permissions
            .get(effect_account_count..)
            .ok_or(TradingSbfError::Content)?
            .iter()
            .any(|permission| *permission != AccountPermission::read_only())
    {
        return Err(TradingSbfError::Content.into());
    }
    let mut scratch_lamports = vec![0_u64; effect_account_count];
    let mut effect_output_lamports = vec![0_u64; effect_account_count];
    let mut scratch_requests = vec![0_u8; request_bytes];
    let mut output_requests = vec![0_u8; request_bytes];
    project_effects_v4_atomic(
        effect.successor,
        tail_count,
        ProjectionV3 {
            scalars,
            identities,
            aliases: aliases
                .get(..effect_account_count)
                .ok_or(TradingSbfError::Content)?,
            accounts: account_inputs
                .get(..effect_account_count)
                .ok_or(TradingSbfError::Content)?,
            permissions: permissions
                .get(..effect_account_count)
                .ok_or(TradingSbfError::Content)?,
            scratch_lamports: &mut scratch_lamports,
            output_lamports: &mut effect_output_lamports,
            scratch_requests: &mut scratch_requests,
            output_requests: &mut output_requests,
        },
    )
    .map_err(|_| TradingSbfError::Transition)?;
    let mut output_lamports = account_inputs
        .iter()
        .map(|account| account.lamports)
        .collect::<Vec<_>>();
    output_lamports
        .get_mut(..effect_account_count)
        .ok_or(TradingSbfError::Content)?
        .copy_from_slice(&effect_output_lamports);
    Ok(ProjectedEffectsV3 {
        lamports: output_lamports,
        requests: output_requests,
    })
}

#[inline(never)]
fn execute_admitted_candidate_v3(
    view: AdmittedCandidateViewV3<'_, '_, '_, '_>,
) -> Result<CandidateExecutionV3, ProgramError> {
    let accelerator_program = view
        .strategy_extras
        .get(6)
        .ok_or(TradingSbfError::Content)?;
    let accelerator_programdata = view
        .strategy_extras
        .get(7)
        .ok_or(TradingSbfError::Content)?;
    let family_request_digest =
        family_request_digest_v3(view.family_request).map_err(|_| TradingSbfError::Content)?;
    let runtime_transcript = view
        .observations
        .iter()
        .zip(view.runtime_accounts)
        .map(|(observation, account)| ShadowRuntimeObservationV3 {
            key: observation.key(),
            owner: observation.owner(),
            lamports: observation.lamports(),
            data: observation.data(),
            signer: false,
            writable: false,
            executable: account.executable,
        })
        .collect::<Vec<_>>();
    let runtime_observations_digest = runtime_observations_digest_v3(&runtime_transcript)
        .map_err(|_| TradingSbfError::Content)?;
    let product_runtime = view.product_runtime_v3.runtime;
    let admitted_context = AdmittedInvocationContextV3 {
        release_set: ContentId::new(view.envelope.release_set())
            .map_err(|_| TradingSbfError::Content)?,
        market: ContentId::new(view.envelope.market()).map_err(|_| TradingSbfError::Content)?,
        root: ContentId::new(view.frame.root.key.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        registry_program: ContentId::new(view.frame.registry.key.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        trading_program: ContentId::new(view.program_id.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        accelerator_program: ContentId::new(accelerator_program.key.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        capability_program: view.selected_program,
        account_profile: view.descriptor.account_profile().program(),
        request_profile: view.descriptor.request_profile().program(),
        transition: view.strategy.strategy().transition_program(),
        effect: view.descriptor.effect().program(),
        lifecycle: view.descriptor.derivation_policy(),
        strategy: view.strategy.strategy_program_id(),
        certificate: view
            .strategy
            .certificate_program_id()
            .ok_or(TradingSbfError::Content)?,
        admission: view
            .strategy
            .admission_program_id()
            .ok_or(TradingSbfError::Content)?,
        artifact_release: view
            .strategy
            .artifact_release_id()
            .ok_or(TradingSbfError::Content)?,
        config: view.context.selection().config(),
        product: ContentId::new(product_runtime.product_record.content_digest.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        portfolio: ContentId::new(product_runtime.portfolio_record.content_digest.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        linked_basis: ContentId::new(
            view.product_runtime_v3
                .linked_basis_record
                .content_digest
                .to_bytes(),
        )
        .map_err(|_| TradingSbfError::Content)?,
        family_request_digest,
        runtime_observations_digest,
        root_prestate_digest: ContentId::new(view.root_prestate)
            .map_err(|_| TradingSbfError::Content)?,
        selected_action: view.selected_action,
        tail_count: view.tail_count,
        account_count: u32::try_from(view.runtime_accounts.len())
            .map_err(|_| TradingSbfError::Content)?,
        scalar_count: u32::try_from(view.scalars.len()).map_err(|_| TradingSbfError::Content)?,
        identity_count: u32::try_from(view.identities.len())
            .map_err(|_| TradingSbfError::Content)?,
    };
    let execution = execute_admitted_aot_v3(
        view.program_id,
        AdmittedCpiFrameV3 {
            caller_authorities: view.caller_authorities,
            hot_fixed_accounts: view.hot_fixed_accounts,
            activation: view.frame.activation_cache,
            registry: view.frame.registry,
            rent: view.frame.rent,
            instructions: view.frame.instructions,
            trading_program: view.frame.trading_program,
            trading_programdata: view.frame.trading_programdata,
            capability_raw: view.frame.descriptor_raw,
            capability_staging: view.frame.descriptor_staging,
            strategy_raw: view.frame.strategy_raw,
            strategy_staging: view.frame.strategy_staging,
            certificate_raw: view
                .strategy_extras
                .first()
                .ok_or(TradingSbfError::Content)?,
            certificate_staging: view
                .strategy_extras
                .get(1)
                .ok_or(TradingSbfError::Content)?,
            admission_raw: view
                .strategy_extras
                .get(2)
                .ok_or(TradingSbfError::Content)?,
            admission_staging: view
                .strategy_extras
                .get(3)
                .ok_or(TradingSbfError::Content)?,
            artifact_raw: view
                .strategy_extras
                .get(4)
                .ok_or(TradingSbfError::Content)?,
            artifact_staging: view
                .strategy_extras
                .get(5)
                .ok_or(TradingSbfError::Content)?,
            accelerator_program,
            accelerator_programdata,
        },
        view.runtime_accounts,
        view.input_scratch_pages,
        &admitted_context,
        *view.strategy,
        view.scalars,
        view.identities,
    )?;
    Ok(CandidateExecutionV3 {
        scalars: execution.scalars,
        identities: execution.identities,
        transcript_digest: execution.transcript_digest,
    })
}

struct ShadowCandidateViewV3<'a, 'data, 'accounts, 'info> {
    program_id: &'a Pubkey,
    frame: &'a HotFrameV3<'accounts, 'info>,
    caller_authority: &'a AccountInfo<'info>,
    strategy_extras: &'a [AccountInfo<'info>],
    runtime_accounts: &'a [&'accounts AccountInfo<'info>],
    observations: &'a [AccountObservationV1<'data>],
    envelope: HotExecutionEnvelopeV3,
    descriptor: &'a CapabilityProgramV4,
    strategy: &'a AuthenticatedExecutionStrategyV2,
    family_request: &'a [u8],
    root_prestate: [u8; 32],
    selected_program: ContentId,
    selected_action: u32,
    effect: SelectedEffectProgramV4<'a>,
    tail_count: u32,
    scalars: &'a [u64],
    identities: &'a [[u8; 32]],
    output_lamports: &'a [u64],
    request_bank: &'a [u8],
}

#[inline(never)]
fn execute_shadow_candidate_v3(
    view: ShadowCandidateViewV3<'_, '_, '_, '_>,
) -> Result<[u8; 32], ProgramError> {
    let accelerator_program = view
        .strategy_extras
        .get(4)
        .ok_or(TradingSbfError::Content)?;
    let accelerator_programdata = view
        .strategy_extras
        .get(5)
        .ok_or(TradingSbfError::Content)?;
    let family_digest =
        family_request_digest_v3(view.family_request).map_err(|_| TradingSbfError::Content)?;
    let runtime_transcript = view
        .observations
        .iter()
        .zip(view.runtime_accounts)
        .map(|(observation, account)| ShadowRuntimeObservationV3 {
            key: observation.key(),
            owner: observation.owner(),
            lamports: observation.lamports(),
            data: observation.data(),
            signer: false,
            writable: false,
            executable: account.executable,
        })
        .collect::<Vec<_>>();
    let runtime_digest = runtime_observations_digest_v3(&runtime_transcript)
        .map_err(|_| TradingSbfError::Content)?;
    let candidate_digest = candidate_digest_v3(view.tail_count, view.scalars, view.identities)
        .map_err(|_| TradingSbfError::Content)?;
    let routes = shadow_routes_v3(view.effect, view.tail_count, view.scalars, view.identities)?;
    let effect_digest = effect_digest_v3(ShadowEffectProjectionV3 {
        tail_count: view.tail_count,
        output_lamports: view.output_lamports,
        request_bank: view.request_bank,
        routes: &routes,
    })
    .map_err(|_| TradingSbfError::Content)?;
    let release_set =
        ContentId::new(view.envelope.release_set()).map_err(|_| TradingSbfError::Content)?;
    let market = ContentId::new(view.envelope.market()).map_err(|_| TradingSbfError::Content)?;
    let root =
        ContentId::new(view.frame.root.key.to_bytes()).map_err(|_| TradingSbfError::Content)?;
    let root_prestate_digest =
        ContentId::new(view.root_prestate).map_err(|_| TradingSbfError::Content)?;
    let invocation_context = invocation_context_digest_v3(ShadowInvocationContextV3 {
        release_set,
        market,
        root,
        capability_program: view.selected_program,
        selected_action: view.selected_action,
        family_request_digest: family_digest,
        root_prestate_digest,
    })
    .map_err(|_| TradingSbfError::Content)?;
    execute_shadow_aot_v3(
        view.program_id,
        ShadowCpiFrameV3 {
            caller_authority: view.caller_authority,
            activation: view.frame.activation_cache,
            registry: view.frame.registry,
            trading_program: view.frame.trading_program,
            trading_programdata: view.frame.trading_programdata,
            accelerator_program,
            accelerator_programdata,
        },
        view.runtime_accounts,
        ShadowRequestV3 {
            release_set,
            market,
            root,
            registry_program: ContentId::new(view.frame.registry.key.to_bytes())
                .map_err(|_| TradingSbfError::Content)?,
            trading_program: ContentId::new(view.program_id.to_bytes())
                .map_err(|_| TradingSbfError::Content)?,
            accelerator_program: ContentId::new(accelerator_program.key.to_bytes())
                .map_err(|_| TradingSbfError::Content)?,
            artifacts: ShadowArtifactTupleV3 {
                capability_program: view.selected_program,
                account_profile: view.descriptor.account_profile().program(),
                request_profile: view.descriptor.request_profile().program(),
                transition: view.strategy.strategy().transition_program(),
                effect: view.descriptor.effect().program(),
                strategy: view.strategy.strategy_program_id(),
                certificate: view
                    .strategy
                    .certificate_program_id()
                    .ok_or(TradingSbfError::Content)?,
            },
            invocation_context,
            digests: ShadowExecutionDigestsV3 {
                runtime_observations: runtime_digest,
                family_request: family_digest,
                interpreted_candidate: candidate_digest,
                interpreted_effect: effect_digest,
            },
            shape: ShadowRuntimeShapeV3 {
                tail_count: view.tail_count,
                account_count: u32::try_from(view.runtime_accounts.len())
                    .map_err(|_| TradingSbfError::Content)?,
                scalar_count: u32::try_from(view.scalars.len())
                    .map_err(|_| TradingSbfError::Content)?,
                identity_count: u32::try_from(view.identities.len())
                    .map_err(|_| TradingSbfError::Content)?,
            },
            family_request: view.family_request,
        },
    )
}

/// The four authenticated record identities the common Hot frame substitutes
/// for a physical address when it observes a logical coordinate.
///
/// Borrowed, not copied, into each observation: a fixed topology aliases many
/// logical coordinates onto few physical accounts, and the SBF bump allocator
/// never frees, so a 90-entry bank pays for every by-value identity twice.
struct LogicalProjectionKeysV3 {
    selected_config: [u8; 32],
    product_root: [u8; 32],
    portfolio: [u8; 32],
    linked_basis: [u8; 32],
}

fn logical_projection_key_v3<'a>(
    coordinate: usize,
    physical_key: &'a Pubkey,
    projected: &'a LogicalProjectionKeysV3,
) -> &'a [u8; 32] {
    match coordinate {
        1 => &projected.selected_config,
        2 => &projected.product_root,
        3 => &projected.portfolio,
        4 => &projected.linked_basis,
        _ => physical_key.as_array(),
    }
}

struct AuthenticatedDynamicSpanWidthsV3 {
    widths: Vec<u32>,
    request_projection_scalars: Vec<u64>,
    transport_span: Option<u16>,
}

fn require_dynamic_span_values_v3(
    profile: AccountProfileV2<'_>,
    expected: &[u32],
    scalars: &[u64],
) -> Result<(), ProgramError> {
    if !profile.uses_dynamic_fixed_spans() {
        return if expected.is_empty() {
            Ok(())
        } else {
            Err(TradingSbfError::Content.into())
        };
    }
    let mut observed = vec![0_u32; usize::from(profile.dynamic_fixed_span_count())];
    profile
        .dynamic_span_widths_from_scalars(scalars, &mut observed)
        .map_err(|_| TradingSbfError::Content)?;
    if observed == expected {
        Ok(())
    } else {
        Err(TradingSbfError::Content.into())
    }
}

/// Derive Profile13 physical widths before account expansion without accepting
/// account-vector length as authority.
///
/// Request-owned selectors are projected once from the exact family bytes into
/// a throwaway bank. A sole non-Request selector is admitted only when the
/// authenticated strategy's canonical bank geometry requires scratch pages;
/// that page count is then derived from scalar/identity widths. Every EffectV4
/// span selector must be one of the Request-owned Profile13 selectors, while
/// the scratch transport span remains AccountProfile-only and effectless.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn authenticate_dynamic_span_widths_v3(
    profile: AccountProfileV2<'_>,
    request: RequestProfileKindV3<'_>,
    effect: SelectedEffectProgramV4<'_>,
    disposition: StrategyDispositionV2,
    tail_count: u32,
    family_request: &[u8],
    request_digest: [u8; 32],
    trusted_environment: TrustedEnvironmentObservationV3,
    scalar_count: usize,
    identity_count: usize,
) -> Result<AuthenticatedDynamicSpanWidthsV3, ProgramError> {
    if !profile.uses_dynamic_fixed_spans() {
        if profile.dynamic_fixed_span_count() != 0 || effect.successor.span_count() != 0 {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(AuthenticatedDynamicSpanWidthsV3 {
            widths: Vec::new(),
            request_projection_scalars: Vec::new(),
            transport_span: None,
        });
    }
    if profile.dynamic_fixed_span_count() == 0 {
        if effect.successor.span_count() != 0 {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(AuthenticatedDynamicSpanWidthsV3 {
            widths: Vec::new(),
            request_projection_scalars: Vec::new(),
            transport_span: None,
        });
    }
    let mut input_scalars = vec![0_u64; scalar_count];
    let mut input_identities = vec![[0_u8; 32]; identity_count];
    *input_identities
        .get_mut(HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3)
        .ok_or(TradingSbfError::Content)? = request_digest;
    seed_trusted_environment_v3(
        trusted_environment,
        &mut input_scalars,
        &mut input_identities,
    )?;
    let mut scratch_scalars = input_scalars.clone();
    let mut scratch_identities = input_identities.clone();
    let mut projected_scalars = input_scalars.clone();
    let mut projected_identities = input_identities.clone();
    request.project_atomic(
        tail_count,
        family_request,
        ProjectionRegistersV1 {
            input_scalars: &input_scalars,
            input_identities: &input_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut projected_scalars,
            output_identities: &mut projected_identities,
        },
    )?;
    // `projected_scalars` is the failure-atomic output of the throwaway request
    // projection; the other banks remain phase-local validation scratch.
    let mut widths = vec![0_u32; usize::from(profile.dynamic_fixed_span_count())];
    let transport_page_count = match classify_bank_transport_v2(
        u32::try_from(scalar_count).map_err(|_| TradingSbfError::Content)?,
        u32::try_from(identity_count).map_err(|_| TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::Content)?
    {
        BankTransportV2::InlineReturnData { .. } => None,
        BankTransportV2::AuthenticatedScratchPages { page_count, .. } => Some(page_count),
    };
    let mut transport_span = None;
    let mut index = 0_u16;
    while index < profile.dynamic_fixed_span_count() {
        let span = profile
            .dynamic_fixed_span(index)
            .map_err(|_| TradingSbfError::Content)?;
        let target = ProjectionTargetV1 {
            kind: ProjectionRegisterKindV1::Scalar,
            space: ProjectionRegisterSpaceV1::Common,
            index: span.count_scalar(),
        };
        let request_owned = request.writes_register(target)?;
        let effect_owned = (0..effect.successor.span_count()).any(|effect_index| {
            effect
                .successor
                .span(effect_index)
                .is_ok_and(|value| value.selector_common_scalar() == span.count_scalar())
        });
        if request_owned {
            if !effect_owned {
                require_trailing_account_profile_only_span_v3(profile, span)?;
            }
        } else {
            if effect_owned
                || disposition != StrategyDispositionV2::AdmittedAot
                || transport_span.is_some()
            {
                return Err(TradingSbfError::Content.into());
            }
            require_trailing_account_profile_only_span_v3(profile, span)?;
            let page_count = transport_page_count.ok_or(TradingSbfError::Content)?;
            *projected_scalars
                .get_mut(usize::from(span.count_scalar()))
                .ok_or(TradingSbfError::Content)? = u64::from(page_count);
            transport_span = Some(index);
        }
        index = index.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    let mut effect_span = 0_u16;
    while effect_span < effect.successor.span_count() {
        let selector = effect
            .successor
            .span(effect_span)
            .map_err(|_| TradingSbfError::Content)?
            .selector_common_scalar();
        if !(0..profile.dynamic_fixed_span_count()).any(|profile_index| {
            profile
                .dynamic_fixed_span(profile_index)
                .is_ok_and(|value| value.count_scalar() == selector)
        }) {
            return Err(TradingSbfError::Content.into());
        }
        effect_span = effect_span.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    profile
        .dynamic_span_widths_from_scalars(&projected_scalars, &mut widths)
        .map_err(|_| TradingSbfError::Content)?;
    effect
        .successor
        .account_count(tail_count, &projected_scalars)
        .map_err(|_| TradingSbfError::Content)?;
    if disposition == StrategyDispositionV2::AdmittedAot
        && transport_page_count.is_some() != transport_span.is_some()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(AuthenticatedDynamicSpanWidthsV3 {
        widths,
        request_projection_scalars: projected_scalars,
        transport_span,
    })
}

fn authenticated_input_scratch_pages_v3<'accounts, 'info>(
    profile: AccountProfileV2<'_>,
    span_counts: &[u32],
    transport_span: Option<u16>,
    logical_accounts: &'accounts [&'accounts AccountInfo<'info>],
) -> Result<&'accounts [&'accounts AccountInfo<'info>], ProgramError> {
    let Some(transport_span) = transport_span else {
        return Ok(&[]);
    };
    if !profile.uses_dynamic_fixed_spans()
        || span_counts.len() != usize::from(profile.dynamic_fixed_span_count())
        || transport_span >= profile.dynamic_fixed_span_count()
    {
        return Err(TradingSbfError::Content.into());
    }
    let span = profile
        .dynamic_fixed_span(transport_span)
        .map_err(|_| TradingSbfError::Content)?;
    require_trailing_account_profile_only_span_v3(profile, span)?;
    let prior_width = span_counts
        .get(..usize::from(transport_span))
        .ok_or(TradingSbfError::Content)?
        .iter()
        .try_fold(0_usize, |sum, width| {
            sum.checked_add(usize::try_from(*width).map_err(|_| TradingSbfError::Content)?)
                .ok_or(TradingSbfError::Content)
        })?;
    let start = usize::from(profile.fixed_account_count())
        .checked_add(prior_width)
        .ok_or(TradingSbfError::Content)?;
    let width = usize::try_from(
        *span_counts
            .get(usize::from(transport_span))
            .ok_or(TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let end = start.checked_add(width).ok_or(TradingSbfError::Content)?;
    logical_accounts
        .get(start..end)
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn require_trailing_account_profile_only_span_v3(
    profile: AccountProfileV2<'_>,
    span: dclutch_account_profile_contract::v2::DynamicFixedSpanV2,
) -> Result<(), ProgramError> {
    if span.insertion_coordinate() != profile.fixed_account_count() {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

fn expand_runtime_accounts_v3<'accounts, 'info>(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    injected: [&'accounts AccountInfo<'info>; 5],
    supplied_suffix: &'accounts [AccountInfo<'info>],
) -> Result<Vec<&'accounts AccountInfo<'info>>, ProgramError> {
    let dynamic = profile.uses_dynamic_fixed_spans();
    let logical_count = if dynamic {
        profile
            .logical_account_count_with_dynamic_spans(tail_count, span_counts)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        if !span_counts.is_empty() {
            return Err(TradingSbfError::Content.into());
        }
        profile
            .logical_account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    };
    let physical_count = if dynamic {
        profile
            .physical_account_count_with_dynamic_spans(tail_count, span_counts)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        profile
            .physical_account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    };
    if logical_count > MAX_HOT_RUNTIME_ACCOUNTS_V3
        || physical_count < injected.len()
        || supplied_suffix.len()
            != physical_count
                .checked_sub(injected.len())
                .ok_or(TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    for coordinate in 0..injected.len() {
        let representative = if dynamic {
            profile
                .representative_with_dynamic_spans(tail_count, span_counts, coordinate)
                .map_err(|_| TradingSbfError::Content)?
        } else {
            profile
                .representative(tail_count, coordinate)
                .map_err(|_| TradingSbfError::Content)?
        };
        let ordinal = if dynamic {
            profile
                .physical_account_ordinal_with_dynamic_spans(tail_count, span_counts, coordinate)
                .map_err(|_| TradingSbfError::Content)?
        } else {
            profile
                .physical_account_ordinal(tail_count, coordinate)
                .map_err(|_| TradingSbfError::Content)?
        };
        if representative != coordinate || ordinal != coordinate {
            return Err(TradingSbfError::Content.into());
        }
    }
    let mut physical = Vec::with_capacity(physical_count);
    physical.extend(injected);
    physical.extend(supplied_suffix.iter());
    if dynamic {
        return expand_dynamic_physical_accounts_v4(
            profile,
            tail_count,
            span_counts,
            physical.as_slice(),
        );
    }
    // One forward sweep, not one prefix recount per coordinate: see
    // `expand_dynamic_physical_accounts_v4` for why the two maps are identical.
    let packs = profile.supports_route_alias_packing();
    let mut logical = Vec::with_capacity(logical_count);
    let mut next = 0_usize;
    let mut coordinate = 0_usize;
    while coordinate < logical_count {
        let representative = profile
            .representative(tail_count, coordinate)
            .map_err(|_| TradingSbfError::Content)?;
        let resolved = if !packs {
            *physical.get(coordinate).ok_or(TradingSbfError::Content)?
        } else if representative == coordinate {
            let resolved = *physical.get(next).ok_or(TradingSbfError::Content)?;
            next = next.checked_add(1).ok_or(TradingSbfError::Content)?;
            resolved
        } else {
            if representative >= coordinate
                || profile
                    .representative(tail_count, representative)
                    .map_err(|_| TradingSbfError::Content)?
                    != representative
            {
                return Err(TradingSbfError::Content.into());
            }
            *logical
                .get(representative)
                .ok_or(TradingSbfError::Content)?
        };
        logical.push(resolved);
        coordinate = coordinate.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(logical)
}

#[inline(never)]
fn downgraded_effect_accounts_v3<'info>(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    logical_accounts: &[&AccountInfo<'info>],
) -> Result<Vec<AccountInfo<'info>>, ProgramError> {
    if profile.uses_dynamic_fixed_spans() {
        return downgrade_dynamic_child_accounts_v4(
            profile,
            tail_count,
            span_counts,
            logical_accounts,
        );
    }
    if !span_counts.is_empty() {
        return Err(TradingSbfError::Content.into());
    }
    if logical_accounts.len()
        != profile
            .logical_account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    let mut downgraded = Vec::new();
    downgraded
        .try_reserve_exact(logical_accounts.len())
        .map_err(|_| TradingSbfError::Content)?;
    for (coordinate, account) in logical_accounts.iter().enumerate() {
        downgraded.push(child_route_view_v3(
            account,
            profile
                .route_privileges(
                    tail_count,
                    profile
                        .representative(tail_count, coordinate)
                        .map_err(|_| TradingSbfError::Content)?,
                )
                .map_err(|_| TradingSbfError::Content)?,
        )?);
    }
    Ok(downgraded)
}

/// Build one child CPI view of a physical account from the privileges its
/// semantic owner declares.
///
/// An authenticated route alias declares no privileges of its own -- the
/// AccountProfile validator's route-alias contract requires the producer to
/// emit it privilege-free -- so the representative coordinate is the sole owner
/// of every privilege fact about the physical account, signer and writable
/// included, not only executability. Reading them from the alias produced a
/// readonly non-signer meta for an account the authenticated FrameSpec-derived
/// representative rule states as writable, which the child program cannot
/// honour; nothing about the alias ever expressed a per-route downgrade,
/// because there is no privilege field in an alias to express one with.
///
/// The other direction is refused here for writability: a declaration never
/// becomes a writable meta for an account the transaction did not include as
/// writable, because no CPI can escalate that and the runtime's own refusal
/// names nothing useful. Signer is deliberately not required of the
/// transaction: a child route's caller authority is a Trading PDA that signs
/// only inside the child CPI, through `invoke_signed`, which is exactly the
/// privilege the FrameSpec owns and the outer frame never grants; a meta that
/// claims a signer Trading cannot produce seeds for still fails closed in the
/// runtime. Executability is exact in both directions: it is a property of the
/// account, never granted or suppressed by a route.
fn child_route_view_v3<'info>(
    account: &AccountInfo<'info>,
    declared: RouteAccountPrivilegesV2,
) -> Result<AccountInfo<'info>, ProgramError> {
    if (declared.writable() && !account.is_writable) || declared.executable() != account.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    let mut logical = account.clone();
    logical.is_signer = declared.signer();
    logical.is_writable = declared.writable();
    Ok(logical)
}

#[derive(Clone, Copy)]
struct CommonProjectionBindingsV3 {
    selected_config: [u8; 32],
    selected_product_record: [u8; 32],
    authenticated_product_record: [u8; 32],
    market_product: [u8; 32],
    runtime_product: [u8; 32],
    product_semantic_basis: [u8; 32],
    authenticated_semantic_basis: [u8; 32],
    authenticated_linked_basis: [u8; 32],
}

fn require_common_projection_bindings_v3(
    bindings: CommonProjectionBindingsV3,
) -> Result<(), ProgramError> {
    if bindings.selected_config == [0; 32]
        || bindings.selected_product_record == [0; 32]
        || bindings.selected_product_record != bindings.authenticated_product_record
        || bindings.market_product == [0; 32]
        || bindings.market_product != bindings.runtime_product
        || bindings.product_semantic_basis == [0; 32]
        || bindings.product_semantic_basis != bindings.authenticated_semantic_basis
        || bindings.authenticated_linked_basis == [0; 32]
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

fn require_tail_count_agreement_v3(
    product_outcome_count: u32,
    projected_tail_count: u32,
) -> Result<(), ProgramError> {
    if product_outcome_count < 2 || product_outcome_count != projected_tail_count {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

fn require_common_projection_permissions_v3(
    permissions: &[AccountPermission],
) -> Result<(), ProgramError> {
    if permissions.get(1) != Some(&AccountPermission::read_only())
        || permissions.get(2) != Some(&AccountPermission::read_only())
        || permissions.get(3) != Some(&AccountPermission::read_only())
        || permissions.get(4) != Some(&AccountPermission::read_only())
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

fn lifecycle_request_target_v4(target: LifecycleRegisterTargetV3) -> ProjectionTargetV1 {
    ProjectionTargetV1 {
        kind: match target.kind() {
            LifecycleRegisterKindV3::Scalar => ProjectionRegisterKindV1::Scalar,
            LifecycleRegisterKindV3::Identity => ProjectionRegisterKindV1::Identity,
        },
        space: match target.scope() {
            CoordinateScopeV3::Fixed => ProjectionRegisterSpaceV1::Common,
            CoordinateScopeV3::Item => ProjectionRegisterSpaceV1::Item,
        },
        index: target.index(),
    }
}

fn lifecycle_transition_target_v4(target: LifecycleRegisterTargetV3) -> RegisterWriteTargetV3 {
    RegisterWriteTargetV3 {
        kind: match target.kind() {
            LifecycleRegisterKindV3::Scalar => RegisterKindV3::Scalar,
            LifecycleRegisterKindV3::Identity => RegisterKindV3::Identity,
        },
        space: match target.scope() {
            CoordinateScopeV3::Fixed => RegisterSpaceV3::Common,
            CoordinateScopeV3::Item => RegisterSpaceV3::Item,
        },
        index: target.index(),
    }
}

/// Materialize current-Rent facts only from the authenticated Rent sysvar and
/// the exact V5 declarations selected by the capability descriptor.
#[inline(never)]
fn authenticate_current_rent_quotes_v5(
    policy: StateLifecyclePolicyV5<'_>,
    rent: &Rent,
) -> Result<Vec<AuthenticatedRentQuoteV5>, ProgramError> {
    let mut quotes = Vec::with_capacity(usize::from(policy.current_rent_quote_count()));
    let mut ordinal = 0_u16;
    while ordinal < policy.current_rent_quote_count() {
        let declaration = policy
            .current_rent_quote(ordinal)
            .map_err(|_| TradingSbfError::Content)?;
        let exact_data_len = declaration.exact_data_len();
        quotes.push(AuthenticatedRentQuoteV5 {
            exact_data_len,
            scalar_destination: declaration.scalar_destination().index(),
            current_minimum: rent.minimum_balance(
                usize::try_from(exact_data_len).map_err(|_| TradingSbfError::Content)?,
            ),
        });
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(quotes)
}

/// Every artifact whose static register ownership is checked together.
struct StaticRegisterOwnershipV5<'a> {
    account_profile: AccountProfileV2<'a>,
    policy: StateLifecyclePolicyV5<'a>,
    action: u32,
    request: RequestProfileKindV3<'a>,
    transition: TransitionProgramV3<'a>,
}

/// Require that no register the lifecycle policy, the trusted-environment
/// observation, or a dynamic fixed span owns is also written by the request
/// profile or by the transition program.
///
/// The three predicates this replaces each asked one target at a time, and
/// both `writes_register` implementations answer a single target by decoding
/// every operation of their whole program. Over the Direct Profile14 lifecycle
/// that is a few dozen full passes of a 66-instruction transition and of the
/// request profile. Every target is collected first - the structural
/// requirements on each plan are still checked while collecting - and each
/// artifact is then walked exactly once for the entire set. The accepted set
/// is unchanged: a target is refused here if and only if
/// `writes_register` would have reported it before.
#[inline(never)]
fn require_static_register_ownership_v5(
    input: StaticRegisterOwnershipV5<'_>,
) -> Result<(), ProgramError> {
    let StaticRegisterOwnershipV5 {
        account_profile,
        policy,
        action,
        request,
        transition,
    } = input;
    let plan_count = policy
        .action_plan_count(action)
        .map_err(|_| TradingSbfError::Content)?;
    // Exact upper bounds, so neither bank walks the bump allocator's doubling
    // ladder: rent quotes and three trusted-environment registers are forbidden
    // to both artifacts, and every per-plan lifecycle register plus every
    // dynamic-span count scalar is additionally forbidden to the transition.
    let shared_bound = usize::from(policy.current_rent_quote_count())
        .checked_add(3)
        .ok_or(TradingSbfError::Content)?;
    let mut transition_bound = shared_bound;
    if account_profile.uses_dynamic_fixed_spans() {
        transition_bound = transition_bound
            .checked_add(usize::from(account_profile.dynamic_fixed_span_count()))
            .ok_or(TradingSbfError::Content)?;
    }
    let mut request_bound = shared_bound;
    let mut counted = 0_u16;
    while counted < plan_count {
        let selected = policy
            .action_plan(action, counted)
            .map_err(|_| TradingSbfError::Content)?;
        for width in [
            usize::from(
                selected
                    .protected_observation_count()
                    .map_err(|_| TradingSbfError::Content)?,
            ),
            usize::from(
                selected
                    .protected_output_count()
                    .map_err(|_| TradingSbfError::Content)?,
            ),
        ] {
            request_bound = request_bound
                .checked_add(width)
                .ok_or(TradingSbfError::Content)?;
            transition_bound = transition_bound
                .checked_add(width)
                .ok_or(TradingSbfError::Content)?;
        }
        for width in [
            usize::from(
                selected
                    .seed_count()
                    .map_err(|_| TradingSbfError::Content)?,
            ),
            usize::from(
                selected
                    .immutable_identity_binding_count()
                    .map_err(|_| TradingSbfError::Content)?,
            ),
        ] {
            transition_bound = transition_bound
                .checked_add(width)
                .ok_or(TradingSbfError::Content)?;
        }
        counted = counted.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    let mut request_forbidden = Vec::with_capacity(request_bound);
    let mut transition_forbidden: Vec<RegisterWriteTargetV3> = Vec::with_capacity(transition_bound);

    let mut quote = 0_u16;
    while quote < policy.current_rent_quote_count() {
        let target = policy
            .current_rent_quote(quote)
            .map_err(|_| TradingSbfError::Content)?
            .scalar_destination();
        request_forbidden.push(lifecycle_request_target_v4(target));
        transition_forbidden.push(lifecycle_transition_target_v4(target));
        quote = quote.checked_add(1).ok_or(TradingSbfError::Content)?;
    }

    for (index, register) in [
        account_profile.trusted_current_slot_scalar(),
        account_profile.trusted_current_executing_program_identity(),
        account_profile.trusted_system_program_identity(),
    ]
    .into_iter()
    .zip([
        ProjectionRegisterKindV1::Scalar,
        ProjectionRegisterKindV1::Identity,
        ProjectionRegisterKindV1::Identity,
    ]) {
        let Some(index) = index else {
            continue;
        };
        request_forbidden.push(ProjectionTargetV1 {
            kind: register,
            space: ProjectionRegisterSpaceV1::Common,
            index,
        });
        transition_forbidden.push(RegisterWriteTargetV3 {
            kind: match register {
                ProjectionRegisterKindV1::Scalar => RegisterKindV3::Scalar,
                ProjectionRegisterKindV1::Identity => RegisterKindV3::Identity,
            },
            space: RegisterSpaceV3::Common,
            index,
        });
    }

    if account_profile.uses_dynamic_fixed_spans() {
        let mut span = 0_u16;
        while span < account_profile.dynamic_fixed_span_count() {
            transition_forbidden.push(RegisterWriteTargetV3 {
                kind: RegisterKindV3::Scalar,
                space: RegisterSpaceV3::Common,
                index: account_profile
                    .dynamic_fixed_span(span)
                    .map_err(|_| TradingSbfError::Content)?
                    .count_scalar(),
            });
            span = span.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
    }

    let mut ordinal = 0_u16;
    while ordinal < plan_count {
        let selected = policy
            .action_plan(action, ordinal)
            .map_err(|_| TradingSbfError::Content)?;
        if selected.operation() != LifecycleOperationV3::AuthenticateOrCreate
            || selected
                .protected_output_count()
                .map_err(|_| TradingSbfError::Content)?
                != 6
        {
            return Err(TradingSbfError::Content.into());
        }
        let mut observation = 0_u8;
        while observation
            < selected
                .protected_observation_count()
                .map_err(|_| TradingSbfError::Content)?
        {
            let target = selected
                .protected_observation_target(observation)
                .map_err(|_| TradingSbfError::Content)?;
            request_forbidden.push(lifecycle_request_target_v4(target));
            transition_forbidden.push(lifecycle_transition_target_v4(target));
            observation = observation.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        let mut output = 0_u8;
        while output
            < selected
                .protected_output_count()
                .map_err(|_| TradingSbfError::Content)?
        {
            let target = selected
                .protected_output_target(output)
                .map_err(|_| TradingSbfError::Content)?;
            request_forbidden.push(lifecycle_request_target_v4(target));
            transition_forbidden.push(lifecycle_transition_target_v4(target));
            output = output.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        let mut seed = 0_u8;
        while seed
            < selected
                .seed_count()
                .map_err(|_| TradingSbfError::Content)?
        {
            if let Some(target) = selected
                .seed_register_target(seed)
                .map_err(|_| TradingSbfError::Content)?
            {
                transition_forbidden.push(lifecycle_transition_target_v4(target));
            }
            seed = seed.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        let mut binding = 0_u16;
        while binding
            < selected
                .immutable_identity_binding_count()
                .map_err(|_| TradingSbfError::Content)?
        {
            transition_forbidden.push(lifecycle_transition_target_v4(
                selected
                    .immutable_identity_binding(binding)
                    .map_err(|_| TradingSbfError::Content)?
                    .canonical(),
            ));
            binding = binding.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }

    if request.writes_any_register(&request_forbidden)?
        || transition
            .writes_any_register(&transition_forbidden)
            .map_err(|_| TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

#[derive(Debug, Eq, PartialEq)]
struct PreparedImmutableIdentityBindingV4 {
    data_offset: u32,
    canonical: [u8; 32],
}

#[derive(Debug, Eq, PartialEq)]
struct PreparedLifecycleInvocationV3 {
    plan: StateLifecyclePlanV3,
    state: usize,
    payer: Option<usize>,
    rent_credit: Option<usize>,
    seeds: Vec<Vec<u8>>,
    immutable_identity_bindings: Vec<PreparedImmutableIdentityBindingV4>,
}

#[derive(Debug, Eq, PartialEq)]
struct PreparedLifecycleBatchV4 {
    plans: Vec<PreparedLifecycleInvocationV3>,
    scalars: Vec<u64>,
    identities: Vec<[u8; 32]>,
}

/// Rewrite one candidate observation's lamport balance in place.
///
/// Only the balance moves while a lifecycle batch is planned, so the candidate
/// bank is built once and its two touched entries are rewritten, rather than
/// materialising a whole new 90-coordinate bank per invocation on an allocator
/// that never frees.
fn set_candidate_lamports_v3<'data>(
    index: usize,
    value: u64,
    observations: &[AccountObservationV1<'data>],
    accounts: &[&AccountInfo<'_>],
    candidate_lamports: &mut [u64],
    candidate_observations: &mut [AccountObservationV1<'data>],
) -> Result<(), ProgramError> {
    *candidate_lamports
        .get_mut(index)
        .ok_or(TradingSbfError::Content)? = value;
    let observation = observations.get(index).ok_or(TradingSbfError::Content)?;
    let account = accounts.get(index).ok_or(TradingSbfError::Content)?;
    *candidate_observations
        .get_mut(index)
        .ok_or(TradingSbfError::Content)? =
        candidate_observation_v3(*observation, account, value);
    Ok(())
}

/// One candidate observation: the authenticated observation at a planned balance.
fn candidate_observation_v3<'data>(
    observation: AccountObservationV1<'data>,
    account: &AccountInfo<'_>,
    lamports: u64,
) -> AccountObservationV1<'data> {
    if observation.adapter_authenticated_variable_data() {
        AccountObservationV1::new_adapter_authenticated_variable_data(
            observation.key_bytes(),
            observation.owner_bytes(),
            lamports,
            observation.data(),
            account.is_signer,
            account.is_writable,
            account.executable,
        )
    } else {
        AccountObservationV1::new(
            observation.key_bytes(),
            observation.owner_bytes(),
            lamports,
            observation.data(),
            account.is_signer,
            account.is_writable,
            account.executable,
        )
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn prepare_lifecycle_v4<'a>(
    program_id: &Pubkey,
    expected_market: [u8; 32],
    expected_release_set: [u8; 32],
    expected_generation: u64,
    policy: StateLifecyclePolicyV5<'_>,
    action: u32,
    account_profile: AccountProfileV2<'_>,
    tail_count: u32,
    observations: &[AccountObservationV1<'a>],
    accounts: &[&AccountInfo<'_>],
    scalars: &[u64],
    identities: &[[u8; 32]],
    rent: &Rent,
    aliases: &[usize],
) -> Result<PreparedLifecycleBatchV4, ProgramError> {
    if observations.len() != accounts.len() || aliases.len() != accounts.len() {
        return Err(TradingSbfError::Content.into());
    }
    let mut output_scalars = scalars.to_vec();
    let mut output_identities = identities.to_vec();
    let mut candidate_lamports = observations
        .iter()
        .map(|observation| observation.lamports())
        .collect::<Vec<_>>();
    // The candidate bank and the three lifecycle register banks are allocated
    // once for the whole batch. The SBF allocator never frees, so materialising
    // them per invocation charged a fresh 90-coordinate observation bank and
    // four register pairs against total-ever-allocated for every planned state.
    let mut candidate_observations = Vec::with_capacity(observations.len());
    for (observation, account) in observations.iter().zip(accounts) {
        candidate_observations.push(candidate_observation_v3(
            *observation,
            account,
            observation.lamports(),
        ));
    }
    let mut scalar_scratch = vec![0_u64; scalars.len()];
    let mut identity_scratch = vec![[0_u8; 32]; identities.len()];
    let mut next_scalars = vec![0_u64; scalars.len()];
    let mut next_identities = vec![[0_u8; 32]; identities.len()];
    let mut used_states = vec![false; accounts.len()];
    let mut output = Vec::new();
    let plan_count = policy
        .action_plan_count(action)
        .map_err(|_| TradingSbfError::Content)?;
    let mut ordinal = 0_u16;
    while ordinal < plan_count {
        let selected = policy
            .action_plan(action, ordinal)
            .map_err(|_| TradingSbfError::Content)?;
        let invocation_count = selected
            .invocation_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?;
        let mut invocation = 0_u32;
        while invocation < invocation_count {
            let item = selected
                .invocation_item(tail_count, invocation)
                .map_err(|_| TradingSbfError::Content)?;
            let registers = LifecycleRegistersV3 {
                scalars: &output_scalars,
                identities: &output_identities,
            };
            if !selected
                .is_enabled(account_profile, tail_count, item, registers)
                .map_err(|_| TradingSbfError::Content)?
            {
                return Err(TradingSbfError::Content.into());
            }
            let indices = selected
                .project_account_indices(account_profile, tail_count, item)
                .map_err(|_| TradingSbfError::Content)?;
            let state = representative_v3(indices.state(), aliases)?;
            reserve_lifecycle_state_v3(state, &mut used_states)?;
            let payer = indices
                .payer()
                .map(|index| representative_v3(index, aliases))
                .transpose()?;
            let rent_credit = indices
                .rent_credit()
                .map(|index| representative_v3(index, aliases))
                .transpose()?;

            let seed_count = selected
                .seed_count()
                .map_err(|_| TradingSbfError::Content)?;
            let mut seeds = Vec::with_capacity(usize::from(seed_count));
            let mut derived = None;
            let mut canonical_bump = None;
            let mut seed = 0_u8;
            while seed < seed_count {
                match selected
                    .materialize_seed_input(account_profile, tail_count, item, registers, seed)
                    .map_err(|_| TradingSbfError::Content)?
                {
                    LifecycleSeedInputValueV3::Bytes(value) => {
                        if canonical_bump.is_some() {
                            return Err(TradingSbfError::Content.into());
                        }
                        seeds.push(value.as_slice().to_vec());
                    }
                    LifecycleSeedInputValueV3::CanonicalBump => {
                        if seed.checked_add(1) != Some(seed_count) || canonical_bump.is_some() {
                            return Err(TradingSbfError::Content.into());
                        }
                        let seed_slices = seeds.iter().map(Vec::as_slice).collect::<Vec<_>>();
                        let (address, bump) =
                            Pubkey::try_find_program_address(seed_slices.as_slice(), program_id)
                                .ok_or(TradingSbfError::Content)?;
                        seeds.push(vec![bump]);
                        derived = Some(address);
                        canonical_bump = Some(bump);
                    }
                }
                seed = seed.checked_add(1).ok_or(TradingSbfError::Content)?;
            }
            let derived = derived.ok_or(TradingSbfError::Content)?;
            let canonical_bump = canonical_bump.ok_or(TradingSbfError::Content)?;
            if accounts
                .get(state)
                .is_none_or(|account| account.key != &derived)
            {
                return Err(TradingSbfError::Content.into());
            }
            let authenticated_credit = rent_credit
                .map(|index| {
                    authenticate_lifecycle_credit_v3(
                        accounts,
                        index,
                        *candidate_lamports
                            .get(index)
                            .ok_or(TradingSbfError::Content)?,
                        rent,
                        expected_market,
                        expected_release_set,
                        expected_generation,
                    )
                })
                .transpose()?;
            let current_rent_minimum = if matches!(
                selected.operation(),
                LifecycleOperationV3::Create | LifecycleOperationV3::AuthenticateOrCreate
            ) {
                let data_bytes = selected
                    .target_data_bytes(tail_count)
                    .map_err(|_| TradingSbfError::Content)?;
                Some(AuthenticatedRentMinimumV3 {
                    data_bytes,
                    lamports: rent.minimum_balance(
                        usize::try_from(data_bytes).map_err(|_| TradingSbfError::Content)?,
                    ),
                })
            } else {
                None
            };
            scalar_scratch.copy_from_slice(&output_scalars);
            identity_scratch.copy_from_slice(&output_identities);
            next_scalars.copy_from_slice(&output_scalars);
            next_identities.copy_from_slice(&output_identities);
            let plan = plan_lifecycle_with_protected_outputs_atomic(
                selected,
                LifecycleContextV3 {
                    account_profile,
                    tail_count,
                    item_index: item,
                    accounts: &candidate_observations,
                    registers: LifecycleRegistersV3 {
                        scalars: &output_scalars,
                        identities: &output_identities,
                    },
                    trading_program: program_id.to_bytes(),
                    system_program: system_program::ID.to_bytes(),
                    adapter_derived_pda: derived.to_bytes(),
                    rent_credit: authenticated_credit,
                    current_rent_minimum,
                },
                canonical_bump,
                LifecycleProtectedRegisterBuffersV3 {
                    scalar_scratch: &mut scalar_scratch,
                    identity_scratch: &mut identity_scratch,
                    output_scalars: &mut next_scalars,
                    output_identities: &mut next_identities,
                },
            )
            .map_err(|_| TradingSbfError::Content)?;
            let immutable_identity_bindings = prepare_immutable_identity_bindings_v4(
                selected,
                account_profile,
                item,
                &next_identities,
            )?;
            match plan {
                StateLifecyclePlanV3::Authenticate(_) => {}
                StateLifecyclePlanV3::Create(value) => {
                    for (index, balance) in [
                        (state, value.state_after),
                        (payer.ok_or(TradingSbfError::Content)?, value.payer_after),
                    ] {
                        set_candidate_lamports_v3(
                            index,
                            balance,
                            observations,
                            accounts,
                            &mut candidate_lamports,
                            &mut candidate_observations,
                        )?;
                    }
                }
                StateLifecyclePlanV3::Close(value) => {
                    for (index, balance) in [
                        (state, value.source_after),
                        (
                            rent_credit.ok_or(TradingSbfError::Content)?,
                            value.rent_credit_after,
                        ),
                    ] {
                        set_candidate_lamports_v3(
                            index,
                            balance,
                            observations,
                            accounts,
                            &mut candidate_lamports,
                            &mut candidate_observations,
                        )?;
                    }
                }
            }
            output.push(PreparedLifecycleInvocationV3 {
                plan,
                state,
                payer,
                rent_credit,
                seeds,
                immutable_identity_bindings,
            });
            core::mem::swap(&mut output_scalars, &mut next_scalars);
            core::mem::swap(&mut output_identities, &mut next_identities);
            invocation = invocation.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(PreparedLifecycleBatchV4 {
        plans: output,
        scalars: output_scalars,
        identities: output_identities,
    })
}

fn prepare_immutable_identity_bindings_v4(
    selected: dclutch_account_profile_contract::lifecycle_v3::SelectedLifecycleV3<'_>,
    profile: AccountProfileV2<'_>,
    item: Option<u32>,
    identities: &[[u8; 32]],
) -> Result<Vec<PreparedImmutableIdentityBindingV4>, ProgramError> {
    let count = selected
        .immutable_identity_binding_count()
        .map_err(|_| TradingSbfError::Content)?;
    let mut output = Vec::with_capacity(usize::from(count));
    let mut ordinal = 0_u16;
    while ordinal < count {
        let binding = selected
            .immutable_identity_binding(ordinal)
            .map_err(|_| TradingSbfError::Content)?;
        let target = binding.canonical();
        if target.kind() != LifecycleRegisterKindV3::Identity {
            return Err(TradingSbfError::Content.into());
        }
        let index = match target.scope() {
            CoordinateScopeV3::Fixed => usize::from(target.index()),
            CoordinateScopeV3::Item => usize::from(profile.common_identity_count())
                .checked_add(
                    usize::try_from(item.ok_or(TradingSbfError::Content)?)
                        .map_err(|_| TradingSbfError::Content)?
                        .checked_mul(usize::from(profile.item_identity_stride()))
                        .ok_or(TradingSbfError::Content)?,
                )
                .and_then(|base| base.checked_add(usize::from(target.index())))
                .ok_or(TradingSbfError::Content)?,
        };
        let canonical = *identities.get(index).ok_or(TradingSbfError::Content)?;
        if canonical == [0; 32] {
            return Err(TradingSbfError::Content.into());
        }
        output.push(PreparedImmutableIdentityBindingV4 {
            data_offset: binding.data_offset(),
            canonical,
        });
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(output)
}

fn require_lifecycle_replan_agreement_v4(
    preplanned: &PreparedLifecycleBatchV4,
    revalidated: &PreparedLifecycleBatchV4,
    transition_scalars: &[u64],
    transition_identities: &[[u8; 32]],
) -> Result<(), ProgramError> {
    if preplanned.plans != revalidated.plans
        || revalidated.scalars != transition_scalars
        || revalidated.identities != transition_identities
    {
        Err(TradingSbfError::Transition.into())
    } else {
        Ok(())
    }
}

#[inline(never)]
fn require_lifecycle_effect_bindings_v4(
    plans: &[PreparedLifecycleInvocationV3],
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    aliases: &[usize],
) -> Result<(), ProgramError> {
    for prepared in plans {
        for binding in &prepared.immutable_identity_bindings {
            let mut exact_write = false;
            let mut fixed = 0_u16;
            while fixed < effect.fixed_operation_count() {
                exact_write |= inspect_lifecycle_binding_effect_v4(
                    prepared.state,
                    binding,
                    effect
                        .resolved_fixed_effect(fixed, tail_count, scalars, identities)
                        .map_err(|_| TradingSbfError::Transition)?,
                    aliases,
                )?;
                fixed = fixed.checked_add(1).ok_or(TradingSbfError::Transition)?;
            }
            let mut item = 0_u32;
            while item < tail_count {
                let mut operation = 0_u16;
                while operation < effect.item_operation_count() {
                    exact_write |= inspect_lifecycle_binding_effect_v4(
                        prepared.state,
                        binding,
                        effect
                            .resolved_item_effect(item, operation, tail_count, scalars, identities)
                            .map_err(|_| TradingSbfError::Transition)?,
                        aliases,
                    )?;
                    operation = operation
                        .checked_add(1)
                        .ok_or(TradingSbfError::Transition)?;
                }
                item = item.checked_add(1).ok_or(TradingSbfError::Transition)?;
            }
            if matches!(prepared.plan, StateLifecyclePlanV3::Create(_)) && !exact_write {
                return Err(TradingSbfError::Transition.into());
            }
        }
    }
    Ok(())
}

fn inspect_lifecycle_binding_effect_v4(
    state: usize,
    binding: &PreparedImmutableIdentityBindingV4,
    effect: ResolvedEffectV3,
    aliases: &[usize],
) -> Result<bool, ProgramError> {
    let (account, offset, width, identity) = match effect {
        ResolvedEffectV3::WriteScalar {
            account, offset, ..
        } => (account, offset, 8_u32, None),
        ResolvedEffectV3::WriteIdentity {
            account,
            offset,
            value,
        } => (account, offset, 32_u32, Some(value)),
        ResolvedEffectV3::WriteU8 {
            account, offset, ..
        } => (account, offset, 1_u32, None),
        ResolvedEffectV3::WriteU16 {
            account, offset, ..
        } => (account, offset, 2_u32, None),
        ResolvedEffectV3::WriteU32 {
            account, offset, ..
        } => (account, offset, 4_u32, None),
        ResolvedEffectV3::TransferLamports { .. }
        | ResolvedEffectV3::RequireLamportsEq { .. }
        | ResolvedEffectV3::WriteRequest { .. } => return Ok(false),
    };
    if representative_v3(account, aliases)? != state
        || !ranges_overlap_v4(offset, width, binding.data_offset, 32)?
    {
        return Ok(false);
    }
    if offset == binding.data_offset && identity == Some(binding.canonical) {
        Ok(true)
    } else {
        Err(TradingSbfError::Transition.into())
    }
}

fn ranges_overlap_v4(
    left_start: u32,
    left_width: u32,
    right_start: u32,
    right_width: u32,
) -> Result<bool, ProgramError> {
    let left_end = left_start
        .checked_add(left_width)
        .ok_or(TradingSbfError::Transition)?;
    let right_end = right_start
        .checked_add(right_width)
        .ok_or(TradingSbfError::Transition)?;
    Ok(left_start < right_end && right_start < left_end)
}

#[cfg(test)]
fn require_canonical_lifecycle_pda_v3(
    program_id: &Pubkey,
    seed_slices: &[&[u8]],
) -> Result<Pubkey, ProgramError> {
    let (bump_seed, canonical_seeds) = seed_slices.split_last().ok_or(TradingSbfError::Content)?;
    let [supplied_bump] = bump_seed else {
        return Err(TradingSbfError::Content.into());
    };
    let (derived, canonical_bump) = Pubkey::try_find_program_address(canonical_seeds, program_id)
        .ok_or(TradingSbfError::Content)?;
    if *supplied_bump != canonical_bump {
        return Err(TradingSbfError::Content.into());
    }
    Ok(derived)
}

fn representative_v3(index: usize, aliases: &[usize]) -> Result<usize, ProgramError> {
    aliases
        .get(index)
        .copied()
        .ok_or_else(|| TradingSbfError::Content.into())
}

fn reserve_lifecycle_state_v3(state: usize, used_states: &mut [bool]) -> Result<(), ProgramError> {
    if state == 0
        || used_states
            .get(state)
            .copied()
            .ok_or(TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    *used_states.get_mut(state).ok_or(TradingSbfError::Content)? = true;
    Ok(())
}

fn authenticate_lifecycle_credit_v3(
    accounts: &[&AccountInfo<'_>],
    index: usize,
    observed_lamports: u64,
    rent: &Rent,
    expected_market: [u8; 32],
    expected_release_set: [u8; 32],
    expected_generation: u64,
) -> Result<AuthenticatedRentCreditV3, ProgramError> {
    let account = accounts.get(index).ok_or(TradingSbfError::Content)?;
    if account.is_signer
        || !account.is_writable
        || account.executable
        || account.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || !rent.is_exempt(observed_lamports, LIFECYCLE_RENT_CREDIT_BYTES_V2)
    {
        return Err(TradingSbfError::Content.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
    if credit.to_bytes().as_slice() != data.as_ref()
        || credit.market().to_bytes() != expected_market
        || credit.release_set().to_bytes() != expected_release_set
        || credit.generation() != expected_generation
    {
        return Err(TradingSbfError::Content.into());
    }
    let seeds = credit.pda_seeds();
    let authority = credit.refund_wallet().to_bytes();
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market.as_slice(),
            generation.as_slice(),
            &bump,
        ],
        account.owner,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if account.key != &expected
        || !accounts.iter().any(|candidate| {
            candidate.key == account.owner
                && candidate.executable
                && !candidate.is_signer
                && !candidate.is_writable
        })
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(AuthenticatedRentCreditV3 {
        key: account.key.to_bytes(),
        beneficiary: authority,
        lamports: observed_lamports,
    })
}

fn apply_lifecycle_candidates_v3(
    plans: &[PreparedLifecycleInvocationV3],
    aliases: &[usize],
    accounts: &mut [AccountInput],
) -> Result<(), ProgramError> {
    for prepared in plans {
        match prepared.plan {
            StateLifecyclePlanV3::Authenticate(_) => {}
            StateLifecyclePlanV3::Create(plan) => {
                set_account_candidate_v3(
                    prepared.state,
                    aliases,
                    accounts,
                    plan.state_after,
                    usize::try_from(plan.target_data_bytes)
                        .map_err(|_| TradingSbfError::Content)?,
                )?;
                set_account_candidate_lamports_v3(
                    prepared.payer.ok_or(TradingSbfError::Content)?,
                    aliases,
                    accounts,
                    plan.payer_after,
                )?;
            }
            StateLifecyclePlanV3::Close(plan) => {
                set_account_candidate_v3(prepared.state, aliases, accounts, plan.source_after, 0)?;
                set_account_candidate_lamports_v3(
                    prepared.rent_credit.ok_or(TradingSbfError::Content)?,
                    aliases,
                    accounts,
                    plan.rent_credit_after,
                )?;
            }
        }
    }
    Ok(())
}

fn set_account_candidate_v3(
    representative: usize,
    aliases: &[usize],
    accounts: &mut [AccountInput],
    lamports: u64,
    data_len: usize,
) -> Result<(), ProgramError> {
    for (coordinate, alias) in aliases.iter().enumerate() {
        if *alias == representative {
            let account = accounts
                .get_mut(coordinate)
                .ok_or(TradingSbfError::Content)?;
            account.lamports = lamports;
            account.data_len = data_len;
        }
    }
    Ok(())
}

fn set_account_candidate_lamports_v3(
    representative: usize,
    aliases: &[usize],
    accounts: &mut [AccountInput],
    lamports: u64,
) -> Result<(), ProgramError> {
    for (coordinate, alias) in aliases.iter().enumerate() {
        if *alias == representative {
            accounts
                .get_mut(coordinate)
                .ok_or(TradingSbfError::Content)?
                .lamports = lamports;
        }
    }
    Ok(())
}

fn apply_lifecycle_creates_v3(
    program_id: &Pubkey,
    plans: &[PreparedLifecycleInvocationV3],
    accounts: &[&AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let system = accounts
        .iter()
        .find(|account| {
            account.key == &system_program::ID
                && account.executable
                && !account.is_signer
                && !account.is_writable
        })
        .copied();
    for prepared in plans {
        let StateLifecyclePlanV3::Create(plan) = prepared.plan else {
            continue;
        };
        let system = system.ok_or(TradingSbfError::Commit)?;
        let state = accounts
            .get(prepared.state)
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let payer = accounts
            .get(prepared.payer.ok_or(TradingSbfError::Commit)?)
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        if state.key.to_bytes() != plan.state
            || payer.key.to_bytes() != plan.payer
            || state.owner != &system_program::ID
            || state.data_len() != 0
            || state.lamports() != plan.state_before
            || payer.lamports()
                != plan
                    .payer_after
                    .checked_add(plan.payer_debit)
                    .ok_or(TradingSbfError::Commit)?
        {
            return Err(TradingSbfError::Commit.into());
        }
        if plan.payer_debit != 0 {
            invoke(
                &system_transfer(payer.key, state.key, plan.payer_debit),
                &[payer.clone(), state.clone(), system.clone()],
            )
            .map_err(|_| TradingSbfError::Commit)?;
        }
        let seed_slices = prepared.seeds.iter().map(Vec::as_slice).collect::<Vec<_>>();
        invoke_signed(
            &allocate(state.key, u64::from(plan.target_data_bytes)),
            &[state.clone(), system.clone()],
            &[seed_slices.as_slice()],
        )
        .map_err(|_| TradingSbfError::Commit)?;
        invoke_signed(
            &assign(state.key, program_id),
            &[state.clone(), system.clone()],
            &[seed_slices.as_slice()],
        )
        .map_err(|_| TradingSbfError::Commit)?;
        let data = state
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Commit)?;
        if state.owner != program_id
            || state.lamports() != plan.state_after
            || data.len()
                != usize::try_from(plan.target_data_bytes).map_err(|_| TradingSbfError::Commit)?
            || data.iter().any(|byte| *byte != 0)
            || payer.lamports() != plan.payer_after
        {
            return Err(TradingSbfError::Commit.into());
        }
    }
    Ok(())
}

fn apply_lifecycle_closes_v3(
    program_id: &Pubkey,
    expected_market: [u8; 32],
    expected_release_set: [u8; 32],
    expected_generation: u64,
    plans: &[PreparedLifecycleInvocationV3],
    accounts: &[&AccountInfo<'_>],
    rent: &Rent,
) -> Result<(), ProgramError> {
    for prepared in plans {
        let StateLifecyclePlanV3::Close(plan) = prepared.plan else {
            continue;
        };
        let state = accounts
            .get(prepared.state)
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let credit = accounts
            .get(prepared.rent_credit.ok_or(TradingSbfError::Commit)?)
            .copied()
            .ok_or(TradingSbfError::Commit)?;
        let authenticated_credit = authenticate_lifecycle_credit_v3(
            accounts,
            prepared.rent_credit.ok_or(TradingSbfError::Commit)?,
            credit.lamports(),
            rent,
            expected_market,
            expected_release_set,
            expected_generation,
        )?;
        if state.key.to_bytes() != plan.state
            || credit.key.to_bytes() != plan.rent_credit
            || state.owner != program_id
            || state.data_len()
                != usize::try_from(plan.source_data_bytes).map_err(|_| TradingSbfError::Commit)?
            || state.lamports() != plan.source_before
            || credit.lamports() != plan.rent_credit_before
            || authenticated_credit.beneficiary != plan.beneficiary
        {
            return Err(TradingSbfError::Commit.into());
        }
        state
            .try_borrow_mut_data()
            .map_err(|_| TradingSbfError::Commit)?
            .fill(0);
        **state
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)? = plan.source_after;
        **credit
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)? = plan.rent_credit_after;
        state.resize(0).map_err(|_| TradingSbfError::Commit)?;
        state.assign(&system_program::ID);
        if state.owner != &system_program::ID
            || state.data_len() != 0
            || state.lamports() != 0
            || credit.lamports() != plan.rent_credit_after
        {
            return Err(TradingSbfError::Commit.into());
        }
    }
    Ok(())
}

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn decode_claims_composition_boxed_v3<'request>(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    request_bank: &'request [u8],
    family_request: &'request [u8],
    parent: ClaimsCompositionParentV3,
) -> Result<Box<ClaimsCompositionV3<'request>>, ProgramError> {
    ClaimsCompositionV3::decode_selected_with_witness(
        effect.base(),
        tail_count,
        scalars,
        identities,
        request_bank,
        family_request,
        parent,
    )
    .map(Box::new)
    .map_err(|_| TradingSbfError::Content.into())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn preflight_child_routes_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: HotFrameV3<'accounts, 'info>,
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'info>],
    request_bank: &[u8],
    family_request: &[u8],
    request_digest: [u8; 32],
    envelope: HotExecutionEnvelopeV3,
    capability_program_set: [u8; 32],
    selected_capability_program: [u8; 32],
    aliases: &[usize],
) -> Result<(), ProgramError> {
    #[cfg(not(feature = "families"))]
    let _ = (
        request_digest,
        capability_program_set,
        selected_capability_program,
    );
    if effect.route_count() == 0 {
        return Ok(());
    }
    let locally_mutated =
        local_mutation_representatives(effect, tail_count, scalars, identities, aliases)?;
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let claims_composition =
        if has_active_role(effect, tail_count, scalars, identities, FixedRole::Claims)? {
            Some(decode_claims_composition_boxed_v3(
                effect,
                tail_count,
                scalars,
                identities,
                request_bank,
                family_request,
                ClaimsCompositionParentV3 {
                    release_set: envelope.release_set(),
                    market: envelope.market(),
                    generation: envelope.generation(),
                    parent_request_digest: request_digest,
                },
            )?)
        } else {
            None
        };
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let claims_program = if claims_composition.is_some() {
        Some(selected_role_program_v3(
            frame,
            effect_accounts,
            ExecutionRoleV1::Claims,
            envelope.release_set(),
        )?)
    } else {
        None
    };
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let custody_program =
        if has_active_role(effect, tail_count, scalars, identities, FixedRole::Custody)? {
            Some(selected_role_program_v3(
                frame,
                effect_accounts,
                ExecutionRoleV1::Custody,
                envelope.release_set(),
            )?)
        } else {
            None
        };
    #[cfg(feature = "families")]
    let resolution_program = if has_active_role(
        effect,
        tail_count,
        scalars,
        identities,
        FixedRole::Resolution,
    )? {
        Some(selected_role_program_v3(
            frame,
            effect_accounts,
            ExecutionRoleV1::Resolution,
            envelope.release_set(),
        )?)
    } else {
        None
    };

    let mut route = 0_u16;
    while route < effect.route_count() {
        let count = effect
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?;
        let mut invocation_index = 0_u32;
        while invocation_index < count {
            let invocation = effect
                .resolved_invocation(route, invocation_index, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Content)?;
            require_chain_receipt_width_v3(effect.base(), invocation)?;
            require_no_common_projection_child_accounts_v3(invocation)?;
            require_child_disjoint_from_local(invocation, aliases, &locally_mutated)?;
            match invocation.role {
                FixedRole::Core => preflight_core_route_v3(
                    program_id,
                    effect.base(),
                    route,
                    invocation_index,
                    tail_count,
                    scalars,
                    identities,
                    effect_accounts,
                    request_bank,
                    family_request,
                    frame.core_program,
                    CoreCompositionParentV3 {
                        release_set: envelope.release_set(),
                        market: envelope.market(),
                        generation: envelope.generation(),
                        trading_program: program_id.to_bytes(),
                    },
                )?,
                FixedRole::Claims => {
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    {
                        let composition = claims_composition
                            .as_deref()
                            .ok_or(TradingSbfError::Content)?;
                        let selected = claims_program.ok_or(TradingSbfError::Release)?;
                        if invocation_index != 0
                            || !(composition.admit_route() == Some(route)
                                || composition.mutation_route() == route
                                || composition.close_route() == Some(route))
                            || invocation_accounts_contain_program(
                                invocation,
                                effect_accounts,
                                selected.key,
                            )? != 1
                        {
                            return Err(TradingSbfError::Content.into());
                        }
                    }
                    #[cfg(not(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    )))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Custody => {
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    preflight_custody_route_v3(
                        program_id,
                        effect.base(),
                        route,
                        invocation_index,
                        tail_count,
                        scalars,
                        identities,
                        effect_accounts,
                        request_bank,
                        custody_program.ok_or(TradingSbfError::Release)?,
                        CustodyCompositionParentV3 {
                            release_set: envelope.release_set(),
                            market: envelope.market(),
                            generation: envelope.generation(),
                            parent_request_digest: request_digest,
                            trading_program: program_id.to_bytes(),
                        },
                    )?;
                    #[cfg(not(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    )))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Resolution => {
                    #[cfg(feature = "families")]
                    preflight_resolution_route_v3(
                        program_id,
                        effect.base(),
                        route,
                        invocation_index,
                        tail_count,
                        scalars,
                        identities,
                        effect_accounts,
                        request_bank,
                        family_request,
                        resolution_program.ok_or(TradingSbfError::Release)?,
                        ResolutionCompositionParentV3 {
                            release_set: envelope.release_set(),
                            market: envelope.market(),
                            generation: envelope.generation(),
                            parent_request_digest: request_digest,
                            trading_program: program_id.to_bytes(),
                            capability_program_set,
                            selected_capability_program,
                            activation_account: frame.activation_cache.key.to_bytes(),
                        },
                    )?;
                    #[cfg(not(feature = "families"))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
            }
            invocation_index = invocation_index
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?;
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(())
}

struct ChildExecutionStateV3 {
    transcript: [u8; 32],
    receipt_bank: ChildReceiptBankV3,
    prior_receipt_bytes: Vec<u8>,
    route: u16,
}

// The sole additional allocation introduced by the verifier-frame split is
// this bounded 88-byte header. Receipt payloads already lived in Vec-backed
// storage before this split; no authenticated fact or commit authority moves
// from account data into the heap.
const _: [(); 88] = [(); core::mem::size_of::<ChildExecutionStateV3>()];

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn execute_child_routes_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: HotFrameV3<'accounts, 'info>,
    request_profile: RequestProfileKindV3<'_>,
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    effect_accounts: &[AccountInfo<'info>],
    request_bank: &[u8],
    family_request: &[u8],
    request_digest: [u8; 32],
    envelope: HotExecutionEnvelopeV3,
    capability_program_set: [u8; 32],
    selected_capability_program: [u8; 32],
) -> Result<[u8; 32], ProgramError> {
    #[cfg(not(feature = "families"))]
    let _ = (capability_program_set, selected_capability_program);
    let mut execution = Box::new(ChildExecutionStateV3 {
        transcript: hashv(&[CHILD_EXECUTION_DIGEST_DOMAIN_V3, &request_digest]).to_bytes(),
        receipt_bank: ChildReceiptBankV3::new(),
        prior_receipt_bytes: Vec::new(),
        route: 0,
    });
    if effect.route_count() == 0 {
        return Ok(execution.transcript);
    }
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let claims_composition =
        if has_active_role(effect, tail_count, scalars, identities, FixedRole::Claims)? {
            Some(decode_claims_composition_boxed_v3(
                effect,
                tail_count,
                scalars,
                identities,
                request_bank,
                family_request,
                ClaimsCompositionParentV3 {
                    release_set: envelope.release_set(),
                    market: envelope.market(),
                    generation: envelope.generation(),
                    parent_request_digest: request_digest,
                },
            )?)
        } else {
            None
        };
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let claims_program = if claims_composition.is_some() {
        Some(selected_role_program_v3(
            frame,
            effect_accounts,
            ExecutionRoleV1::Claims,
            envelope.release_set(),
        )?)
    } else {
        None
    };
    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    let custody_program =
        if has_active_role(effect, tail_count, scalars, identities, FixedRole::Custody)? {
            Some(selected_role_program_v3(
                frame,
                effect_accounts,
                ExecutionRoleV1::Custody,
                envelope.release_set(),
            )?)
        } else {
            None
        };
    #[cfg(feature = "families")]
    let resolution_program = if has_active_role(
        effect,
        tail_count,
        scalars,
        identities,
        FixedRole::Resolution,
    )? {
        Some(selected_role_program_v3(
            frame,
            effect_accounts,
            ExecutionRoleV1::Resolution,
            envelope.release_set(),
        )?)
    } else {
        None
    };

    while execution.route < effect.route_count() {
        let route = execution.route;
        let count = effect
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?;
        let mut invocation = 0_u32;
        while invocation < count {
            let resolved = effect
                .resolved_invocation(route, invocation, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Content)?;
            execution.prior_receipt_bytes.clear();
            let mut dependency_index = 0_u16;
            while dependency_index < resolved.receipt_dependencies.len() {
                let dependency = effect
                    .resolved_receipt_dependency(resolved.receipt_dependencies, dependency_index)
                    .map_err(|_| TradingSbfError::Content)?;
                let dependency_program = match dependency.producer_role {
                    FixedRole::Core => frame.core_program,
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    FixedRole::Claims => claims_program.ok_or(TradingSbfError::Release)?,
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    FixedRole::Custody => custody_program.ok_or(TradingSbfError::Release)?,
                    #[cfg(feature = "families")]
                    FixedRole::Resolution => resolution_program.ok_or(TradingSbfError::Release)?,
                    #[cfg(not(feature = "families"))]
                    _ => return Err(TradingSbfError::UnsupportedContent.into()),
                };
                let producer_invocation = effect
                    .resolved_invocation(
                        dependency.producer_route,
                        dependency.producer_invocation,
                        tail_count,
                        scalars,
                        identities,
                    )
                    .map_err(|_| TradingSbfError::Content)?;
                let expected_provenance = child_receipt_provenance_v4(
                    producer_invocation,
                    dependency.producer_role,
                    dependency.producer_route,
                    dependency.producer_invocation,
                    dependency_program.key,
                    envelope.release_set(),
                    envelope.market(),
                    envelope.generation(),
                    request_digest,
                    request_bank,
                    family_request,
                )?;
                let receipt = execution
                    .receipt_bank
                    .resolve(
                        Some(dependency),
                        Some(dependency_program.key),
                        Some(expected_provenance),
                    )?
                    .ok_or(TradingSbfError::Transition)?;
                execution
                    .prior_receipt_bytes
                    .try_reserve(receipt.len())
                    .map_err(|_| TradingSbfError::Content)?;
                execution.prior_receipt_bytes.extend_from_slice(receipt);
                dependency_index = dependency_index
                    .checked_add(1)
                    .ok_or(TradingSbfError::Content)?;
            }
            let prior_receipt = if execution.prior_receipt_bytes.is_empty() {
                None
            } else {
                Some(execution.prior_receipt_bytes.as_slice())
            };
            let (role, child_digest, child_program) = match resolved.role {
                FixedRole::Core => (
                    FixedRole::Core,
                    execute_core_route_v3(
                        program_id,
                        effect.base(),
                        route,
                        invocation,
                        tail_count,
                        scalars,
                        identities,
                        effect_accounts,
                        request_bank,
                        family_request,
                        prior_receipt,
                        frame.core_program,
                        CoreCompositionParentV3 {
                            release_set: envelope.release_set(),
                            market: envelope.market(),
                            generation: envelope.generation(),
                            trading_program: program_id.to_bytes(),
                        },
                    )?,
                    frame.core_program,
                ),
                FixedRole::Claims => {
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    {
                        let receipt = execute_claims_route_v3(
                            program_id,
                            effect.base(),
                            claims_composition
                                .as_deref()
                                .copied()
                                .ok_or(TradingSbfError::Content)?,
                            route,
                            tail_count,
                            scalars,
                            identities,
                            effect_accounts,
                            request_bank,
                            family_request,
                            prior_receipt,
                            claims_program.ok_or(TradingSbfError::Release)?,
                        )?;
                        (
                            FixedRole::Claims,
                            claims_receipt_digest_v3(receipt)?,
                            claims_program.ok_or(TradingSbfError::Release)?,
                        )
                    }
                    #[cfg(not(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    )))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Custody => {
                    #[cfg(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    ))]
                    {
                        let digest = execute_custody_route_v3(
                            program_id,
                            effect.base(),
                            route,
                            invocation,
                            tail_count,
                            scalars,
                            identities,
                            effect_accounts,
                            request_bank,
                            prior_receipt,
                            custody_program.ok_or(TradingSbfError::Release)?,
                            CustodyCompositionParentV3 {
                                release_set: envelope.release_set(),
                                market: envelope.market(),
                                generation: envelope.generation(),
                                parent_request_digest: request_digest,
                                trading_program: program_id.to_bytes(),
                            },
                        )?;
                        (
                            FixedRole::Custody,
                            digest,
                            custody_program.ok_or(TradingSbfError::Release)?,
                        )
                    }
                    #[cfg(not(any(
                        feature = "families",
                        feature = "series-family",
                        feature = "dealer-family"
                    )))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Resolution => {
                    #[cfg(feature = "families")]
                    {
                        let digest = execute_resolution_route_v3(
                            program_id,
                            effect.base(),
                            route,
                            invocation,
                            tail_count,
                            scalars,
                            identities,
                            effect_accounts,
                            request_bank,
                            family_request,
                            prior_receipt,
                            resolution_program.ok_or(TradingSbfError::Release)?,
                            ResolutionCompositionParentV3 {
                                release_set: envelope.release_set(),
                                market: envelope.market(),
                                generation: envelope.generation(),
                                parent_request_digest: request_digest,
                                trading_program: program_id.to_bytes(),
                                capability_program_set,
                                selected_capability_program,
                                activation_account: frame.activation_cache.key.to_bytes(),
                            },
                        )?;
                        (
                            FixedRole::Resolution,
                            digest,
                            resolution_program.ok_or(TradingSbfError::Release)?,
                        )
                    }
                    #[cfg(not(feature = "families"))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
            };
            let (producer, receipt_bytes) = get_return_data().ok_or(TradingSbfError::Transition)?;
            if producer != *child_program.key {
                return Err(TradingSbfError::Transition.into());
            }
            require_borrowed_witness_receipt_v3(request_profile, resolved, role, &receipt_bytes)?;
            let provenance = child_receipt_provenance_v4(
                resolved,
                role,
                route,
                invocation,
                child_program.key,
                envelope.release_set(),
                envelope.market(),
                envelope.generation(),
                request_digest,
                request_bank,
                family_request,
            )?;
            let receipt_kind: [u8; 8] = receipt_bytes
                .get(..8)
                .ok_or(TradingSbfError::Transition)?
                .try_into()
                .map_err(|_| TradingSbfError::Transition)?;
            let receipt_digest = hash(&receipt_bytes).to_bytes();
            execution.receipt_bank.record_exact(
                role,
                route,
                invocation,
                producer,
                provenance.context_digest,
                provenance.request_kind,
                provenance.request_digest,
                receipt_kind,
                receipt_bytes,
            )?;
            execution.transcript = hashv(&[
                CHILD_EXECUTION_DIGEST_DOMAIN_V3,
                &execution.transcript,
                &[fixed_role_tag_v3(role)],
                &route.to_le_bytes(),
                &invocation.to_le_bytes(),
                child_program.key.as_ref(),
                &receipt_digest,
                &child_digest,
            ])
            .to_bytes();
            invocation = invocation.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        execution.route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(execution.transcript)
}

fn fixed_role_tag_v3(role: FixedRole) -> u8 {
    match role {
        FixedRole::Core => 0,
        FixedRole::Claims => 1,
        FixedRole::Resolution => 3,
        FixedRole::Custody => 4,
    }
}

#[allow(clippy::too_many_arguments)]
fn child_receipt_provenance_v4(
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    role: FixedRole,
    route: u16,
    invocation_index: u32,
    child_program: &Pubkey,
    release_set: [u8; 32],
    market: [u8; 32],
    generation: u64,
    parent_request_digest: [u8; 32],
    request_bank: &[u8],
    family_request: &[u8],
) -> Result<ExpectedReceiptProvenanceV4, ProgramError> {
    let request_end = invocation
        .request_offset
        .checked_add(invocation.request_len)
        .ok_or(TradingSbfError::Content)?;
    let child_request = request_bank
        .get(invocation.request_offset..request_end)
        .ok_or(TradingSbfError::Content)?;
    let borrowed_request = invocation
        .borrowed_witness
        .map(|witness| {
            witness
                .slice(family_request)
                .map_err(|_| TradingSbfError::Content)
        })
        .transpose()?;
    let request_kind_source = if child_request.len() >= 8 {
        child_request
    } else if child_request.is_empty() {
        borrowed_request.ok_or(TradingSbfError::Content)?
    } else {
        return Err(TradingSbfError::Content.into());
    };
    let request_kind = request_kind_source
        .get(..8)
        .ok_or(TradingSbfError::Content)?
        .try_into()
        .map_err(|_| TradingSbfError::Content)?;
    let request_digest = match borrowed_request {
        Some(witness) => {
            hashv(&[CHILD_REQUEST_DIGEST_DOMAIN_V4, child_request, witness]).to_bytes()
        }
        None => hashv(&[CHILD_REQUEST_DIGEST_DOMAIN_V4, child_request]).to_bytes(),
    };
    let context_digest = hashv(&[
        CHILD_RECEIPT_CONTEXT_DOMAIN_V4,
        &release_set,
        &market,
        &generation.to_le_bytes(),
        &parent_request_digest,
        &[fixed_role_tag_v3(role)],
        &route.to_le_bytes(),
        &invocation_index.to_le_bytes(),
        child_program.as_ref(),
        &request_digest,
    ])
    .to_bytes();
    Ok(ExpectedReceiptProvenanceV4 {
        context_digest,
        request_kind,
        request_digest,
    })
}

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn has_active_role(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    role: FixedRole,
) -> Result<bool, ProgramError> {
    let mut route = 0_u16;
    while route < effect.route_count() {
        if effect
            .route(route)
            .map_err(|_| TradingSbfError::Content)?
            .role()
            == role
            && effect
                .invocation_count(route, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Content)?
                != 0
        {
            return Ok(true);
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(false)
}

fn local_mutation_representatives(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    aliases: &[usize],
) -> Result<Vec<bool>, ProgramError> {
    let mut output = vec![false; aliases.len()];
    let mut fixed = 0_u16;
    while fixed < effect.fixed_operation_count() {
        mark_local_mutation(
            effect
                .resolved_fixed_effect(fixed, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Content)?,
            aliases,
            &mut output,
        )?;
        fixed = fixed.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    let mut item = 0_u32;
    while item < tail_count {
        let mut operation = 0_u16;
        while operation < effect.item_operation_count() {
            mark_local_mutation(
                effect
                    .resolved_item_effect(item, operation, tail_count, scalars, identities)
                    .map_err(|_| TradingSbfError::Content)?,
                aliases,
                &mut output,
            )?;
            operation = operation.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(output)
}

fn mark_local_mutation(
    effect: ResolvedEffectV3,
    aliases: &[usize],
    output: &mut [bool],
) -> Result<(), ProgramError> {
    let coordinates = match effect {
        ResolvedEffectV3::TransferLamports {
            source,
            destination,
            ..
        } => [Some(source), Some(destination)],
        ResolvedEffectV3::WriteScalar { account, .. }
        | ResolvedEffectV3::WriteIdentity { account, .. }
        | ResolvedEffectV3::WriteU8 { account, .. }
        | ResolvedEffectV3::WriteU16 { account, .. }
        | ResolvedEffectV3::WriteU32 { account, .. } => [Some(account), None],
        ResolvedEffectV3::RequireLamportsEq { .. } | ResolvedEffectV3::WriteRequest { .. } => {
            [None, None]
        }
    };
    for coordinate in coordinates.into_iter().flatten() {
        let representative = *aliases.get(coordinate).ok_or(TradingSbfError::Content)?;
        *output
            .get_mut(representative)
            .ok_or(TradingSbfError::Content)? = true;
    }
    Ok(())
}

fn require_child_disjoint_from_local(
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    aliases: &[usize],
    locally_mutated: &[bool],
) -> Result<(), ProgramError> {
    let mut coordinates = Vec::new();
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_end = fixed_start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    coordinates.extend(fixed_start..fixed_end);
    let item_count = usize::from(invocation.item_account_count);
    let stride = usize::from(invocation.item_account_stride);
    let mut item = 0_u32;
    while item < invocation.repeated_item_count {
        let start = invocation
            .item_account_start
            .checked_add(
                usize::try_from(item)
                    .map_err(|_| TradingSbfError::Content)?
                    .checked_mul(stride)
                    .ok_or(TradingSbfError::Content)?,
            )
            .ok_or(TradingSbfError::Content)?;
        let end = start
            .checked_add(item_count)
            .ok_or(TradingSbfError::Content)?;
        coordinates.extend(start..end);
        item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    for coordinate in coordinates {
        let representative = *aliases.get(coordinate).ok_or(TradingSbfError::Content)?;
        if locally_mutated
            .get(representative)
            .copied()
            .ok_or(TradingSbfError::Content)?
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    Ok(())
}

fn require_no_common_projection_child_accounts_v3(
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
) -> Result<(), ProgramError> {
    const RESERVED_END: usize = 5;
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_count = usize::from(invocation.fixed_account_count);
    let fixed_end = fixed_start
        .checked_add(fixed_count)
        .ok_or(TradingSbfError::Content)?;
    if fixed_count != 0 && fixed_start < RESERVED_END && fixed_end > 0 {
        return Err(TradingSbfError::Content.into());
    }
    let item_count = usize::from(invocation.item_account_count);
    let stride = usize::from(invocation.item_account_stride);
    let mut item = 0_u32;
    while item < invocation.repeated_item_count {
        let start = invocation
            .item_account_start
            .checked_add(
                usize::try_from(item)
                    .map_err(|_| TradingSbfError::Content)?
                    .checked_mul(stride)
                    .ok_or(TradingSbfError::Content)?,
            )
            .ok_or(TradingSbfError::Content)?;
        let end = start
            .checked_add(item_count)
            .ok_or(TradingSbfError::Content)?;
        if item_count != 0 && start < RESERVED_END && end > 0 {
            return Err(TradingSbfError::Content.into());
        }
        item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(())
}

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn invocation_accounts_contain_program(
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    accounts: &[AccountInfo<'_>],
    program: &Pubkey,
) -> Result<usize, ProgramError> {
    let fixed_start = usize::from(invocation.fixed_account_start);
    let fixed_end = fixed_start
        .checked_add(usize::from(invocation.fixed_account_count))
        .ok_or(TradingSbfError::Content)?;
    let mut count = accounts
        .get(fixed_start..fixed_end)
        .ok_or(TradingSbfError::Content)?
        .iter()
        .filter(|account| account.key == program)
        .count();
    let item_count = usize::from(invocation.item_account_count);
    let stride = usize::from(invocation.item_account_stride);
    let mut item = 0_u32;
    while item < invocation.repeated_item_count {
        let start = invocation
            .item_account_start
            .checked_add(
                usize::try_from(item)
                    .map_err(|_| TradingSbfError::Content)?
                    .checked_mul(stride)
                    .ok_or(TradingSbfError::Content)?,
            )
            .ok_or(TradingSbfError::Content)?;
        let end = start
            .checked_add(item_count)
            .ok_or(TradingSbfError::Content)?;
        count = count
            .checked_add(
                accounts
                    .get(start..end)
                    .ok_or(TradingSbfError::Content)?
                    .iter()
                    .filter(|account| account.key == program)
                    .count(),
            )
            .ok_or(TradingSbfError::Content)?;
        item = item.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(count)
}

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn selected_role_program_v3<'accounts, 'info>(
    frame: HotFrameV3<'_, 'info>,
    accounts: &'accounts [AccountInfo<'info>],
    role: ExecutionRoleV1,
    release_set: [u8; 32],
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    let cache = frame
        .activation_cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&cache).map_err(|_| TradingSbfError::Release)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| TradingSbfError::Release)?
        .to_bytes()
        != release_set
    {
        return Err(TradingSbfError::Release.into());
    }
    let expected = activated
        .role(role)
        .map_err(|_| TradingSbfError::Release)?
        .release()
        .program()
        .to_bytes();
    drop(cache);
    let mut found = None;
    for account in accounts {
        if account.key.to_bytes() == expected {
            if found.is_some() || !account.executable || account.is_signer || account.is_writable {
                return Err(TradingSbfError::Release.into());
            }
            found = Some(account);
        }
    }
    found.ok_or_else(|| TradingSbfError::Release.into())
}

#[cfg(any(
    feature = "families",
    feature = "series-family",
    feature = "dealer-family"
))]
fn claims_receipt_digest_v3(receipt: ClaimsRouteReceiptV3) -> Result<[u8; 32], ProgramError> {
    let bytes = match receipt {
        ClaimsRouteReceiptV3::Admit(value) => value
            .to_receipt_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
        ClaimsRouteReceiptV3::Affine(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::SignedDelta(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::SparseNativeTransfer(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::Founding(value) => Vec::from(value.to_bytes()),
        ClaimsRouteReceiptV3::RationalLifecycle(value) => value
            .to_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
        ClaimsRouteReceiptV3::RationalRepresentation(value) => value
            .to_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
        ClaimsRouteReceiptV3::Close(value) => value
            .to_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
    };
    Ok(hashv(&[CHILD_EXECUTION_DIGEST_DOMAIN_V3, &bytes]).to_bytes())
}

#[derive(Clone, Copy)]
enum RequestProfileKindV3<'a> {
    Unsigned(RequestProfileV1<'a>),
    Signed(RequestProfileV2<'a>),
    Borrowed(RequestProfileV3<'a>),
    RepeatedRows(RequestProfileV4<'a>),
}

impl<'a> RequestProfileKindV3<'a> {
    const fn v1(self) -> RequestProfileV1<'a> {
        match self {
            Self::Unsigned(profile) => profile,
            Self::Signed(profile) => profile.request_profile(),
            Self::Borrowed(profile) => profile.request_profile(),
            Self::RepeatedRows(profile) => profile.request_profile(),
        }
    }

    fn writes_register(self, target: ProjectionTargetV1) -> Result<bool, ProgramError> {
        match self {
            Self::RepeatedRows(profile) => profile
                .writes_register(target)
                .map_err(|_| TradingSbfError::Content.into()),
            Self::Unsigned(_) | Self::Signed(_) | Self::Borrowed(_) => self
                .v1()
                .writes_register(target)
                .map_err(|_| TradingSbfError::Content.into()),
        }
    }

    fn writes_any_register(self, targets: &[ProjectionTargetV1]) -> Result<bool, ProgramError> {
        match self {
            Self::RepeatedRows(profile) => profile
                .writes_any_register(targets)
                .map_err(|_| TradingSbfError::Content.into()),
            Self::Unsigned(_) | Self::Signed(_) | Self::Borrowed(_) => self
                .v1()
                .writes_any_register(targets)
                .map_err(|_| TradingSbfError::Content.into()),
        }
    }

    fn project_atomic(
        self,
        tail_count: u32,
        family_request: &'a [u8],
        registers: ProjectionRegistersV1<'_>,
    ) -> Result<(), ProgramError> {
        match self {
            Self::Unsigned(profile) => {
                project_request_atomic(profile, tail_count, family_request, registers)
                    .map_err(|_| TradingSbfError::Content.into())
            }
            Self::Signed(profile) => project_request_atomic(
                profile.request_profile(),
                tail_count,
                family_request,
                registers,
            )
            .map_err(|_| TradingSbfError::Content.into()),
            Self::Borrowed(profile) => profile
                .project_prefix_atomic(tail_count, family_request, registers)
                .map_err(|_| TradingSbfError::Content.into()),
            Self::RepeatedRows(profile) => {
                let mut candidate_scalars = vec![0_u64; registers.output_scalars.len()];
                let mut candidate_identities = vec![[0_u8; 32]; registers.output_identities.len()];
                profile
                    .project_atomic(
                        family_request,
                        ProjectionRegistersV4 {
                            input_scalars: registers.input_scalars,
                            input_identities: registers.input_identities,
                            scratch_scalars: registers.scratch_scalars,
                            scratch_identities: registers.scratch_identities,
                            candidate_scalars: &mut candidate_scalars,
                            candidate_identities: &mut candidate_identities,
                            output_scalars: registers.output_scalars,
                            output_identities: registers.output_identities,
                        },
                    )
                    .map_err(|_| TradingSbfError::Content.into())
            }
        }
    }

    fn require_request_shape(
        self,
        tail_count: u32,
        family_request: &'a [u8],
    ) -> Result<(), ProgramError> {
        match self {
            Self::Borrowed(profile) => profile
                .split_request(tail_count, family_request)
                .map(|_| ())
                .map_err(|_| TradingSbfError::Content.into()),
            Self::RepeatedRows(profile) => {
                if profile
                    .request_bytes()
                    .map_err(|_| TradingSbfError::Content)?
                    == family_request.len()
                {
                    Ok(())
                } else {
                    Err(TradingSbfError::Content.into())
                }
            }
            Self::Unsigned(_) | Self::Signed(_) => {
                if self
                    .v1()
                    .request_bytes(tail_count)
                    .map_err(|_| TradingSbfError::Content)?
                    == family_request.len()
                {
                    Ok(())
                } else {
                    Err(TradingSbfError::Content.into())
                }
            }
        }
    }
}

fn decode_request_profile<'a>(
    descriptor: CapabilityProgramV4,
    bytes: &'a [u8],
) -> Result<RequestProfileKindV3<'a>, ProgramError> {
    let selected = descriptor.request_profile().program().to_bytes();
    let authenticated = hash(bytes).to_bytes();
    if descriptor.request_profile().schema().to_bytes() == REQUEST_PROFILE_SCHEMA_ID_V1 {
        RequestProfileV1::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::Unsigned)
            .map_err(|_| TradingSbfError::Content.into())
    } else if descriptor.request_profile().schema().to_bytes()
        == REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID
    {
        RequestProfileV2::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::Signed)
            .map_err(|_| TradingSbfError::Content.into())
    } else if descriptor.request_profile().schema().to_bytes()
        == REQUEST_PROFILE_V3_SCHEMA_RELEASE_ID
    {
        RequestProfileV3::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::Borrowed)
            .map_err(|_| TradingSbfError::Content.into())
    } else if descriptor.request_profile().schema().to_bytes()
        == REQUEST_PROFILE_V4_SCHEMA_RELEASE_ID
    {
        RequestProfileV4::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::RepeatedRows)
            .map_err(|_| TradingSbfError::Content.into())
    } else {
        Err(TradingSbfError::UnsupportedContent.into())
    }
}

#[inline(never)]
fn decode_selected_effect_v4<'a>(
    schema: [u8; 32],
    bytes: &'a [u8],
) -> Result<SelectedEffectProgramV4<'a>, ProgramError> {
    if schema != EFFECT_SCHEMA_ID_V4 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let successor = EffectProgramV4::decode(bytes).map_err(|_| TradingSbfError::Content)?;
    // Profile13 and the EffectV4 kernel jointly own selected account spans.
    // Borrowed family ranges remain fail-closed until the child-request append
    // and continuation-window path lands as one coherent boundary.
    if successor.range_count() != 0 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    Ok(SelectedEffectProgramV4 {
        base: successor.base(),
        successor,
    })
}

#[allow(clippy::too_many_arguments)]
fn require_geometry(
    account: AccountProfileV2<'_>,
    request: RequestProfileKindV3<'_>,
    transition: TransitionProgramV3<'_>,
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    family_request: &[u8],
    runtime_accounts: usize,
    span_counts: &[u32],
    preprojected_scalars: &[u64],
) -> Result<(), ProgramError> {
    request.require_request_shape(tail_count, family_request)?;
    let request_v1 = request.v1();
    let expected_accounts = if account.uses_dynamic_fixed_spans() {
        account
            .logical_account_count_with_dynamic_spans(tail_count, span_counts)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        if !span_counts.is_empty() {
            return Err(TradingSbfError::Content.into());
        }
        account
            .logical_account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    };
    let effect_accounts = if preprojected_scalars.is_empty() {
        effect
            .base()
            .account_count(tail_count)
            .map_err(|_| TradingSbfError::Content)?
    } else {
        effect
            .successor
            .account_count(tail_count, preprojected_scalars)
            .map_err(|_| TradingSbfError::Content)?
    };
    if expected_accounts != runtime_accounts
        || effect_accounts > expected_accounts
        || account.fixed_account_count() != effect.fixed_account_count()
        || account.item_account_stride() != effect.item_account_stride()
        || account.common_scalar_count() != request_v1.common_scalar_count()
        || account.item_scalar_stride() != request_v1.item_scalar_stride()
        || account.common_identity_count() != request_v1.common_identity_count()
        || account.item_identity_stride() != request_v1.item_identity_stride()
        || account.common_scalar_count() != transition.common_scalar_count()
        || account.item_scalar_stride() != transition.item_scalar_stride()
        || account.common_identity_count() != transition.common_identity_count()
        || account.item_identity_stride() != transition.item_identity_stride()
        || account.common_scalar_count() != effect.common_scalar_count()
        || account.item_scalar_stride() != effect.item_scalar_stride()
        || account.common_identity_count() != effect.common_identity_count()
        || account.item_identity_stride() != effect.item_identity_stride()
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

#[inline(never)]
fn require_borrowed_witness_coverage_v3<'a>(
    request_profile: RequestProfileKindV3<'a>,
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    family_request: &'a [u8],
) -> Result<(), ProgramError> {
    let RequestProfileKindV3::Borrowed(profile) = request_profile else {
        return Ok(());
    };
    let (_, declared_witness) = profile
        .split_request(tail_count, family_request)
        .map_err(|_| TradingSbfError::Content)?;
    let policy = profile.witness_policy();
    let expected_role = borrowed_witness_role_v3(policy.consumer_role);
    let mut borrower_count = 0_u16;
    let mut route_index = 0_u16;
    while route_index < effect.route_count() {
        let route = effect
            .route(route_index)
            .map_err(|_| TradingSbfError::Content)?;
        if route.borrows_witness() {
            borrower_count = borrower_count
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?;
            if route.role() != expected_role
                || route.kind() != dclutch_effect_kernel::v3::RouteKindV3::Once
                || route.fixed_request_bytes() != 0
                || route.item_request_bytes() != 0
                || effect
                    .invocation_count(route_index, tail_count, scalars, identities)
                    .map_err(|_| TradingSbfError::Content)?
                    != 1
            {
                return Err(TradingSbfError::Content.into());
            }
            let invocation = effect
                .resolved_invocation(route_index, 0, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Content)?;
            let witness = invocation
                .borrowed_witness
                .ok_or(TradingSbfError::Content)?;
            if invocation.request_len != 0
                || witness
                    .slice(family_request)
                    .map_err(|_| TradingSbfError::Content)?
                    != declared_witness
            {
                return Err(TradingSbfError::Content.into());
            }
        }
        route_index = route_index.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    if borrower_count != 1 {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

const fn borrowed_witness_role_v3(role: BorrowedWitnessRoleV3) -> FixedRole {
    match role {
        BorrowedWitnessRoleV3::Core => FixedRole::Core,
        BorrowedWitnessRoleV3::Claims => FixedRole::Claims,
        BorrowedWitnessRoleV3::Resolution => FixedRole::Resolution,
        BorrowedWitnessRoleV3::Custody => FixedRole::Custody,
    }
}

fn require_borrowed_witness_receipt_v3(
    request_profile: RequestProfileKindV3<'_>,
    invocation: dclutch_effect_kernel::v3::ResolvedInvocationV3,
    role: FixedRole,
    receipt: &[u8],
) -> Result<(), ProgramError> {
    let RequestProfileKindV3::Borrowed(profile) = request_profile else {
        return Ok(());
    };
    if invocation.borrowed_witness.is_none() {
        return Ok(());
    }
    let policy: BorrowedWitnessPolicyV3 = profile.witness_policy();
    if role != borrowed_witness_role_v3(policy.consumer_role)
        || receipt.len()
            != usize::try_from(policy.child_receipt_bytes).map_err(|_| TradingSbfError::Content)?
        || receipt.get(..8) != Some(policy.child_receipt_magic.as_slice())
    {
        Err(TradingSbfError::Transition.into())
    } else {
        Ok(())
    }
}

/// Resolve every runtime coordinate to its canonical representative once.
///
/// Exact capacity: a fallible `collect` reports a zero lower bound, which walks
/// the never-freeing SBF bump allocator through its whole doubling ladder.
fn representative_coordinates_v3(
    profile: AccountProfileV2<'_>,
    tail_count: u32,
    span_counts: &[u32],
    runtime_account_count: usize,
) -> Result<Vec<usize>, ProgramError> {
    let dynamic = profile.uses_dynamic_fixed_spans();
    let mut output = Vec::with_capacity(runtime_account_count);
    for coordinate in 0..runtime_account_count {
        let representative = if dynamic {
            profile.representative_with_dynamic_spans(tail_count, span_counts, coordinate)
        } else {
            profile.representative(tail_count, coordinate)
        }
        .map_err(|_| TradingSbfError::Content)?;
        output.push(representative);
    }
    Ok(output)
}

/// Resolve the authenticated runtime tail width for one selected profile.
///
/// A profile that declares a tail-count projection binds its own tail scalar
/// to the independently authenticated Product Runtime V3 outcome count. That
/// binding is *checked*, not assumed: the full account projection runs at this
/// width in `project_account_and_request_registers_v3`, and
/// `require_projected_tail_count_agreement_v3` refuses unless the profile's own
/// projected tail scalar equals the same authenticated count.
///
/// Discovering the width by running the account projection at a fictitious
/// `tail_count` of zero cannot work and was never load-bearing. It cannot work
/// because a fixed rule with a nonzero `data_item_stride` — Profile 14's
/// Portfolio, linked-basis and Claims records among them — has no valid width
/// at tail zero, so `validate_accounts` refuses with `DataLengthMismatch`
/// before the projection reads anything. It was never load-bearing because the
/// only consumer of the discovered value immediately required it to equal the
/// authenticated Product outcome count anyway.
fn project_tail_count(
    profile: AccountProfileV2<'_>,
    authenticated_product_tail_count: u32,
) -> Result<u32, ProgramError> {
    if profile
        .tail_count_projection()
        .map_err(|_| TradingSbfError::Content)?
        .is_none()
    {
        return Ok(0);
    }
    Ok(authenticated_product_tail_count)
}

fn require_projected_tail_count_agreement_v3(
    profile: AccountProfileV2<'_>,
    authenticated_product_tail_count: u32,
    scalars: &[u64],
) -> Result<(), ProgramError> {
    let Some(projection) = profile
        .tail_count_projection()
        .map_err(|_| TradingSbfError::Content)?
    else {
        return Ok(());
    };
    if scalars.get(usize::from(projection.register()))
        != Some(&u64::from(authenticated_product_tail_count))
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

struct ProjectedRequestRegistersV3 {
    scalars: Vec<u64>,
    identities: Vec<[u8; 32]>,
}

/// Keep the transient Account/Request projection banks in one noinline phase.
/// Only the final candidate registers cross the boundary; scratch banks never
/// remain live across child CPI or commit-last execution.
#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn project_account_and_request_registers_v3<'artifact, 'accounts, 'info>(
    current_instruction: u16,
    native_message_offset_bias: u16,
    instruction_data: &'artifact [u8],
    frame: HotFrameV3<'accounts, 'info>,
    account_profile: AccountProfileV2<'artifact>,
    request_profile: RequestProfileKindV3<'artifact>,
    lifecycle: StateLifecyclePolicyV5<'artifact>,
    current_rent_quotes: &[AuthenticatedRentQuoteV5],
    span_counts: &[u32],
    tail_count: u32,
    observations: &[AccountObservationV1<'_>],
    family_request: &'artifact [u8],
    request_digest: [u8; 32],
    trusted_environment: TrustedEnvironmentObservationV3,
    authenticated_product_tail_count: u32,
    scalar_count: usize,
    identity_count: usize,
) -> Result<ProjectedRequestRegistersV3, ProgramError> {
    let mut input_scalars = vec![0_u64; scalar_count];
    let mut input_identities = vec![[0_u8; 32]; identity_count];
    *input_identities
        .get_mut(HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3)
        .ok_or(TradingSbfError::Content)? = request_digest;
    seed_trusted_environment_v3(
        trusted_environment,
        &mut input_scalars,
        &mut input_identities,
    )?;
    if account_profile.uses_dynamic_fixed_spans() {
        if span_counts.len() != usize::from(account_profile.dynamic_fixed_span_count()) {
            return Err(TradingSbfError::Content.into());
        }
        let mut index = 0_u16;
        while index < account_profile.dynamic_fixed_span_count() {
            let span = account_profile
                .dynamic_fixed_span(index)
                .map_err(|_| TradingSbfError::Content)?;
            *input_scalars
                .get_mut(usize::from(span.count_scalar()))
                .ok_or(TradingSbfError::Content)? = u64::from(
                *span_counts
                    .get(usize::from(index))
                    .ok_or(TradingSbfError::Content)?,
            );
            index = index.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
    } else if !span_counts.is_empty() {
        return Err(TradingSbfError::Content.into());
    }
    // Five chained projections share three scalar banks and three identity
    // banks, rotated by `swap`, instead of cloning a fresh pair per step. The
    // SBF allocator never frees, so a clone per step charged seven live pairs
    // of total-ever-allocated for a chain that is never more than three deep.
    let mut current_scalars = input_scalars;
    let mut current_identities = input_identities;
    let mut scratch_scalars = vec![0_u64; scalar_count];
    let mut scratch_identities = vec![[0_u8; 32]; identity_count];
    let mut next_scalars = vec![0_u64; scalar_count];
    let mut next_identities = vec![[0_u8; 32]; identity_count];

    let account_registers = ProjectionRegistersV2 {
        input_scalars: &current_scalars,
        input_identities: &current_identities,
        scratch_scalars: &mut scratch_scalars,
        scratch_identities: &mut scratch_identities,
        output_scalars: &mut next_scalars,
        output_identities: &mut next_identities,
    };
    if account_profile.uses_dynamic_fixed_spans() {
        project_dynamic_fixed_spans_atomic(
            account_profile,
            tail_count,
            span_counts,
            observations,
            account_registers,
        )
    } else {
        project_accounts_atomic(account_profile, tail_count, observations, account_registers)
    }
    .map_err(|_| TradingSbfError::Content)?;
    core::mem::swap(&mut current_scalars, &mut next_scalars);
    core::mem::swap(&mut current_identities, &mut next_identities);
    require_projected_tail_count_agreement_v3(
        account_profile,
        authenticated_product_tail_count,
        &current_scalars,
    )?;
    require_trusted_environment_v3(trusted_environment, &current_scalars, &current_identities)?;

    lifecycle
        .project_authenticated_current_rent_quotes_atomic(
            account_profile,
            tail_count,
            &current_scalars,
            current_rent_quotes,
            LifecycleRentQuoteBuffersV5 {
                scalar_scratch: &mut scratch_scalars,
                output_scalars: &mut next_scalars,
            },
        )
        .map_err(|_| TradingSbfError::Content)?;
    core::mem::swap(&mut current_scalars, &mut next_scalars);

    if let RequestProfileKindV3::Signed(profile) = request_profile {
        next_identities.copy_from_slice(&current_identities);
        seed_native_signatures_at_authenticated_instruction(
            current_instruction,
            instruction_data,
            native_message_offset_bias,
            frame.instructions,
            profile,
            tail_count,
            NativeSignatureRegistersV1 {
                input_identities: &current_identities,
                scratch_identities: &mut scratch_identities,
                output_identities: &mut next_identities,
            },
        )?;
        core::mem::swap(&mut current_identities, &mut next_identities);
    }

    request_profile.project_atomic(
        tail_count,
        family_request,
        ProjectionRegistersV1 {
            input_scalars: &current_scalars,
            input_identities: &current_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut next_scalars,
            output_identities: &mut next_identities,
        },
    )?;
    core::mem::swap(&mut current_scalars, &mut next_scalars);
    core::mem::swap(&mut current_identities, &mut next_identities);
    if account_profile.uses_dynamic_fixed_spans() {
        let mut revalidated = vec![0_u32; span_counts.len()];
        account_profile
            .dynamic_span_widths_from_scalars(&current_scalars, &mut revalidated)
            .map_err(|_| TradingSbfError::Content)?;
        if revalidated != span_counts {
            return Err(TradingSbfError::Content.into());
        }
    }
    require_trusted_environment_v3(trusted_environment, &current_scalars, &current_identities)?;
    lifecycle
        .validate_projected_current_rent_quotes(
            account_profile,
            tail_count,
            &current_scalars,
            current_rent_quotes,
        )
        .map_err(|_| TradingSbfError::Content)?;
    Ok(ProjectedRequestRegistersV3 {
        scalars: current_scalars,
        identities: current_identities,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TrustedEnvironmentObservationV3 {
    current_slot: Option<(usize, u64)>,
    current_executing_program: Option<(usize, [u8; 32])>,
    system_program: Option<(usize, [u8; 32])>,
}

fn observe_trusted_environment_v3(
    profile: AccountProfileV2<'_>,
    program_id: &Pubkey,
) -> Result<TrustedEnvironmentObservationV3, ProgramError> {
    let current_slot = match profile.trusted_environment() {
        TrustedEnvironmentV2::None => None,
        TrustedEnvironmentV2::CurrentSlot { destination } => {
            let current_slot = Clock::get().map_err(|_| TradingSbfError::Content)?.slot;
            Some((usize::from(destination), current_slot))
        }
    };
    Ok(TrustedEnvironmentObservationV3 {
        current_slot,
        current_executing_program: profile
            .trusted_current_executing_program_identity()
            .map(|destination| (usize::from(destination), program_id.to_bytes())),
        system_program: profile
            .trusted_system_program_identity()
            .map(|destination| (usize::from(destination), system_program::ID.to_bytes())),
    })
}

fn seed_trusted_environment_v3(
    observation: TrustedEnvironmentObservationV3,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<(), ProgramError> {
    if let Some((destination, current_slot)) = observation.current_slot {
        *scalars
            .get_mut(destination)
            .ok_or(TradingSbfError::Content)? = current_slot;
    }
    if let Some((destination, current_program)) = observation.current_executing_program {
        *identities
            .get_mut(destination)
            .ok_or(TradingSbfError::Content)? = current_program;
    }
    if let Some((destination, system_program)) = observation.system_program {
        *identities
            .get_mut(destination)
            .ok_or(TradingSbfError::Content)? = system_program;
    }
    Ok(())
}

fn require_trusted_environment_v3(
    observation: TrustedEnvironmentObservationV3,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<(), ProgramError> {
    if observation
        .current_slot
        .is_some_and(|(destination, current_slot)| scalars.get(destination) != Some(&current_slot))
        || observation
            .current_executing_program
            .is_some_and(|(destination, current_program)| {
                identities.get(destination) != Some(&current_program)
            })
        || observation
            .system_program
            .is_some_and(|(destination, system_program)| {
                identities.get(destination) != Some(&system_program)
            })
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

#[inline(never)]
fn shadow_routes_v3(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<Vec<ShadowResolvedRouteV3>, ProgramError> {
    let mut output = Vec::new();
    let mut route = 0_u16;
    while route < effect.route_count() {
        let count = effect
            .invocation_count(route, tail_count, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?;
        let mut invocation_index = 0_u32;
        while invocation_index < count {
            let invocation = effect
                .resolved_invocation(route, invocation_index, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Content)?;
            let borrowed_witness = match invocation.borrowed_witness {
                Some(witness) => Some((
                    u32::try_from(witness.source_offset()).map_err(|_| TradingSbfError::Content)?,
                    u32::try_from(witness.len()).map_err(|_| TradingSbfError::Content)?,
                )),
                None => None,
            };
            let mut shadow_dependencies = Vec::new();
            let mut dependency_index = 0_u16;
            while dependency_index < invocation.receipt_dependencies.len() {
                let dependency = effect
                    .resolved_receipt_dependency(invocation.receipt_dependencies, dependency_index)
                    .map_err(|_| TradingSbfError::Content)?;
                shadow_dependencies.push(ShadowReceiptDependencyV3 {
                    producer_role: match dependency.producer_role {
                        FixedRole::Core => ShadowRouteRoleV3::Core,
                        FixedRole::Claims => ShadowRouteRoleV3::Claims,
                        FixedRole::Resolution => ShadowRouteRoleV3::Resolution,
                        FixedRole::Custody => ShadowRouteRoleV3::Custody,
                    },
                    producer_route: dependency.producer_route,
                    producer_invocation: dependency.producer_invocation,
                    expected_receipt_bytes: dependency.expected_receipt_bytes,
                });
                dependency_index = dependency_index
                    .checked_add(1)
                    .ok_or(TradingSbfError::Content)?;
            }
            let receipt_dependency = if shadow_dependencies.len() == 1 {
                shadow_dependencies.first().copied()
            } else {
                None
            };
            let receipt_dependency_count =
                u16::try_from(shadow_dependencies.len()).map_err(|_| TradingSbfError::Content)?;
            let receipt_dependencies_digest = if shadow_dependencies.is_empty() {
                [0; 32]
            } else {
                receipt_dependencies_digest_v4(&shadow_dependencies)
                    .map_err(|_| TradingSbfError::Content)?
            };
            output.push(ShadowResolvedRouteV3 {
                role: match invocation.role {
                    FixedRole::Core => ShadowRouteRoleV3::Core,
                    FixedRole::Claims => ShadowRouteRoleV3::Claims,
                    FixedRole::Resolution => ShadowRouteRoleV3::Resolution,
                    FixedRole::Custody => ShadowRouteRoleV3::Custody,
                },
                kind: match invocation.kind {
                    dclutch_effect_kernel::v3::RouteKindV3::Once => ShadowRouteKindV3::Once,
                    dclutch_effect_kernel::v3::RouteKindV3::AffineOnce => {
                        ShadowRouteKindV3::AffineOnce
                    }
                    dclutch_effect_kernel::v3::RouteKindV3::Each => ShadowRouteKindV3::Each,
                },
                item: invocation.item,
                fixed_account_start: invocation.fixed_account_start,
                fixed_account_count: invocation.fixed_account_count,
                item_account_start: u32::try_from(invocation.item_account_start)
                    .map_err(|_| TradingSbfError::Content)?,
                item_account_count: invocation.item_account_count,
                item_account_stride: invocation.item_account_stride,
                repeated_item_count: invocation.repeated_item_count,
                request_offset: u32::try_from(invocation.request_offset)
                    .map_err(|_| TradingSbfError::Content)?,
                request_len: u32::try_from(invocation.request_len)
                    .map_err(|_| TradingSbfError::Content)?,
                borrowed_witness,
                receipt_dependency,
                receipt_dependency_count,
                receipt_dependencies_digest,
            });
            invocation_index = invocation_index
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?;
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(output)
}

#[inline(never)]
fn preflight_local_effects(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    aliases: &[usize],
) -> Result<(), ProgramError> {
    let mut fixed = 0_u16;
    while fixed < effect.fixed_operation_count() {
        require_root_write_is_state_only(
            effect
                .resolved_fixed_effect(fixed, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Transition)?,
            aliases,
        )?;
        fixed = fixed.checked_add(1).ok_or(TradingSbfError::Transition)?;
    }
    let mut item = 0_u32;
    while item < tail_count {
        let mut operation = 0_u16;
        while operation < effect.item_operation_count() {
            require_root_write_is_state_only(
                effect
                    .resolved_item_effect(item, operation, tail_count, scalars, identities)
                    .map_err(|_| TradingSbfError::Transition)?,
                aliases,
            )?;
            operation = operation
                .checked_add(1)
                .ok_or(TradingSbfError::Transition)?;
        }
        item = item.checked_add(1).ok_or(TradingSbfError::Transition)?;
    }
    Ok(())
}

fn require_root_write_is_state_only(
    resolved: ResolvedEffectV3,
    aliases: &[usize],
) -> Result<(), ProgramError> {
    let (account, offset) = match resolved {
        ResolvedEffectV3::WriteScalar {
            account, offset, ..
        }
        | ResolvedEffectV3::WriteIdentity {
            account, offset, ..
        }
        | ResolvedEffectV3::WriteU8 {
            account, offset, ..
        }
        | ResolvedEffectV3::WriteU16 {
            account, offset, ..
        }
        | ResolvedEffectV3::WriteU32 {
            account, offset, ..
        } => (account, offset),
        _ => return Ok(()),
    };
    let representative = *aliases.get(account).ok_or(TradingSbfError::Transition)?;
    if representative == 0
        && usize::try_from(offset).map_err(|_| TradingSbfError::Transition)?
            < CAPABILITY_ROOT_HEADER_BYTES_V1
    {
        Err(TradingSbfError::Commit.into())
    } else {
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn commit_local_effects(
    effect: SelectedEffectProgramV4<'_>,
    tail_count: u32,
    scalars: &[u64],
    identities: &[[u8; 32]],
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
    output_lamports: &[u64],
    rent: &Rent,
    root_only: bool,
) -> Result<(), ProgramError> {
    for (coordinate, account) in accounts.iter().enumerate() {
        let representative = *aliases.get(coordinate).ok_or(TradingSbfError::Commit)?;
        if representative != coordinate || (coordinate == 0) != root_only {
            continue;
        }
        let output = *output_lamports
            .get(coordinate)
            .ok_or(TradingSbfError::Commit)?;
        if account.lamports() != output {
            **account
                .try_borrow_mut_lamports()
                .map_err(|_| TradingSbfError::Commit)? = output;
        }
    }
    let mut fixed = 0_u16;
    while fixed < effect.fixed_operation_count() {
        commit_data_effect(
            effect
                .resolved_fixed_effect(fixed, tail_count, scalars, identities)
                .map_err(|_| TradingSbfError::Commit)?,
            accounts,
            aliases,
            root_only,
        )?;
        fixed = fixed.checked_add(1).ok_or(TradingSbfError::Commit)?;
    }
    let mut item = 0_u32;
    while item < tail_count {
        let mut operation = 0_u16;
        while operation < effect.item_operation_count() {
            commit_data_effect(
                effect
                    .resolved_item_effect(item, operation, tail_count, scalars, identities)
                    .map_err(|_| TradingSbfError::Commit)?,
                accounts,
                aliases,
                root_only,
            )?;
            operation = operation.checked_add(1).ok_or(TradingSbfError::Commit)?;
        }
        item = item.checked_add(1).ok_or(TradingSbfError::Commit)?;
    }
    for (coordinate, account) in accounts.iter().enumerate() {
        if *aliases.get(coordinate).ok_or(TradingSbfError::Commit)? == coordinate
            && (coordinate == 0) == root_only
            && account.data_len() != 0
            && !rent.is_exempt(account.lamports(), account.data_len())
        {
            return Err(TradingSbfError::Commit.into());
        }
    }
    Ok(())
}

fn commit_data_effect(
    resolved: ResolvedEffectV3,
    accounts: &[&AccountInfo<'_>],
    aliases: &[usize],
    root_only: bool,
) -> Result<(), ProgramError> {
    let (coordinate, offset, bytes): (usize, usize, Vec<u8>) = match resolved {
        ResolvedEffectV3::WriteScalar {
            account,
            offset,
            value,
        } => (
            account,
            usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
            Vec::from(value.to_le_bytes()),
        ),
        ResolvedEffectV3::WriteIdentity {
            account,
            offset,
            value,
        } => (
            account,
            usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
            Vec::from(value),
        ),
        ResolvedEffectV3::WriteU8 {
            account,
            offset,
            value,
        } => (
            account,
            usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
            Vec::from(value.to_le_bytes()),
        ),
        ResolvedEffectV3::WriteU16 {
            account,
            offset,
            value,
        } => (
            account,
            usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
            Vec::from(value.to_le_bytes()),
        ),
        ResolvedEffectV3::WriteU32 {
            account,
            offset,
            value,
        } => (
            account,
            usize::try_from(offset).map_err(|_| TradingSbfError::Commit)?,
            Vec::from(value.to_le_bytes()),
        ),
        _ => return Ok(()),
    };
    let representative = *aliases.get(coordinate).ok_or(TradingSbfError::Commit)?;
    if (representative == 0) != root_only {
        return Ok(());
    }
    let account = accounts
        .get(representative)
        .ok_or(TradingSbfError::Commit)?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    let end = offset
        .checked_add(bytes.len())
        .ok_or(TradingSbfError::Commit)?;
    data.get_mut(offset..end)
        .ok_or(TradingSbfError::Commit)?
        .copy_from_slice(&bytes);
    Ok(())
}

fn authenticate_descriptor_root_selection(
    descriptor: &CapabilityProgramV4,
    context: &TradingFamilyContextV1,
    entry: &dclutch_capability_contract::CapabilityEntryV1,
) -> Result<(), ProgramError> {
    if descriptor
        .validate_selection(context.selection(), *entry)
        .is_err()
        || descriptor
            .root_account_bytes()
            .map_err(|_| TradingSbfError::Root)?
            != context.root_account_bytes()
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

fn authenticate_market(
    frame: HotFrameV3<'_, '_>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<CoreState, ProgramError> {
    if frame.market.owner != frame.core_program.key || frame.market.data_len() != STATE_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let bytes = frame
        .market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let state = CoreState::decode(&bytes).map_err(|_| TradingSbfError::Content)?;
    if state
        .encode()
        .map_err(|_| TradingSbfError::Content)?
        .as_slice()
        != bytes.as_ref()
        || state.identity.market_id.to_bytes() != frame.market.key.to_bytes()
        || state.identity.selected_release_set.to_bytes() != envelope.release_set()
        || state.identity.registry_program.to_bytes() != frame.registry.key.to_bytes()
        || state.identity.generation != envelope.generation()
        || envelope.market() != frame.market.key.to_bytes()
        || Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
            frame.core_program.key,
        )
        .0 != *frame.market.key
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(state)
}

fn reauthenticate_role<'accounts, 'info>(
    frame: HotFrameV3<'accounts, 'info>,
    role: ExecutionRoleV1,
    role_program: &AccountInfo<'info>,
    role_programdata: &AccountInfo<'info>,
    release_set: [u8; 32],
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        frame.registry.key,
    )
    .0;
    if frame.activation_cache.key != &expected_cache
        || frame.activation_cache.owner != frame.registry.key
    {
        return Err(TradingSbfError::Release.into());
    }
    let instruction = Instruction {
        program_id: *frame.registry.key,
        accounts: vec![
            AccountMeta::new_readonly(*frame.activation_cache.key, false),
            AccountMeta::new_readonly(*role_program.key, false),
            AccountMeta::new_readonly(*role_programdata.key, false),
        ],
        data: RegistryInstructionV1::Reauthenticate(role)
            .to_bytes()
            .to_vec(),
    };
    invoke(
        &instruction,
        &[
            frame.activation_cache.clone(),
            role_program.clone(),
            role_programdata.clone(),
            frame.registry.clone(),
        ],
    )
    .map_err(|_| TradingSbfError::Release)?;
    let (producer, bytes) = get_return_data().ok_or(TradingSbfError::Release)?;
    let receipt =
        AuthenticatedRoleReceiptV1::decode(&bytes).map_err(|_| TradingSbfError::Release)?;
    if producer != *frame.registry.key
        || receipt.role() != role
        || receipt.execution_release_set_id().to_bytes() != release_set
        || receipt.program().as_bytes() != &role_program.key.to_bytes()
    {
        return Err(TradingSbfError::Release.into());
    }
    Ok(receipt)
}

fn borrow_finalized_record<'a, 'info>(
    frame: HotFrameV3<'_, 'info>,
    raw: &'a AccountInfo<'info>,
    staging: &AccountInfo<'info>,
    rent: &Rent,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<core::cell::Ref<'a, [u8]>, ProgramError> {
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        frame.registry.key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        frame.registry.key,
    )
    .0;
    if raw.key != &expected_raw
        || raw.owner != frame.registry.key
        || raw.is_signer
        || raw.is_writable
        || raw.executable
        || hash(&data).to_bytes() != digest
        || !rent.is_exempt(raw.lamports(), data.len())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.is_signer
        || staging.is_writable
        || staging.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(core::cell::Ref::map(data, |bytes| &**bytes))
}

#[derive(Clone, Copy)]
struct HotFrameV3<'accounts, 'info> {
    market: &'accounts AccountInfo<'info>,
    root: &'accounts AccountInfo<'info>,
    manifest_raw: &'accounts AccountInfo<'info>,
    manifest_staging: &'accounts AccountInfo<'info>,
    program_set_raw: &'accounts AccountInfo<'info>,
    program_set_staging: &'accounts AccountInfo<'info>,
    descriptor_raw: &'accounts AccountInfo<'info>,
    descriptor_staging: &'accounts AccountInfo<'info>,
    config_raw: &'accounts AccountInfo<'info>,
    config_staging: &'accounts AccountInfo<'info>,
    account_profile_raw: &'accounts AccountInfo<'info>,
    account_profile_staging: &'accounts AccountInfo<'info>,
    request_profile_raw: &'accounts AccountInfo<'info>,
    request_profile_staging: &'accounts AccountInfo<'info>,
    transition_raw: &'accounts AccountInfo<'info>,
    transition_staging: &'accounts AccountInfo<'info>,
    effect_raw: &'accounts AccountInfo<'info>,
    effect_staging: &'accounts AccountInfo<'info>,
    lifecycle_raw: &'accounts AccountInfo<'info>,
    lifecycle_staging: &'accounts AccountInfo<'info>,
    strategy_raw: &'accounts AccountInfo<'info>,
    strategy_staging: &'accounts AccountInfo<'info>,
    activation_cache: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    instructions: &'accounts AccountInfo<'info>,
    product_raw: &'accounts AccountInfo<'info>,
    product_staging: &'accounts AccountInfo<'info>,
    result_domain_raw: &'accounts AccountInfo<'info>,
    result_domain_staging: &'accounts AccountInfo<'info>,
    portfolio_raw: &'accounts AccountInfo<'info>,
    portfolio_staging: &'accounts AccountInfo<'info>,
    linked_basis_raw: &'accounts AccountInfo<'info>,
    linked_basis_staging: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> HotFrameV3<'accounts, 'info> {
    fn from_accounts(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        if accounts.len() < HOT_FIXED_ACCOUNT_COUNT_V3 {
            return Err(TradingSbfError::Content.into());
        }
        Ok(Self {
            market: account(accounts, HOT_MARKET_ACCOUNT_V3)?,
            root: account(accounts, HOT_ROOT_ACCOUNT_V3)?,
            manifest_raw: account(accounts, HOT_MANIFEST_RAW_ACCOUNT_V3)?,
            manifest_staging: account(accounts, HOT_MANIFEST_STAGING_ACCOUNT_V3)?,
            program_set_raw: account(accounts, HOT_PROGRAM_SET_RAW_ACCOUNT_V3)?,
            program_set_staging: account(accounts, HOT_PROGRAM_SET_STAGING_ACCOUNT_V3)?,
            descriptor_raw: account(accounts, HOT_DESCRIPTOR_RAW_ACCOUNT_V3)?,
            descriptor_staging: account(accounts, HOT_DESCRIPTOR_STAGING_ACCOUNT_V3)?,
            config_raw: account(accounts, HOT_CONFIG_RAW_ACCOUNT_V3)?,
            config_staging: account(accounts, HOT_CONFIG_STAGING_ACCOUNT_V3)?,
            account_profile_raw: account(accounts, HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3)?,
            account_profile_staging: account(accounts, HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3)?,
            request_profile_raw: account(accounts, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3)?,
            request_profile_staging: account(accounts, HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3)?,
            transition_raw: account(accounts, HOT_TRANSITION_RAW_ACCOUNT_V3)?,
            transition_staging: account(accounts, HOT_TRANSITION_STAGING_ACCOUNT_V3)?,
            effect_raw: account(accounts, HOT_EFFECT_RAW_ACCOUNT_V3)?,
            effect_staging: account(accounts, HOT_EFFECT_STAGING_ACCOUNT_V3)?,
            lifecycle_raw: account(accounts, HOT_LIFECYCLE_RAW_ACCOUNT_V3)?,
            lifecycle_staging: account(accounts, HOT_LIFECYCLE_STAGING_ACCOUNT_V3)?,
            strategy_raw: account(accounts, HOT_STRATEGY_RAW_ACCOUNT_V3)?,
            strategy_staging: account(accounts, HOT_STRATEGY_STAGING_ACCOUNT_V3)?,
            activation_cache: account(accounts, HOT_ACTIVATION_CACHE_ACCOUNT_V3)?,
            core_program: account(accounts, HOT_CORE_PROGRAM_ACCOUNT_V3)?,
            core_programdata: account(accounts, HOT_CORE_PROGRAMDATA_ACCOUNT_V3)?,
            trading_program: account(accounts, HOT_TRADING_PROGRAM_ACCOUNT_V3)?,
            trading_programdata: account(accounts, HOT_TRADING_PROGRAMDATA_ACCOUNT_V3)?,
            registry: account(accounts, HOT_REGISTRY_PROGRAM_ACCOUNT_V3)?,
            rent: account(accounts, HOT_RENT_SYSVAR_ACCOUNT_V3)?,
            instructions: account(accounts, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)?,
            product_raw: account(accounts, HOT_PRODUCT_RAW_ACCOUNT_V3)?,
            product_staging: account(accounts, HOT_PRODUCT_STAGING_ACCOUNT_V3)?,
            result_domain_raw: account(accounts, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3)?,
            result_domain_staging: account(accounts, HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3)?,
            portfolio_raw: account(accounts, HOT_PORTFOLIO_RAW_ACCOUNT_V3)?,
            portfolio_staging: account(accounts, HOT_PORTFOLIO_STAGING_ACCOUNT_V3)?,
            linked_basis_raw: account(accounts, HOT_LINKED_BASIS_RAW_ACCOUNT_V3)?,
            linked_basis_staging: account(accounts, HOT_LINKED_BASIS_STAGING_ACCOUNT_V3)?,
        })
    }

    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
        permits_fixed_market_union: bool,
    ) -> Result<Self, ProgramError> {
        let value = Self::from_accounts(accounts)?;
        if value.market.is_signer
            || (value.market.is_writable && !permits_fixed_market_union)
            || value.market.executable
            || value.root.is_signer
            || !value.root.is_writable
            || value.root.executable
            || value.trading_program.key != program_id
            || !value.trading_program.executable
            || value.trading_program.is_signer
            || value.trading_program.is_writable
            || !value.core_program.executable
            || value.core_program.is_signer
            || value.core_program.is_writable
            || !value.registry.executable
            || value.registry.is_signer
            || value.registry.is_writable
            || value.rent.key != &sysvar::rent::ID
            || value.rent.is_signer
            || value.rent.is_writable
            || value.rent.executable
        {
            return Err(TradingSbfError::Content.into());
        }
        for (left, account) in accounts
            .get(..HOT_FIXED_ACCOUNT_COUNT_V3)
            .ok_or(TradingSbfError::Content)?
            .iter()
            .enumerate()
        {
            if accounts
                .get(left.saturating_add(1)..HOT_FIXED_ACCOUNT_COUNT_V3)
                .ok_or(TradingSbfError::Content)?
                .iter()
                .any(|other| other.key == account.key)
            {
                return Err(TradingSbfError::Content.into());
            }
        }
        Ok(value)
    }

    fn parse_accelerator_readonly(
        trading_program: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        if accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
            || accounts
                .iter()
                .any(|account| account.is_signer || account.is_writable)
        {
            return Err(TradingSbfError::Content.into());
        }
        let value = Self::from_accounts(accounts)?;
        if value.market.executable
            || value.root.executable
            || value.trading_program.key != trading_program
            || !value.trading_program.executable
            || !value.core_program.executable
            || !value.registry.executable
            || value.rent.key != &sysvar::rent::ID
            || value.rent.executable
            || value.instructions.key != &sysvar::instructions::ID
            || value.instructions.executable
        {
            return Err(TradingSbfError::Content.into());
        }
        for (left, account) in accounts.iter().enumerate() {
            if accounts
                .get(left.saturating_add(1)..)
                .ok_or(TradingSbfError::Content)?
                .iter()
                .any(|other| other.key == account.key)
            {
                return Err(TradingSbfError::Content.into());
            }
        }
        Ok(value)
    }
}

fn account<'a, 'info>(
    accounts: &'a [AccountInfo<'info>],
    index: usize,
) -> Result<&'a AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}

/// Return whether instruction data selects the common V3 hot outer.
pub fn is_hot_execution_v3(instruction_data: &[u8]) -> bool {
    instruction_data.get(..8) == Some(HOT_EXECUTION_MAGIC_V3.as_slice())
}

#[cfg(test)]
mod tests {
    use alloc::boxed::Box;

    use super::*;

    use dclutch_account_profile_contract::lifecycle_v3::CreateStatePlanV3;
    use dclutch_account_profile_contract::v2::{
        AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES, AccountPrestateV2,
        DYNAMIC_FIXED_SPAN_HEADER_BYTES,
        HEADER_BYTES as ACCOUNT_PROFILE_HEADER_BYTES,
        OPERATION_BYTES as ACCOUNT_PROFILE_OPERATION_BYTES,
        RULE_BYTES as ACCOUNT_PROFILE_RULE_BYTES, TrustedBuiltinIdentityV2, TrustedEnvironmentV2,
        TrustedIdentityEnvironmentV2,
        encode::{
            AccountAliasInputV2, AccountCoordinateV2, AccountEffectPermissionsV2,
            AccountOperationInputV2, AccountPrivilegesV2, AccountProfileArtifactV2,
            AccountRuleInputV2, AccountRuleWithPrestateInputV2, DynamicFixedSpanInputV2,
            RegisterGeometryV2, ScalarCoordinateV2, encode_account_profile_v2_atomic,
            encode_account_profile_with_authenticated_route_alias_v2_atomic,
            encode_account_profile_with_dynamic_fixed_span_v2_atomic,
        },
    };
    use dclutch_transition_vm::v3::{
        HEADER_BYTES as TRANSITION_HEADER_BYTES_V3,
        INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES_V3, InstructionV3, ProgramGeometryV3,
        ScalarRegisterV3, encode_program_atomic,
    };

    #[test]
    fn lifecycle_rent_credit_v2_binds_market_release_and_generation() {
        use dclutch_rent_contract::{
            RefundAuthority,
            lifecycle_v2::{
                LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
            },
        };

        let rent_program = Pubkey::new_unique();
        let refund = Pubkey::new_unique();
        let market = Pubkey::new_unique();
        let release = Pubkey::new_unique();
        let generation = 9_u64;
        let generation_seed = generation.to_le_bytes();
        let (credit_key, bump) = Pubkey::find_program_address(
            &[
                LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
                market.as_ref(),
                &generation_seed,
            ],
            &rent_program,
        );
        let state = LifecycleRentCreditV2::new(
            RefundAuthority::new(refund.to_bytes()).expect("refund"),
            LifecycleAccountIdV2::new(market.to_bytes()).expect("market"),
            LifecycleAccountIdV2::new(release.to_bytes()).expect("release"),
            generation,
            bump,
        )
        .expect("state");
        let rent = Rent::default();
        let floor = rent.minimum_balance(LIFECYCLE_RENT_CREDIT_BYTES_V2);
        let credit = AccountInfo::new(
            Box::leak(Box::new(credit_key)),
            false,
            true,
            Box::leak(Box::new(floor)),
            Box::leak(state.to_bytes().to_vec().into_boxed_slice()),
            Box::leak(Box::new(rent_program)),
            false,
        );
        let owner = AccountInfo::new(
            Box::leak(Box::new(rent_program)),
            false,
            false,
            Box::leak(Box::new(1_u64)),
            Box::leak(Vec::new().into_boxed_slice()),
            Box::leak(Box::new(Pubkey::new_unique())),
            true,
        );
        let accounts = [&credit, &owner];
        let authenticated = authenticate_lifecycle_credit_v3(
            &accounts,
            0,
            floor,
            &rent,
            market.to_bytes(),
            release.to_bytes(),
            generation,
        )
        .expect("exact lifecycle credit");
        assert_eq!(authenticated.beneficiary, refund.to_bytes());
        assert!(
            authenticate_lifecycle_credit_v3(
                &accounts,
                0,
                floor,
                &rent,
                Pubkey::new_unique().to_bytes(),
                release.to_bytes(),
                generation,
            )
            .is_err()
        );
        assert!(
            authenticate_lifecycle_credit_v3(
                &accounts,
                0,
                floor,
                &rent,
                market.to_bytes(),
                Pubkey::new_unique().to_bytes(),
                generation,
            )
            .is_err()
        );
        assert!(
            authenticate_lifecycle_credit_v3(
                &accounts,
                0,
                floor,
                &rent,
                market.to_bytes(),
                release.to_bytes(),
                generation + 1,
            )
            .is_err()
        );

        let stale_v1 = AccountInfo::new(
            Box::leak(Box::new(Pubkey::new_unique())),
            false,
            true,
            Box::leak(Box::new(rent.minimum_balance(48))),
            Box::leak(vec![0_u8; 48].into_boxed_slice()),
            Box::leak(Box::new(rent_program)),
            false,
        );
        let stale_accounts = [&stale_v1, &owner];
        assert!(
            authenticate_lifecycle_credit_v3(
                &stale_accounts,
                0,
                stale_v1.lamports(),
                &rent,
                market.to_bytes(),
                release.to_bytes(),
                generation,
            )
            .is_err()
        );
    }

    #[test]
    fn effect_v4_schema_and_zero_extension_envelope_are_exact() {
        use dclutch_effect_kernel::{
            v3::{
                HEADER_BYTES,
                encode::{EffectGeometryV3, encode_effect_program_v3_atomic},
            },
            v4::{BorrowedRangePolicyV4, HEADER_BYTES_V4, encode_program_v4_atomic},
        };
        let mut base_scratch = [0_u8; HEADER_BYTES];
        let mut base = [0_u8; HEADER_BYTES];
        encode_effect_program_v3_atomic(
            EffectGeometryV3 {
                fixed_accounts: 1,
                item_account_stride: 0,
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &[],
            &[],
            &[],
            &mut base_scratch,
            &mut base,
        )
        .expect("fixed base");
        let mut scratch = vec![0_u8; HEADER_BYTES_V4 + HEADER_BYTES];
        let mut output = vec![0_u8; HEADER_BYTES_V4 + HEADER_BYTES];
        encode_program_v4_atomic(
            &base,
            BorrowedRangePolicyV4::DisjointExactCoverage,
            1,
            &[],
            &[],
            &mut scratch,
            &mut output,
        )
        .expect("zero-extension successor");
        let selected =
            decode_selected_effect_v4(EFFECT_SCHEMA_ID_V4, &output).expect("selected V4 effect");
        assert_eq!(selected.successor.span_count(), 0);
        assert_eq!(selected.successor.range_count(), 0);
        assert!(decode_selected_effect_v4([7; 32], &output).is_err());

        let mut hostile = output;
        hostile[0] ^= 1;
        assert!(decode_selected_effect_v4(EFFECT_SCHEMA_ID_V4, &hostile).is_err());
    }

    #[test]
    fn authenticated_accelerator_inline_bank_is_exact_and_untruncated() {
        let mut bank = Vec::new();
        bank.extend_from_slice(&11_u64.to_le_bytes());
        bank.extend_from_slice(&u64::MAX.to_le_bytes());
        bank.extend_from_slice(&[0x5a; 32]);
        let content = |byte| ContentId::new([byte; 32]).expect("nonzero content");
        let request = AcceleratorRequestV2::new(
            RequestTransportV2::Inline,
            content(1),
            content(2),
            content(3),
            content(4),
            ContentId::new(hash(&bank).to_bytes()).expect("bank digest"),
            7,
            2,
            1,
            0,
            &bank,
        )
        .expect("inline request");
        assert_eq!(
            authenticate_accelerator_input_bank_v4(request, &[], &Pubkey::new_unique())
                .expect("authenticated inline bank"),
            bank
        );
        let (scalars, identities) =
            decode_accelerator_register_bank_v4(request, &bank).expect("register decode");
        assert_eq!(scalars, [11, u64::MAX]);
        assert_eq!(identities, [[0x5a; 32]]);

        let wrong_digest = AcceleratorRequestV2::new(
            RequestTransportV2::Inline,
            content(1),
            content(2),
            content(3),
            content(4),
            content(9),
            7,
            2,
            1,
            0,
            &bank,
        )
        .expect("hostile request shape");
        assert!(
            authenticate_accelerator_input_bank_v4(wrong_digest, &[], &Pubkey::new_unique())
                .is_err()
        );
        assert!(decode_accelerator_register_bank_v4(request, &bank[..40]).is_err());
    }

    #[test]
    fn selector_is_exact_and_does_not_shadow_activation() {
        assert!(is_hot_execution_v3(b"DCLTHOT3"));
        assert!(!is_hot_execution_v3(b"DCLTHOT2"));
        assert!(!is_hot_execution_v3(b"DCLTHOT"));
    }

    #[test]
    fn registry_continuation_authenticates_admission_and_market_union() {
        use dclutch_registry_svm::continuation_v1::RegistryContinuationAdmissionSeedsV1;
        use solana_instructions_sysvar::construct_instructions_data;
        use solana_program::sysvar::instructions::{BorrowedAccountMeta, BorrowedInstruction};

        fn info(
            key: Pubkey,
            signer: bool,
            writable: bool,
            owner: Pubkey,
            executable: bool,
            data: Vec<u8>,
        ) -> AccountInfo<'static> {
            AccountInfo::new(
                Box::leak(Box::new(key)),
                signer,
                writable,
                Box::leak(Box::new(0_u64)),
                Box::leak(data.into_boxed_slice()),
                Box::leak(Box::new(owner)),
                executable,
            )
        }

        let program_id = Pubkey::new_unique();
        let registry = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let mut keys = (0..=HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|_| Pubkey::new_unique())
            .collect::<Vec<_>>();
        keys[HOT_TRADING_PROGRAM_ACCOUNT_V3] = program_id;
        keys[HOT_REGISTRY_PROGRAM_ACCOUNT_V3] = registry;
        keys[HOT_RENT_SYSVAR_ACCOUNT_V3] = sysvar::rent::ID;
        keys[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3] = sysvar::instructions::ID;
        let activation_bytes = vec![0xa7; 64];
        let release = ContentId::new([0x31; 32]).expect("release");
        let envelope = HotExecutionEnvelopeV3::new(
            1,
            release.to_bytes(),
            keys[HOT_MARKET_ACCOUNT_V3].to_bytes(),
            7,
            [0x32; 32],
        )
        .expect("envelope");
        let mut hot_bytes = envelope.to_bytes().to_vec();
        hot_bytes.push(9);
        let activation_digest =
            ContentId::new(hash(&activation_bytes).to_bytes()).expect("activation digest");
        let hot_digest = ContentId::new(hash(&hot_bytes).to_bytes()).expect("Hot digest");
        let continuation = RegistryContinuationRequestV1::new_core_trading_hot(
            release,
            activation_digest,
            hot_digest,
            u32::try_from(hot_bytes.len()).expect("Hot width"),
        )
        .expect("continuation");
        let batch = continuation.role_batch_request().expect("batch");
        let batch_digest =
            ContentId::new(hash(&batch.to_bytes()).to_bytes()).expect("batch digest");
        let seeds = RegistryContinuationAdmissionSeedsV1::new(
            continuation,
            keys[HOT_ACTIVATION_CACHE_ACCOUNT_V3].to_bytes(),
            batch_digest,
        )
        .expect("admission seeds");
        let release_seed = seeds.release_set();
        let cache_seed = seeds.activation_cache();
        let batch_seed = seeds.batch_request_digest();
        let mask_seed = seeds.role_mask();
        let role_seed = seeds.continuation_role();
        let digest_seed = seeds.continuation_digest();
        keys[HOT_FIXED_ACCOUNT_COUNT_V3] = Pubkey::find_program_address(
            &[
                seeds.domain(),
                release_seed.as_slice(),
                cache_seed.as_slice(),
                batch_seed.as_slice(),
                mask_seed.as_slice(),
                role_seed.as_slice(),
                digest_seed.as_slice(),
            ],
            &registry,
        )
        .0;

        let top_data = hot_bytes.clone();
        let outer_keys = [
            keys[HOT_ACTIVATION_CACHE_ACCOUNT_V3],
            keys[HOT_CORE_PROGRAM_ACCOUNT_V3],
            keys[HOT_CORE_PROGRAMDATA_ACCOUNT_V3],
            keys[HOT_TRADING_PROGRAM_ACCOUNT_V3],
            keys[HOT_TRADING_PROGRAMDATA_ACCOUNT_V3],
            keys[HOT_FIXED_ACCOUNT_COUNT_V3],
        ];
        let build_metas = || {
            let mut metas = outer_keys
                .iter()
                .map(|key| BorrowedAccountMeta {
                    pubkey: key,
                    is_signer: false,
                    is_writable: false,
                })
                .collect::<Vec<_>>();
            metas.extend(
                keys.iter()
                    .enumerate()
                    .map(|(index, key)| BorrowedAccountMeta {
                        pubkey: key,
                        is_signer: false,
                        is_writable: index == HOT_MARKET_ACCOUNT_V3 || index == HOT_ROOT_ACCOUNT_V3,
                    }),
            );
            metas
        };
        let borrowed = [BorrowedInstruction {
            program_id: &registry,
            accounts: build_metas(),
            data: &top_data,
        }];
        let mut instructions_data = construct_instructions_data(&borrowed);
        let instructions_end = instructions_data.len();
        instructions_data
            .get_mut(instructions_end - 2..)
            .expect("current instruction")
            .copy_from_slice(&0_u16.to_le_bytes());

        let mut accounts = keys
            .iter()
            .enumerate()
            .map(|(index, key)| {
                let executable = matches!(
                    index,
                    HOT_CORE_PROGRAM_ACCOUNT_V3
                        | HOT_TRADING_PROGRAM_ACCOUNT_V3
                        | HOT_REGISTRY_PROGRAM_ACCOUNT_V3
                );
                let signer = index == HOT_FIXED_ACCOUNT_COUNT_V3;
                let writable = index == HOT_MARKET_ACCOUNT_V3 || index == HOT_ROOT_ACCOUNT_V3;
                let account_owner = if index == HOT_FIXED_ACCOUNT_COUNT_V3 {
                    system_program::ID
                } else {
                    owner
                };
                let data = if index == HOT_ACTIVATION_CACHE_ACCOUNT_V3 {
                    activation_bytes.clone()
                } else if index == HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3 {
                    instructions_data.clone()
                } else {
                    Vec::new()
                };
                info(*key, signer, writable, account_owner, executable, data)
            })
            .collect::<Vec<_>>();

        let authenticated =
            authenticate_hot_invocation_v3(&program_id, &accounts, &hot_bytes, envelope)
                .expect("Registry continuation");
        assert_eq!(
            authenticated.strategy_extras_start,
            HOT_FIXED_ACCOUNT_COUNT_V3 + 1
        );
        assert_eq!(authenticated.native_message_offset_bias, 0);
        assert!(authenticated.permits_fixed_market_union);
        assert_eq!(
            authenticated.role_authentication,
            HotRoleAuthenticationV3::AuthenticatedContinuation
        );
        assert!(HotFrameV3::parse(&program_id, &accounts, false).is_err());
        assert!(HotFrameV3::parse(&program_id, &accounts, true).is_ok());

        let mut substituted_data = top_data.clone();
        *substituted_data.last_mut().expect("family byte") ^= 1;
        let substituted = [BorrowedInstruction {
            program_id: &registry,
            accounts: build_metas(),
            data: &substituted_data,
        }];
        let mut substituted_instructions = construct_instructions_data(&substituted);
        let substituted_end = substituted_instructions.len();
        substituted_instructions
            .get_mut(substituted_end - 2..)
            .expect("substituted current instruction")
            .copy_from_slice(&0_u16.to_le_bytes());
        accounts[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3]
            .try_borrow_mut_data()
            .expect("instructions data")
            .copy_from_slice(&substituted_instructions);
        assert!(
            authenticate_hot_invocation_v3(&program_id, &accounts, &hot_bytes, envelope).is_err()
        );
        accounts[HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3]
            .try_borrow_mut_data()
            .expect("instructions restore")
            .copy_from_slice(&instructions_data);

        accounts[HOT_FIXED_ACCOUNT_COUNT_V3].is_signer = false;
        assert!(
            authenticate_hot_invocation_v3(&program_id, &accounts, &hot_bytes, envelope).is_err()
        );
    }

    #[test]
    fn lifecycle_v5_quotes_are_derived_only_from_current_rent() {
        use dclutch_account_profile_contract::lifecycle_v3::{
            ACTION_PLAN_BYTES, CURRENT_RENT_QUOTE_BYTES_V5, HEADER_BYTES, PROTECTED_OUTPUT_BYTES,
            RECIPE_BYTES, SEED_BYTES,
            encode::{
                LifecycleAccountCoordinateV3, LifecycleCurrentRentQuoteInputV5,
                LifecycleGuardInputV3, LifecycleOperationInputV3, LifecyclePlanInputV3,
                LifecycleRecipeInputV3, LifecycleSeedInputV3, encode_lifecycle_policy_v5_atomic,
            },
        };

        const WIDTH: usize = HEADER_BYTES
            + RECIPE_BYTES
            + 2 * SEED_BYTES
            + ACTION_PLAN_BYTES
            + PROTECTED_OUTPUT_BYTES
            + CURRENT_RENT_QUOTE_BYTES_V5;
        let recipes = [LifecycleRecipeInputV3 {
            state: LifecycleAccountCoordinateV3::fixed(0),
            seed_start: 0,
            seed_count: 2,
            bump_offset: 1,
            data_base: 8,
            data_stride: 0,
        }];
        let seeds = [
            LifecycleSeedInputV3::Literal(b"hot-rent-quote-v5"),
            LifecycleSeedInputV3::CanonicalBump,
        ];
        let plans = [LifecyclePlanInputV3 {
            action: 1,
            operation: LifecycleOperationInputV3::Authenticate,
            recipe: 0,
            payer: None,
            rent_credit: None,
            principal: None,
            beneficiary: None,
            guard: LifecycleGuardInputV3::Always,
        }];
        let mut scratch = [0_u8; WIDTH];
        let mut bytes = [0_u8; WIDTH];
        encode_lifecycle_policy_v5_atomic(
            &recipes,
            &seeds,
            &plans,
            &[None],
            &[],
            &[LifecycleCurrentRentQuoteInputV5 {
                exact_data_len: 152,
                scalar_destination: 64,
            }],
            &mut scratch,
            &mut bytes,
        )
        .expect("lifecycle V5 with current-Rent declaration");
        let policy = StateLifecyclePolicyV5::decode_selected([1; 32], [1; 32], &bytes)
            .expect("selected lifecycle V5");
        let rent = Rent::default();
        let quotes = authenticate_current_rent_quotes_v5(policy, &rent)
            .expect("authenticated current Rent quote");
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].exact_data_len, 152);
        assert_eq!(quotes[0].scalar_destination, 64);
        assert_eq!(quotes[0].current_minimum, rent.minimum_balance(152));
    }

    #[test]
    fn profile13_zero_spans_expand_aliases_and_downgrade_child_privileges() {
        const READONLY: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, false, false);
        const WRITABLE: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, true, false);
        const NO_EFFECTS: AccountEffectPermissionsV2 =
            AccountEffectPermissionsV2::new(false, false, false);

        let exact = |privileges| AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        let alias = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::Fixed(4),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        };
        let rules = [
            exact(READONLY),
            exact(READONLY),
            exact(READONLY),
            exact(READONLY),
            exact(WRITABLE),
            exact(READONLY),
            alias,
        ];
        let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
            .checked_add(
                rules
                    .len()
                    .checked_mul(ACCOUNT_PROFILE_RULE_BYTES)
                    .expect("rules"),
            )
            .expect("width");
        let mut scratch = vec![0_u8; width];
        let mut bytes = vec![0_u8; width];
        encode_account_profile_with_dynamic_fixed_span_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::SystemProgram { destination: 0 },
            &[],
            &rules,
            &[],
            &[],
            RegisterGeometryV2 {
                common_scalars: 0,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut bytes,
        )
        .expect("profile13 zero spans");
        let profile = AccountProfileV2::decode(&bytes).expect("decode profile13");
        assert_eq!(
            profile.artifact_profile(),
            dclutch_account_profile_contract::v2::DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        );
        assert_eq!(profile.dynamic_fixed_span_count(), 0);

        let make_account = |writable| {
            let key = Box::leak(Box::new(Pubkey::new_unique()));
            let owner = Box::leak(Box::new(Pubkey::new_unique()));
            let lamports = Box::leak(Box::new(0_u64));
            let data = Box::leak(Vec::new().into_boxed_slice());
            AccountInfo::new(key, false, writable, lamports, data, owner, false)
        };
        let physical = [
            make_account(false),
            make_account(false),
            make_account(false),
            make_account(false),
            make_account(true),
            make_account(false),
        ];
        let logical = expand_runtime_accounts_v3(
            profile,
            0,
            &[],
            [
                &physical[0],
                &physical[1],
                &physical[2],
                &physical[3],
                &physical[4],
            ],
            &physical[5..],
        )
        .expect("expand physical representatives");
        assert_eq!(logical.len(), 7);
        assert_eq!(logical[4].key, logical[6].key);

        let child = downgraded_effect_accounts_v3(profile, 0, &[], &logical)
            .expect("downgrade route views");
        // Coordinate 6 is an authenticated route alias of the writable
        // representative 4. An alias is emitted privilege-free, so it states
        // nothing at all, and a child CPI meta built from the alias would hand
        // the child program a readonly view of an account its own authenticated
        // declaration states as writable.
        assert!(child[4].is_writable);
        assert!(child[6].is_writable);
        assert_eq!(child[4].key, child[6].key);

        // A declaration never becomes a writable meta for an account the
        // transaction did not include as writable.
        let withheld = [
            make_account(false),
            make_account(false),
            make_account(false),
            make_account(false),
            make_account(false),
            make_account(false),
        ];
        let withheld_logical = expand_runtime_accounts_v3(
            profile,
            0,
            &[],
            [
                &withheld[0],
                &withheld[1],
                &withheld[2],
                &withheld[3],
                &withheld[4],
            ],
            &withheld[5..],
        )
        .expect("expand physical representatives");
        assert!(downgraded_effect_accounts_v3(profile, 0, &[], &withheld_logical).is_err());
        assert!(
            expand_runtime_accounts_v3(
                profile,
                0,
                &[],
                [
                    &physical[0],
                    &physical[1],
                    &physical[2],
                    &physical[3],
                    &physical[4],
                ],
                &physical[4..],
            )
            .is_err()
        );
    }

    #[test]
    fn fixed_route_alias_of_an_executable_representative_survives_the_child_downgrade() {
        const READONLY: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, false, false);
        const EXECUTABLE: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, false, true);
        const NO_EFFECTS: AccountEffectPermissionsV2 =
            AccountEffectPermissionsV2::new(false, false, false);

        let exact = |privileges| AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::Exact,
        };
        // Post-`cc228cd` an authenticated route alias is privilege-free: the
        // representative owns the physical executable fact.
        let alias = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::Fixed(1),
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedRouteAlias,
        };
        let rules = [exact(READONLY), exact(EXECUTABLE), alias];
        let width = AUTHENTICATED_ROUTE_ALIAS_HEADER_BYTES
            .checked_add(
                rules
                    .len()
                    .checked_mul(ACCOUNT_PROFILE_RULE_BYTES)
                    .expect("rules"),
            )
            .expect("width");
        let mut scratch = vec![0_u8; width];
        let mut bytes = vec![0_u8; width];
        encode_account_profile_with_authenticated_route_alias_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::SystemProgram { destination: 0 },
            &rules,
            &[],
            &[],
            &[],
            RegisterGeometryV2 {
                common_scalars: 0,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut bytes,
        )
        .expect("authenticated route alias profile");
        let profile = AccountProfileV2::decode(&bytes).expect("decode route alias profile");
        assert!(!profile.uses_dynamic_fixed_spans());

        let account = |executable| {
            let key = Box::leak(Box::new(Pubkey::new_unique()));
            let owner = Box::leak(Box::new(Pubkey::new_unique()));
            let lamports = Box::leak(Box::new(0_u64));
            let data = Box::leak(Vec::new().into_boxed_slice());
            AccountInfo::new(key, false, false, lamports, data, owner, executable)
        };
        let plain = account(false);
        let program = account(true);
        let logical = [&plain, &program, &program];

        let child = downgraded_effect_accounts_v3(profile, 0, &[], &logical)
            .expect("alias of an executable representative downgrades");
        assert!(child[1].executable);
        assert!(child[2].executable, "alias lost its physical executability");
        assert_eq!(child[1].key, child[2].key);
        assert!(!child[2].is_signer && !child[2].is_writable);

        // The representative's own executable bit is still checked against the
        // physical account, in both directions.
        let hostile = [&plain, &plain, &plain];
        assert!(downgraded_effect_accounts_v3(profile, 0, &[], &hostile).is_err());
        let inverted = [&program, &program, &program];
        assert!(downgraded_effect_accounts_v3(profile, 0, &[], &inverted).is_err());
    }

    #[test]
    fn profile13_trailing_transport_span_selects_exact_scratch_pages() {
        const READONLY: AccountPrivilegesV2 = AccountPrivilegesV2::new(false, false, false);
        const NO_EFFECTS: AccountEffectPermissionsV2 =
            AccountEffectPermissionsV2::new(false, false, false);
        let rule = AccountRuleWithPrestateInputV2 {
            rule: AccountRuleInputV2 {
                privileges: READONLY,
                effect_permissions: NO_EFFECTS,
                alias: AccountAliasInputV2::SelfCoordinate,
                data_length: 0,
                data_item_stride: 0,
            },
            prestate: AccountPrestateV2::AuthenticatedOpaqueReadonlyData,
        };
        let fixed_rules = [rule; 5];
        let spans = [DynamicFixedSpanInputV2 {
            insertion_coordinate: 5,
            count_scalar: 0,
            rule_start: 0,
            rule_stride: 1,
            minimum: 1,
            maximum: 4,
            step: 1,
        }];
        let width = DYNAMIC_FIXED_SPAN_HEADER_BYTES
            + ACCOUNT_PROFILE_RULE_BYTES * (fixed_rules.len() + 1)
            + dclutch_account_profile_contract::v2::DYNAMIC_FIXED_SPAN_ENTRY_BYTES;
        let mut scratch = vec![0_u8; width];
        let mut bytes = vec![0_u8; width];
        encode_account_profile_with_dynamic_fixed_span_v2_atomic(
            TrustedEnvironmentV2::None,
            TrustedIdentityEnvironmentV2::None,
            TrustedBuiltinIdentityV2::SystemProgram { destination: 0 },
            &spans,
            &fixed_rules,
            &[rule],
            &[],
            RegisterGeometryV2 {
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 1,
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut bytes,
        )
        .expect("trailing scratch span");
        let profile = AccountProfileV2::decode(&bytes).expect("decode profile13");
        let make_account = || {
            AccountInfo::new(
                Box::leak(Box::new(Pubkey::new_unique())),
                false,
                false,
                Box::leak(Box::new(0_u64)),
                Box::leak(Vec::new().into_boxed_slice()),
                Box::leak(Box::new(Pubkey::new_unique())),
                false,
            )
        };
        let accounts = (0..7).map(|_| make_account()).collect::<Vec<_>>();
        let logical = accounts.iter().collect::<Vec<_>>();
        let pages = authenticated_input_scratch_pages_v3(profile, &[2], Some(0), &logical)
            .expect("exact trailing pages");
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].key, accounts[5].key);
        assert_eq!(pages[1].key, accounts[6].key);
        assert!(authenticated_input_scratch_pages_v3(profile, &[2], Some(1), &logical).is_err());
        assert!(authenticated_input_scratch_pages_v3(profile, &[4], Some(0), &logical).is_err());
    }

    #[cfg(any(
        feature = "families",
        feature = "series-family",
        feature = "dealer-family"
    ))]
    #[test]
    fn selected_family_profile_links_real_claims_and_custody_routes() {
        let _claims_route = execute_claims_route_v3;
        let _custody_preflight = preflight_custody_route_v3;
        let _custody_route = execute_custody_route_v3;

        assert_ne!(core::mem::size_of::<ClaimsRouteReceiptV3>(), 0);
        assert_ne!(core::mem::size_of::<CustodyCompositionParentV3>(), 0);
    }

    #[test]
    fn trusted_current_slot_survives_projection_boundary_and_reaches_transition() {
        let observation = TrustedEnvironmentObservationV3 {
            current_slot: Some((1, 42)),
            current_executing_program: Some((2, [0x91; 32])),
            system_program: Some((0, system_program::ID.to_bytes())),
        };
        let mut projected = [0_u64; 3];
        let mut projected_identities = [[0_u8; 32]; 3];
        seed_trusted_environment_v3(observation, &mut projected, &mut projected_identities)
            .expect("trusted seed");
        require_trusted_environment_v3(observation, &projected, &projected_identities)
            .expect("seed preserved");

        let mut hostile = projected;
        hostile[1] = 41;
        assert_eq!(
            require_trusted_environment_v3(observation, &hostile, &projected_identities),
            Err(TradingSbfError::Content.into())
        );
        let mut hostile_identities = projected_identities;
        hostile_identities[2] = [0x92; 32];
        assert_eq!(
            require_trusted_environment_v3(observation, &projected, &hostile_identities),
            Err(TradingSbfError::Content.into())
        );
        let mut hostile_builtin = projected_identities;
        hostile_builtin[0] = [0x93; 32];
        assert_eq!(
            require_trusted_environment_v3(observation, &projected, &hostile_builtin),
            Err(TradingSbfError::Content.into())
        );

        let width = TRANSITION_HEADER_BYTES_V3 + TRANSITION_INSTRUCTION_BYTES_V3;
        let mut program_scratch = vec![0_u8; width];
        let mut program_bytes = vec![0_u8; width];
        encode_program_atomic(
            ProgramGeometryV3 {
                common_scalars: 3,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &[InstructionV3::copy_scalar(
                ScalarRegisterV3::common(1),
                ScalarRegisterV3::common(2),
            )],
            &[],
            &[],
            &mut program_scratch,
            &mut program_bytes,
        )
        .expect("transition program");
        let program = TransitionProgramV3::decode(&program_bytes).expect("transition decode");
        let mut transition_scratch = [0_u64; 3];
        let mut transition_output = [0_u64; 3];
        execute_fold_atomic(
            program,
            0,
            RegisterInput {
                scalars: &projected,
                identities: &[],
            },
            RegisterOutput {
                scalars: &mut transition_scratch,
                identities: &mut [],
            },
            RegisterOutput {
                scalars: &mut transition_output,
                identities: &mut [],
            },
        )
        .expect("transition sees trusted slot");
        assert_eq!(transition_output, [0, 42, 42]);
    }

    #[test]
    fn root_header_and_alias_projection_cannot_be_written() {
        let root_header = ResolvedEffectV3::WriteScalar {
            account: 0,
            offset: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 - 8).expect("offset"),
            value: 9,
        };
        assert!(require_root_write_is_state_only(root_header, &[0, 1]).is_err());

        let first_state_byte = ResolvedEffectV3::WriteScalar {
            account: 0,
            offset: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1).expect("offset"),
            value: 9,
        };
        assert_eq!(
            require_root_write_is_state_only(first_state_byte, &[0, 1]),
            Ok(())
        );

        let aliased_header = ResolvedEffectV3::WriteIdentity {
            account: 1,
            offset: 0,
            value: [7; 32],
        };
        assert!(require_root_write_is_state_only(aliased_header, &[0, 0]).is_err());

        let ordinary_account = ResolvedEffectV3::WriteIdentity {
            account: 1,
            offset: 0,
            value: [7; 32],
        };
        assert_eq!(
            require_root_write_is_state_only(ordinary_account, &[0, 1]),
            Ok(())
        );

        let narrow_root_header = ResolvedEffectV3::WriteU8 {
            account: 0,
            offset: u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 - 1).expect("offset"),
            value: 1,
        };
        assert!(require_root_write_is_state_only(narrow_root_header, &[0, 1]).is_err());
    }

    #[test]
    fn typed_writes_initialize_zeroed_lifecycle_state_exactly() {
        let root_key = Box::leak(Box::new(Pubkey::new_unique()));
        let state_key = Box::leak(Box::new(Pubkey::new_unique()));
        let owner = Box::leak(Box::new(Pubkey::new_unique()));
        let root_lamports = Box::leak(Box::new(1_u64));
        let state_lamports = Box::leak(Box::new(1_u64));
        let root_data = Box::leak(Vec::new().into_boxed_slice());
        let state_data = Box::leak(vec![0_u8; 16].into_boxed_slice());
        let root = AccountInfo::new(
            root_key,
            false,
            true,
            root_lamports,
            root_data,
            owner,
            false,
        );
        let state = AccountInfo::new(
            state_key,
            false,
            true,
            state_lamports,
            state_data,
            owner,
            false,
        );
        let accounts = [&root, &state];
        let aliases = [0, 1];
        for effect in [
            ResolvedEffectV3::WriteU8 {
                account: 1,
                offset: 0,
                value: 0xa1,
            },
            ResolvedEffectV3::WriteU16 {
                account: 1,
                offset: 1,
                value: 0xb2c3,
            },
            ResolvedEffectV3::WriteU32 {
                account: 1,
                offset: 3,
                value: 0xd4e5_f607,
            },
        ] {
            commit_data_effect(effect, &accounts, &aliases, false).expect("typed write");
        }
        let data = state.try_borrow_data().expect("state data");
        assert_eq!(data.first(), Some(&0xa1));
        assert_eq!(data.get(1..3), Some(0xb2c3_u16.to_le_bytes().as_slice()));
        assert_eq!(
            data.get(3..7),
            Some(0xd4e5_f607_u32.to_le_bytes().as_slice())
        );
        assert!(data.get(7..).expect("tail").iter().all(|byte| *byte == 0));
    }

    #[test]
    fn lifecycle_candidate_updates_every_alias_and_reserves_nonroot_once() {
        let plan = PreparedLifecycleInvocationV3 {
            plan: StateLifecyclePlanV3::Create(CreateStatePlanV3 {
                state: [1; 32],
                payer: [2; 32],
                rent_credit: [3; 32],
                beneficiary: [4; 32],
                target_data_bytes: 144,
                historical_rent_principal: 30,
                state_before: 5,
                state_after: 30,
                payer_debit: 25,
                payer_after: 75,
                bump: 9,
            }),
            state: 1,
            payer: Some(2),
            rent_credit: Some(4),
            seeds: Vec::new(),
            immutable_identity_bindings: Vec::new(),
        };
        let aliases = [0, 1, 2, 1, 4];
        let mut accounts = vec![
            AccountInput {
                lamports: 1,
                data_len: 8,
            };
            aliases.len()
        ];
        apply_lifecycle_candidates_v3(&[plan], &aliases, &mut accounts).expect("candidate applies");
        let state_after = accounts.get(1).expect("state candidate");
        assert_eq!(state_after.lamports, 30);
        assert_eq!(state_after.data_len, 144);
        assert_eq!(accounts.get(3), Some(state_after));
        assert_eq!(accounts.get(2).map(|account| account.lamports), Some(75));

        let mut used = [false; 3];
        assert_eq!(
            reserve_lifecycle_state_v3(0, &mut used),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(reserve_lifecycle_state_v3(1, &mut used), Ok(()));
        assert_eq!(
            reserve_lifecycle_state_v3(1, &mut used),
            Err(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn lifecycle_immutable_binding_requires_one_exact_typed_write() {
        let binding = PreparedImmutableIdentityBindingV4 {
            data_offset: 16,
            canonical: [0x63; 32],
        };
        let aliases = [0, 1, 1];
        assert_eq!(
            inspect_lifecycle_binding_effect_v4(
                1,
                &binding,
                ResolvedEffectV3::WriteIdentity {
                    account: 2,
                    offset: 16,
                    value: binding.canonical,
                },
                &aliases,
            ),
            Ok(true)
        );
        assert_eq!(
            inspect_lifecycle_binding_effect_v4(
                1,
                &binding,
                ResolvedEffectV3::WriteIdentity {
                    account: 1,
                    offset: 16,
                    value: [0x64; 32],
                },
                &aliases,
            ),
            Err(TradingSbfError::Transition.into())
        );
        assert_eq!(
            inspect_lifecycle_binding_effect_v4(
                1,
                &binding,
                ResolvedEffectV3::WriteU32 {
                    account: 1,
                    offset: 44,
                    value: 0,
                },
                &aliases,
            ),
            Err(TradingSbfError::Transition.into())
        );
        assert_eq!(
            inspect_lifecycle_binding_effect_v4(
                1,
                &binding,
                ResolvedEffectV3::WriteIdentity {
                    account: 0,
                    offset: 16,
                    value: binding.canonical,
                },
                &aliases,
            ),
            Ok(false)
        );
    }

    #[test]
    fn lifecycle_pda_requires_canonical_bump_not_merely_valid_bump() {
        let program_id = Pubkey::new_from_array([0x71; 32]);
        let identity = [0x42; 32];
        let prefix = [b"general-state".as_slice(), identity.as_slice()];
        let (canonical_key, canonical_bump) = Pubkey::find_program_address(&prefix, &program_id);
        let canonical_seed = [canonical_bump];
        let canonical = [prefix[0], prefix[1], canonical_seed.as_slice()];
        assert_eq!(
            require_canonical_lifecycle_pda_v3(&program_id, &canonical),
            Ok(canonical_key)
        );

        let alternate = (0_u8..=u8::MAX)
            .find(|bump| {
                if *bump == canonical_bump {
                    return false;
                }
                let bump_seed = [*bump];
                Pubkey::create_program_address(
                    &[prefix[0], prefix[1], bump_seed.as_slice()],
                    &program_id,
                )
                .is_ok()
            })
            .expect("at least one noncanonical valid bump");
        let alternate_seed = [alternate];
        let hostile = [prefix[0], prefix[1], alternate_seed.as_slice()];
        assert!(require_canonical_lifecycle_pda_v3(&program_id, &hostile).is_err());
    }

    #[test]
    fn common_projection_bindings_and_child_reservations_are_exact() {
        let id = |tag: u8| [tag; 32];
        let physical = Pubkey::new_from_array(id(1));
        let projected = LogicalProjectionKeysV3 {
            selected_config: id(2),
            product_root: id(3),
            portfolio: id(4),
            linked_basis: id(5),
        };
        for (coordinate, expected) in [
            (0_usize, id(1)),
            (1, id(2)),
            (2, id(3)),
            (3, id(4)),
            (4, id(5)),
            (5, id(1)),
        ] {
            assert_eq!(
                *logical_projection_key_v3(coordinate, &physical, &projected),
                expected
            );
        }
        assert_ne!(
            *logical_projection_key_v3(1, &physical, &projected),
            id(1)
        );
        let canonical = CommonProjectionBindingsV3 {
            selected_config: id(1),
            selected_product_record: id(2),
            authenticated_product_record: id(2),
            market_product: id(3),
            runtime_product: id(3),
            product_semantic_basis: id(4),
            authenticated_semantic_basis: id(4),
            authenticated_linked_basis: id(5),
        };
        assert_eq!(require_common_projection_bindings_v3(canonical), Ok(()));
        for hostile in [
            // The selected config's binding to an authenticated finalized
            // record is owned by `borrow_finalized_record`, which refuses
            // before this predicate is reached; what stays here is the refusal
            // of an unset selection.
            CommonProjectionBindingsV3 {
                selected_config: [0; 32],
                ..canonical
            },
            CommonProjectionBindingsV3 {
                selected_product_record: id(6),
                ..canonical
            },
            CommonProjectionBindingsV3 {
                market_product: id(6),
                ..canonical
            },
            CommonProjectionBindingsV3 {
                product_semantic_basis: id(6),
                ..canonical
            },
            CommonProjectionBindingsV3 {
                authenticated_linked_basis: [0; 32],
                ..canonical
            },
        ] {
            assert_eq!(
                require_common_projection_bindings_v3(hostile),
                Err(TradingSbfError::Content.into())
            );
        }

        let invocation = dclutch_effect_kernel::v3::ResolvedInvocationV3 {
            role: FixedRole::Custody,
            kind: dclutch_effect_kernel::v3::RouteKindV3::Once,
            item: None,
            fixed_account_start: 1,
            fixed_account_count: 1,
            item_account_start: 0,
            item_account_count: 0,
            item_account_stride: 0,
            repeated_item_count: 0,
            request_offset: 0,
            request_len: 1,
            borrowed_witness: None,
            receipt_dependencies: dclutch_effect_kernel::v3::ResolvedReceiptDependenciesV3::empty(),
            receipt_dependency: None,
        };
        assert_eq!(
            require_no_common_projection_child_accounts_v3(invocation),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(
            require_no_common_projection_child_accounts_v3(
                dclutch_effect_kernel::v3::ResolvedInvocationV3 {
                    fixed_account_start: 5,
                    ..invocation
                }
            ),
            Ok(())
        );
        assert_eq!(require_tail_count_agreement_v3(7, 7), Ok(()));
        assert_eq!(
            require_tail_count_agreement_v3(7, 6),
            Err(TradingSbfError::Content.into())
        );
        let mut permissions = [AccountPermission::read_only(); 5];
        permissions[0] = AccountPermission::program_owned_mutable();
        assert_eq!(
            require_common_projection_permissions_v3(&permissions),
            Ok(())
        );
        permissions[2] = AccountPermission::program_owned_mutable();
        assert_eq!(
            require_common_projection_permissions_v3(&permissions),
            Err(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn projected_product_tail_count_is_rechecked_after_atomic_account_projection() {
        let rules = [AccountRuleInputV2 {
            privileges: AccountPrivilegesV2::new(false, false, false),
            effect_permissions: AccountEffectPermissionsV2::new(false, false, false),
            alias: AccountAliasInputV2::SelfCoordinate,
            data_length: 4,
            data_item_stride: 0,
        }];
        let operations = [AccountOperationInputV2::ProjectTailCountU32 {
            account: AccountCoordinateV2::fixed(0),
            destination: ScalarCoordinateV2::common(0),
            data_offset: 0,
        }];
        let bytes = ACCOUNT_PROFILE_HEADER_BYTES
            + ACCOUNT_PROFILE_RULE_BYTES
            + ACCOUNT_PROFILE_OPERATION_BYTES;
        let mut scratch = vec![0_u8; bytes];
        let mut encoded = vec![0_u8; bytes];
        encode_account_profile_v2_atomic(
            AccountProfileArtifactV2::TypedScalar,
            &rules,
            &[],
            &operations,
            &[],
            RegisterGeometryV2 {
                common_scalars: 1,
                item_scalar_stride: 0,
                common_identities: 0,
                item_identity_stride: 0,
            },
            &mut scratch,
            &mut encoded,
        )
        .expect("tail-count profile");
        let profile = AccountProfileV2::decode(&encoded).expect("decode profile");
        assert_eq!(
            require_projected_tail_count_agreement_v3(profile, 7, &[7]),
            Ok(())
        );
        assert_eq!(
            require_projected_tail_count_agreement_v3(profile, 7, &[6]),
            Err(TradingSbfError::Content.into())
        );
    }

    #[test]
    fn admitted_runtime_follows_the_exact_chunk_authority_vector() {
        assert_eq!(HOT_ADMITTED_CALLER_AUTHORITIES_START_V3, 46);
        assert_eq!(hot_admitted_runtime_accounts_start_v3(1, 1), Ok(47));
        assert_eq!(hot_admitted_runtime_accounts_start_v3(120, 2), Ok(48));
        assert!(hot_admitted_runtime_accounts_start_v3(0, 0).is_err());
    }
}
