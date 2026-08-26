//! Family-neutral Trading V3 hot execution boundary.
//!
//! This module owns the common physical interpreter path. It authenticates the
//! Market, immutable root selection, finalized artifact graph, and current
//! release programs before projecting any mutation. The first executable cut
//! accepts interpreted programs with local effects and no fixed-role route;
//! child routes remain fail-closed until their canonical producer receipts are
//! consumed here.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_account_profile_contract::{
    AccountObservationV1,
    lifecycle_v3::{
        AuthenticatedRentCreditV3, AuthenticatedRentMinimumV3, LifecycleContextV3,
        LifecycleOperationV3, LifecycleRegistersV3,
        SCHEMA_RELEASE_ID as STATE_LIFECYCLE_POLICY_SCHEMA_ID_V3, SeedValueV3,
        StateLifecyclePlanV3, StateLifecyclePolicyV3, plan_lifecycle,
    },
    v2::{
        AccountProfileV2, ProjectionRegistersV2, SCHEMA_RELEASE_ID as ACCOUNT_PROFILE_SCHEMA_ID_V2,
        derive_effect_permissions, project_atomic as project_accounts_atomic,
        project_tail_count_atomic,
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
    set_v1::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1, CapabilityProgramSetV1},
    v3::{
        CAPABILITY_PROGRAM_V3_BYTES, CapabilityProgramV3, SCHEMA_RELEASE_ID as PROGRAM_SCHEMA_ID_V3,
    },
};
use dclutch_effect_kernel::{
    v2::{AccountInput, AccountPermission, FixedRole},
    v3::{
        ProgramV3 as EffectProgramV3, ProjectionV3, ResolvedEffectV3,
        SCHEMA_RELEASE_ID as EFFECT_SCHEMA_ID_V3, project_atomic as project_effects_atomic,
    },
};
use dclutch_execution_strategy_contract::v2::{
    EXECUTION_STRATEGY_PROGRAM_BYTES_V2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_product_runtime_v2::ContentId as ProductContentId;
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2 as ProductRecordFrameV2, ProductRuntimeFrameV2,
    authenticate_product_runtime_v2,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::ACTIVATION_PDA_DOMAIN_V1;
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_rent_contract::{RENT_CREDIT_BYTES_V1, RentCreditV1};
use dclutch_request_profile_contract::{
    ProjectionRegistersV1, RequestProfileV1, SCHEMA_RELEASE_ID as REQUEST_PROFILE_SCHEMA_ID_V1,
    project_atomic as project_request_atomic,
    v2::{NativeSignatureRegistersV1, REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID, RequestProfileV2},
};
use dclutch_transition_vm::v3::{
    ProgramV3 as TransitionProgramV3, RegisterInput, RegisterOutput,
    SCHEMA_RELEASE_ID as TRANSITION_SCHEMA_ID_V3, execute_fold_atomic,
};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign, transfer as system_transfer};

use crate::{
    TradingSbfError,
    core_composition_v3::{
        CoreCompositionParentV3, execute_core_route_v3, preflight_core_route_v3,
    },
    dispatch::TradingFamilyContextV1,
    execution_strategy_v2::{
        ADMITTED_AOT_STRATEGY_ACCOUNT_COUNT_V2, INTERPRETED_STRATEGY_ACCOUNT_COUNT_V2,
        SHADOW_AOT_STRATEGY_ACCOUNT_COUNT_V2, authenticate_execution_strategy_v2,
    },
    native_signature::{
        authenticate_and_seed_native_signatures, authenticate_current_top_level_instruction,
    },
};

#[cfg(feature = "families")]
use crate::resolution_composition_v3::{
    ResolutionCompositionParentV3, execute_resolution_route_v3, preflight_resolution_route_v3,
};

#[cfg(feature = "families")]
use crate::{
    claims_composition_v3::{ClaimsRouteReceiptV3, execute_claims_route_v3},
    custody_composition_v3::{
        CustodyCompositionParentV3, execute_custody_route_v3, preflight_custody_route_v3,
    },
};
#[cfg(feature = "families")]
use dclutch_claims_svm::composition_v3::{ClaimsCompositionParentV3, ClaimsCompositionV3};
#[cfg(feature = "families")]
use dclutch_registry_contract::ActivatedExecutionReleaseSetViewV1;

// These are SBF-heap profile bounds, not semantic/product limits. The lifting
// path is scratch-page transport under authenticated ExecutionStrategy V2.
const MAX_HOT_RUNTIME_ACCOUNTS_V3: usize = 256;
const MAX_HOT_SCALARS_V3: usize = 512;
const MAX_HOT_IDENTITIES_V3: usize = 128;
const MAX_HOT_REQUEST_BYTES_V3: usize = 8_192;

const EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-execution:v3";
const CHILD_EXECUTION_DIGEST_DOMAIN_V3: &[u8] = b"dclutch:hot-child-execution:v3";

/// Execute one complete common V3 hot action.
#[inline(never)]
pub fn process_hot_execution_v3(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let (envelope, family_request) = HotExecutionEnvelopeV3::split_instruction(instruction_data)
        .map_err(|_| TradingSbfError::Content)?;
    let frame = HotFrameV3::parse(program_id, accounts)?;
    authenticate_current_top_level_instruction(
        program_id,
        accounts,
        instruction_data,
        frame.instructions,
    )?;
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

    let market = authenticate_market(frame, envelope)?;
    let core_receipt = reauthenticate_role(
        frame,
        ExecutionRoleV1::Core,
        frame.core_program,
        frame.core_programdata,
        envelope.release_set(),
    )?;
    if core_receipt.program().as_bytes() != &frame.core_program.key.to_bytes() {
        return Err(TradingSbfError::Release.into());
    }
    let trading_receipt = reauthenticate_role(
        frame,
        ExecutionRoleV1::Trading,
        frame.trading_program,
        frame.trading_programdata,
        envelope.release_set(),
    )?;
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
    let immutable_root_header = root_header.to_bytes();
    drop(root_data);

    let rent = Rent::from_account_info(frame.rent).map_err(|_| TradingSbfError::Content)?;
    let product_runtime = authenticate_product_runtime_v2(
        frame.registry.key,
        &rent,
        ProductContentId::new(market.identity.product_record.to_bytes())
            .map_err(|_| TradingSbfError::Content)?,
        ProductRuntimeFrameV2 {
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
        },
    )
    .map_err(|_| TradingSbfError::Content)?;
    let product_outcome_count = product_runtime.outcome_count;
    let manifest_data = borrow_finalized_record(
        frame,
        frame.manifest_raw,
        frame.manifest_staging,
        &rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        context.selection().manifest().to_bytes(),
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| TradingSbfError::Content)?;
    let entry = manifest
        .entry(context.selection().entry_index())
        .map_err(|_| TradingSbfError::Content)?;
    if entry.kind_id() != context.selection().kind()
        || entry.release_id() != context.selection().capability_release()
        || entry.config_id() != context.selection().config()
    {
        return Err(TradingSbfError::Content.into());
    }

    let program_set_data = borrow_finalized_record(
        frame,
        frame.program_set_raw,
        frame.program_set_staging,
        &rent,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V1,
        context.selection().capability_release().to_bytes(),
    )?;
    let program_set = CapabilityProgramSetV1::decode_selected(
        context.selection().capability_release().to_bytes(),
        hash(&program_set_data).to_bytes(),
        &program_set_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let selected_entry = program_set
        .select_entry(family_request)
        .map_err(|_| TradingSbfError::Content)?;
    let selected_program = selected_entry.program();
    let selected_action = selected_entry.selector();

    let descriptor_data = borrow_finalized_record(
        frame,
        frame.descriptor_raw,
        frame.descriptor_staging,
        &rent,
        PROGRAM_SCHEMA_ID_V3,
        selected_program.to_bytes(),
    )?;
    if descriptor_data.len() != CAPABILITY_PROGRAM_V3_BYTES {
        return Err(TradingSbfError::Content.into());
    }
    let descriptor =
        CapabilityProgramV3::decode(&descriptor_data).map_err(|_| TradingSbfError::Content)?;
    authenticate_descriptor_root_selection(descriptor, context, entry)?;

    let config_data = borrow_finalized_record(
        frame,
        frame.config_raw,
        frame.config_staging,
        &rent,
        descriptor.config_schema().to_bytes(),
        context.selection().config().to_bytes(),
    )?;
    let config_digest = hash(&config_data).to_bytes();
    drop(config_data);
    require_common_projection_bindings_v3(
        context.selection().config().to_bytes(),
        config_digest,
        market.identity.product_record.to_bytes(),
        product_runtime.product_record.content_digest.to_bytes(),
        market.identity.product_id.to_bytes(),
        product_runtime.product_id.to_bytes(),
    )?;
    let lifecycle_data = borrow_finalized_record(
        frame,
        frame.lifecycle_raw,
        frame.lifecycle_staging,
        &rent,
        STATE_LIFECYCLE_POLICY_SCHEMA_ID_V3,
        descriptor.derivation_policy().to_bytes(),
    )?;
    let lifecycle = StateLifecyclePolicyV3::decode_selected(
        descriptor.derivation_policy().to_bytes(),
        hash(&lifecycle_data).to_bytes(),
        &lifecycle_data,
    )
    .map_err(|_| TradingSbfError::Content)?;

    let account_profile_data = borrow_finalized_record(
        frame,
        frame.account_profile_raw,
        frame.account_profile_staging,
        &rent,
        ACCOUNT_PROFILE_SCHEMA_ID_V2,
        descriptor.account_profile().to_bytes(),
    )?;
    let account_profile =
        AccountProfileV2::decode(&account_profile_data).map_err(|_| TradingSbfError::Content)?;
    lifecycle
        .validate_account_profile(account_profile)
        .map_err(|_| TradingSbfError::Content)?;

    let request_profile_data = borrow_finalized_record(
        frame,
        frame.request_profile_raw,
        frame.request_profile_staging,
        &rent,
        descriptor.request_profile_schema().to_bytes(),
        descriptor.request_profile_program().to_bytes(),
    )?;
    let request_profile = decode_request_profile(descriptor, &request_profile_data)?;

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
    let runtime_start = HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3
        .checked_add(strategy_extra_count)
        .ok_or(TradingSbfError::Content)?;
    let strategy_extras = accounts
        .get(HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3..runtime_start)
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
        selected_program,
        frame.registry,
        frame.rent,
        &strategy_accounts,
    )?;
    if strategy.strategy().disposition() != StrategyDispositionV2::Interpreted {
        return Err(TradingSbfError::UnsupportedContent.into());
    }

    let transition_data = borrow_finalized_record(
        frame,
        frame.transition_raw,
        frame.transition_staging,
        &rent,
        strategy.strategy().transition_schema().to_bytes(),
        strategy.strategy().transition_program().to_bytes(),
    )?;
    if strategy.strategy().transition_schema().to_bytes() != TRANSITION_SCHEMA_ID_V3 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let transition =
        TransitionProgramV3::decode(&transition_data).map_err(|_| TradingSbfError::Content)?;

    let effect_data = borrow_finalized_record(
        frame,
        frame.effect_raw,
        frame.effect_staging,
        &rent,
        EFFECT_SCHEMA_ID_V3,
        descriptor.effect_program().to_bytes(),
    )?;
    let effect = EffectProgramV3::decode_selected(
        descriptor.effect_program().to_bytes(),
        hash(&effect_data).to_bytes(),
        &effect_data,
    )
    .map_err(|_| TradingSbfError::Content)?;

    let mut runtime_accounts = Vec::new();
    runtime_accounts.extend_from_slice(&[
        frame.root,
        frame.config_raw,
        frame.product_raw,
        frame.portfolio_raw,
    ]);
    runtime_accounts.extend(
        accounts
            .get(runtime_start..)
            .ok_or(TradingSbfError::Content)?,
    );
    if runtime_accounts.len() > MAX_HOT_RUNTIME_ACCOUNTS_V3 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let runtime_data = runtime_accounts
        .iter()
        .map(|account| {
            account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let observations = runtime_accounts
        .iter()
        .zip(&runtime_data)
        .map(|(account, data)| {
            AccountObservationV1::new(
                account.key.to_bytes(),
                account.owner.to_bytes(),
                account.lamports(),
                data.as_ref(),
                account.is_signer,
                account.is_writable,
                account.executable,
            )
        })
        .collect::<Vec<_>>();

    let tail_count = project_tail_count(account_profile, &observations, request_digest)?;
    require_tail_count_agreement_v3(product_outcome_count, tail_count)?;
    require_geometry(
        account_profile,
        request_profile.v1(),
        transition,
        effect,
        tail_count,
        family_request.len(),
        runtime_accounts.len(),
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

    let input_scalars = vec![0_u64; scalar_count];
    let mut input_identities = vec![[0_u8; 32]; identity_count];
    *input_identities
        .get_mut(HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3)
        .ok_or(TradingSbfError::Content)? = request_digest;
    let mut account_scratch_scalars = input_scalars.clone();
    let mut account_scratch_identities = input_identities.clone();
    let mut account_output_scalars = input_scalars.clone();
    let mut account_output_identities = input_identities.clone();
    project_accounts_atomic(
        account_profile,
        tail_count,
        &observations,
        ProjectionRegistersV2 {
            input_scalars: &input_scalars,
            input_identities: &input_identities,
            scratch_scalars: &mut account_scratch_scalars,
            scratch_identities: &mut account_scratch_identities,
            output_scalars: &mut account_output_scalars,
            output_identities: &mut account_output_identities,
        },
    )
    .map_err(|_| TradingSbfError::Content)?;

    let mut signed_identities = account_output_identities.clone();
    if let RequestProfileKindV3::Signed(profile) = request_profile {
        let mut signature_scratch = account_output_identities.clone();
        authenticate_and_seed_native_signatures(
            program_id,
            accounts,
            instruction_data,
            frame.instructions,
            profile,
            tail_count,
            NativeSignatureRegistersV1 {
                input_identities: &account_output_identities,
                scratch_identities: &mut signature_scratch,
                output_identities: &mut signed_identities,
            },
        )?;
    }

    let mut request_scratch_scalars = account_output_scalars.clone();
    let mut request_scratch_identities = signed_identities.clone();
    let mut request_output_scalars = account_output_scalars.clone();
    let mut request_output_identities = signed_identities.clone();
    project_request_atomic(
        request_profile.v1(),
        tail_count,
        family_request,
        ProjectionRegistersV1 {
            input_scalars: &account_output_scalars,
            input_identities: &signed_identities,
            scratch_scalars: &mut request_scratch_scalars,
            scratch_identities: &mut request_scratch_identities,
            output_scalars: &mut request_output_scalars,
            output_identities: &mut request_output_identities,
        },
    )
    .map_err(|_| TradingSbfError::Content)?;

    let mut transition_scratch_scalars = request_output_scalars.clone();
    let mut transition_scratch_identities = request_output_identities.clone();
    let mut transition_output_scalars = request_output_scalars.clone();
    let mut transition_output_identities = request_output_identities.clone();
    execute_fold_atomic(
        transition,
        tail_count,
        RegisterInput {
            scalars: &request_output_scalars,
            identities: &request_output_identities,
        },
        RegisterOutput {
            scalars: &mut transition_scratch_scalars,
            identities: &mut transition_scratch_identities,
        },
        RegisterOutput {
            scalars: &mut transition_output_scalars,
            identities: &mut transition_output_identities,
        },
    )
    .map_err(|_| TradingSbfError::Transition)?;

    let aliases = (0..runtime_accounts.len())
        .map(|coordinate| {
            account_profile
                .representative(tail_count, coordinate)
                .map_err(|_| TradingSbfError::Content)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let lifecycle_plans = prepare_lifecycle_v3(
        program_id,
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
    let mut account_inputs = observations
        .iter()
        .map(|observation| AccountInput {
            lamports: observation.lamports(),
            data_len: observation.data().len(),
        })
        .collect::<Vec<_>>();
    apply_lifecycle_candidates_v3(&lifecycle_plans, &aliases, &mut account_inputs)?;
    let mut permissions = vec![AccountPermission::read_only(); runtime_accounts.len()];
    derive_effect_permissions(account_profile, tail_count, &mut permissions)
        .map_err(|_| TradingSbfError::Content)?;
    require_common_projection_permissions_v3(&permissions)?;
    let mut scratch_lamports = vec![0_u64; runtime_accounts.len()];
    let mut output_lamports = vec![0_u64; runtime_accounts.len()];
    let mut scratch_requests = vec![0_u8; request_bytes];
    let mut output_requests = vec![0_u8; request_bytes];
    project_effects_atomic(
        effect,
        tail_count,
        ProjectionV3 {
            scalars: &transition_output_scalars,
            identities: &transition_output_identities,
            aliases: &aliases,
            accounts: &account_inputs,
            permissions: &permissions,
            scratch_lamports: &mut scratch_lamports,
            output_lamports: &mut output_lamports,
            scratch_requests: &mut scratch_requests,
            output_requests: &mut output_requests,
        },
    )
    .map_err(|_| TradingSbfError::Transition)?;

    preflight_local_effects(
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        &aliases,
    )?;
    let effect_accounts = runtime_accounts
        .iter()
        .map(|account| (*account).clone())
        .collect::<Vec<_>>();
    preflight_child_routes_v3(
        program_id,
        frame,
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
    drop(observations);
    drop(runtime_data);
    apply_lifecycle_creates_v3(program_id, &lifecycle_plans, &runtime_accounts)?;
    let child_execution_digest = execute_child_routes_v3(
        program_id,
        frame,
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
    )?;
    apply_lifecycle_closes_v3(program_id, &lifecycle_plans, &runtime_accounts, &rent)?;
    commit_local_effects(
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        &runtime_accounts,
        &aliases,
        &output_lamports,
        &rent,
        false,
    )?;
    commit_local_effects(
        effect,
        tail_count,
        &transition_output_scalars,
        &transition_output_identities,
        &runtime_accounts,
        &aliases,
        &output_lamports,
        &rent,
        true,
    )?;
    let root_poststate = {
        let bytes = frame
            .root
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Commit)?;
        if bytes.get(..CAPABILITY_ROOT_HEADER_BYTES_V1) != Some(immutable_root_header.as_slice()) {
            return Err(TradingSbfError::Commit.into());
        }
        hash(&bytes).to_bytes()
    };
    let execution_digest = hashv(&[
        EXECUTION_DIGEST_DOMAIN_V3,
        &selected_program.to_bytes(),
        &descriptor.account_profile().to_bytes(),
        &descriptor.request_profile_program().to_bytes(),
        &strategy.strategy_program_id().to_bytes(),
        &strategy.strategy().transition_program().to_bytes(),
        &descriptor.effect_program().to_bytes(),
        &descriptor.derivation_policy().to_bytes(),
        &context.selection().config().to_bytes(),
        &market.identity.product_record.to_bytes(),
        &product_outcome_count.to_le_bytes(),
        &request_digest,
        &child_execution_digest,
        &root_poststate,
    ])
    .to_bytes();
    let ack = HotExecutionAckV3::new(HotExecutionAckV3 {
        release_set: envelope.release_set(),
        market: envelope.market(),
        generation: envelope.generation(),
        root: frame.root.key.to_bytes(),
        request_digest,
        selected_program: selected_program.to_bytes(),
        root_prestate_digest: root_prestate,
        root_poststate_digest: root_poststate,
        execution_digest,
    })
    .map_err(|_| TradingSbfError::Commit)?;
    set_return_data(&ack.to_bytes());
    Ok(())
}

fn require_common_projection_bindings_v3(
    selected_config: [u8; 32],
    authenticated_config: [u8; 32],
    selected_product_record: [u8; 32],
    authenticated_product_record: [u8; 32],
    market_product: [u8; 32],
    runtime_product: [u8; 32],
) -> Result<(), ProgramError> {
    if selected_config == [0; 32]
        || selected_config != authenticated_config
        || selected_product_record == [0; 32]
        || selected_product_record != authenticated_product_record
        || market_product == [0; 32]
        || market_product != runtime_product
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
    {
        Err(TradingSbfError::Content.into())
    } else {
        Ok(())
    }
}

struct PreparedLifecycleInvocationV3 {
    plan: StateLifecyclePlanV3,
    state: usize,
    payer: Option<usize>,
    rent_credit: Option<usize>,
    seeds: Vec<SeedValueV3>,
}

#[allow(clippy::too_many_arguments)]
fn prepare_lifecycle_v3<'a>(
    program_id: &Pubkey,
    policy: StateLifecyclePolicyV3<'_>,
    action: u32,
    account_profile: AccountProfileV2<'_>,
    tail_count: u32,
    observations: &[AccountObservationV1<'a>],
    accounts: &[&AccountInfo<'_>],
    scalars: &[u64],
    identities: &[[u8; 32]],
    rent: &Rent,
    aliases: &[usize],
) -> Result<Vec<PreparedLifecycleInvocationV3>, ProgramError> {
    if observations.len() != accounts.len() || aliases.len() != accounts.len() {
        return Err(TradingSbfError::Content.into());
    }
    let registers = LifecycleRegistersV3 {
        scalars,
        identities,
    };
    let mut candidate_lamports = observations
        .iter()
        .map(|observation| observation.lamports())
        .collect::<Vec<_>>();
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
            if !selected
                .is_enabled(account_profile, tail_count, item, registers)
                .map_err(|_| TradingSbfError::Content)?
            {
                invocation = invocation.checked_add(1).ok_or(TradingSbfError::Content)?;
                continue;
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
            let mut seed = 0_u8;
            while seed < seed_count {
                seeds.push(
                    selected
                        .materialize_seed(account_profile, tail_count, item, registers, seed)
                        .map_err(|_| TradingSbfError::Content)?,
                );
                seed = seed.checked_add(1).ok_or(TradingSbfError::Content)?;
            }
            let seed_slices = seeds.iter().map(SeedValueV3::as_slice).collect::<Vec<_>>();
            let derived = Pubkey::create_program_address(&seed_slices, program_id)
                .map_err(|_| TradingSbfError::Content)?;
            if accounts
                .get(state)
                .is_none_or(|account| account.key != &derived)
            {
                return Err(TradingSbfError::Content.into());
            }
            let candidate_observations = observations
                .iter()
                .zip(accounts)
                .zip(&candidate_lamports)
                .map(|((observation, account), lamports)| {
                    AccountObservationV1::new(
                        observation.key(),
                        observation.owner(),
                        *lamports,
                        observation.data(),
                        account.is_signer,
                        account.is_writable,
                        account.executable,
                    )
                })
                .collect::<Vec<_>>();
            let authenticated_credit = rent_credit
                .map(|index| {
                    authenticate_lifecycle_credit_v3(
                        accounts,
                        index,
                        *candidate_lamports
                            .get(index)
                            .ok_or(TradingSbfError::Content)?,
                        rent,
                    )
                })
                .transpose()?;
            let current_rent_minimum = if selected.operation() == LifecycleOperationV3::Create {
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
            let plan = plan_lifecycle(
                selected,
                LifecycleContextV3 {
                    account_profile,
                    tail_count,
                    item_index: item,
                    accounts: &candidate_observations,
                    registers,
                    trading_program: program_id.to_bytes(),
                    system_program: system_program::ID.to_bytes(),
                    adapter_derived_pda: derived.to_bytes(),
                    rent_credit: authenticated_credit,
                    current_rent_minimum,
                },
            )
            .map_err(|_| TradingSbfError::Content)?;
            match plan {
                StateLifecyclePlanV3::Authenticate(_) => {}
                StateLifecyclePlanV3::Create(value) => {
                    *candidate_lamports
                        .get_mut(state)
                        .ok_or(TradingSbfError::Content)? = value.state_after;
                    *candidate_lamports
                        .get_mut(payer.ok_or(TradingSbfError::Content)?)
                        .ok_or(TradingSbfError::Content)? = value.payer_after;
                }
                StateLifecyclePlanV3::Close(value) => {
                    *candidate_lamports
                        .get_mut(state)
                        .ok_or(TradingSbfError::Content)? = value.source_after;
                    *candidate_lamports
                        .get_mut(rent_credit.ok_or(TradingSbfError::Content)?)
                        .ok_or(TradingSbfError::Content)? = value.rent_credit_after;
                }
            }
            output.push(PreparedLifecycleInvocationV3 {
                plan,
                state,
                payer,
                rent_credit,
                seeds,
            });
            invocation = invocation.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        ordinal = ordinal.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(output)
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
) -> Result<AuthenticatedRentCreditV3, ProgramError> {
    let account = accounts.get(index).ok_or(TradingSbfError::Content)?;
    if account.is_signer
        || !account.is_writable
        || account.executable
        || account.data_len() != RENT_CREDIT_BYTES_V1
        || !rent.is_exempt(observed_lamports, RENT_CREDIT_BYTES_V1)
    {
        return Err(TradingSbfError::Content.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let credit = RentCreditV1::decode(&data).map_err(|_| TradingSbfError::Content)?;
    if credit.to_bytes().as_slice() != data.as_ref() {
        return Err(TradingSbfError::Content.into());
    }
    let seeds = credit.pda_seeds();
    let authority = seeds.refund_authority().to_bytes();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[seeds.domain(), authority.as_slice(), &bump],
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
        let seed_slices = prepared
            .seeds
            .iter()
            .map(SeedValueV3::as_slice)
            .collect::<Vec<_>>();
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

#[allow(clippy::too_many_arguments)]
fn preflight_child_routes_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: HotFrameV3<'accounts, 'info>,
    effect: EffectProgramV3<'_>,
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
    #[cfg(feature = "families")]
    let claims_composition =
        if has_active_role(effect, tail_count, scalars, identities, FixedRole::Claims)? {
            Some(
                ClaimsCompositionV3::decode_selected(
                    effect,
                    tail_count,
                    scalars,
                    identities,
                    request_bank,
                    ClaimsCompositionParentV3 {
                        release_set: envelope.release_set(),
                        market: envelope.market(),
                        generation: envelope.generation(),
                        parent_request_digest: request_digest,
                    },
                )
                .map_err(|_| TradingSbfError::Content)?,
            )
        } else {
            None
        };
    #[cfg(feature = "families")]
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
    #[cfg(feature = "families")]
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
            require_no_common_projection_child_accounts_v3(invocation)?;
            require_child_disjoint_from_local(invocation, aliases, &locally_mutated)?;
            match invocation.role {
                FixedRole::Core => preflight_core_route_v3(
                    program_id,
                    effect,
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
                    #[cfg(feature = "families")]
                    {
                        let composition = claims_composition.ok_or(TradingSbfError::Content)?;
                        let selected = claims_program.ok_or(TradingSbfError::Release)?;
                        if invocation_index != 0
                            || !(composition.admit_route() == Some(route)
                                || composition.affine_route() == route
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
                    #[cfg(not(feature = "families"))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Custody => {
                    #[cfg(feature = "families")]
                    preflight_custody_route_v3(
                        program_id,
                        effect,
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
                    #[cfg(not(feature = "families"))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Resolution => {
                    #[cfg(feature = "families")]
                    preflight_resolution_route_v3(
                        program_id,
                        effect,
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

#[allow(clippy::too_many_arguments)]
fn execute_child_routes_v3<'accounts, 'info>(
    program_id: &Pubkey,
    frame: HotFrameV3<'accounts, 'info>,
    effect: EffectProgramV3<'_>,
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
    let mut transcript = hashv(&[CHILD_EXECUTION_DIGEST_DOMAIN_V3, &request_digest]).to_bytes();
    if effect.route_count() == 0 {
        return Ok(transcript);
    }
    #[cfg(feature = "families")]
    let claims_composition =
        if has_active_role(effect, tail_count, scalars, identities, FixedRole::Claims)? {
            Some(
                ClaimsCompositionV3::decode_selected(
                    effect,
                    tail_count,
                    scalars,
                    identities,
                    request_bank,
                    ClaimsCompositionParentV3 {
                        release_set: envelope.release_set(),
                        market: envelope.market(),
                        generation: envelope.generation(),
                        parent_request_digest: request_digest,
                    },
                )
                .map_err(|_| TradingSbfError::Content)?,
            )
        } else {
            None
        };
    #[cfg(feature = "families")]
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
    #[cfg(feature = "families")]
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
        let mut invocation = 0_u32;
        while invocation < count {
            let (role, child_digest) = match effect
                .route(route)
                .map_err(|_| TradingSbfError::Content)?
                .role()
            {
                FixedRole::Core => (
                    FixedRole::Core,
                    execute_core_route_v3(
                        program_id,
                        effect,
                        route,
                        invocation,
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
                ),
                FixedRole::Claims => {
                    #[cfg(feature = "families")]
                    {
                        let receipt = execute_claims_route_v3(
                            program_id,
                            effect,
                            claims_composition.ok_or(TradingSbfError::Content)?,
                            route,
                            tail_count,
                            scalars,
                            identities,
                            effect_accounts,
                            request_bank,
                            claims_program.ok_or(TradingSbfError::Release)?,
                        )?;
                        (FixedRole::Claims, claims_receipt_digest_v3(receipt)?)
                    }
                    #[cfg(not(feature = "families"))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Custody => {
                    #[cfg(feature = "families")]
                    {
                        let digest = execute_custody_route_v3(
                            program_id,
                            effect,
                            route,
                            invocation,
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
                        (FixedRole::Custody, digest)
                    }
                    #[cfg(not(feature = "families"))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
                FixedRole::Resolution => {
                    #[cfg(feature = "families")]
                    {
                        let digest = execute_resolution_route_v3(
                            program_id,
                            effect,
                            route,
                            invocation,
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
                        (FixedRole::Resolution, digest)
                    }
                    #[cfg(not(feature = "families"))]
                    return Err(TradingSbfError::UnsupportedContent.into());
                }
            };
            transcript = hashv(&[
                CHILD_EXECUTION_DIGEST_DOMAIN_V3,
                &transcript,
                &[fixed_role_tag_v3(role)],
                &route.to_le_bytes(),
                &invocation.to_le_bytes(),
                &child_digest,
            ])
            .to_bytes();
            invocation = invocation.checked_add(1).ok_or(TradingSbfError::Content)?;
        }
        route = route.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(transcript)
}

fn fixed_role_tag_v3(role: FixedRole) -> u8 {
    match role {
        FixedRole::Core => 0,
        FixedRole::Claims => 1,
        FixedRole::Resolution => 3,
        FixedRole::Custody => 4,
    }
}

#[cfg(feature = "families")]
fn has_active_role(
    effect: EffectProgramV3<'_>,
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
    effect: EffectProgramV3<'_>,
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
        | ResolvedEffectV3::WriteIdentity { account, .. } => [Some(account), None],
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
    const RESERVED_END: usize = 4;
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

#[cfg(feature = "families")]
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

#[cfg(feature = "families")]
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

#[cfg(feature = "families")]
fn claims_receipt_digest_v3(receipt: ClaimsRouteReceiptV3) -> Result<[u8; 32], ProgramError> {
    let bytes = match receipt {
        ClaimsRouteReceiptV3::Admit(value) => value
            .to_receipt_bytes()
            .map(Vec::from)
            .map_err(|_| TradingSbfError::Transition)?,
        ClaimsRouteReceiptV3::Affine(value) => Vec::from(value.to_bytes()),
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
}

impl<'a> RequestProfileKindV3<'a> {
    const fn v1(self) -> RequestProfileV1<'a> {
        match self {
            Self::Unsigned(profile) => profile,
            Self::Signed(profile) => profile.request_profile(),
        }
    }
}

fn decode_request_profile<'a>(
    descriptor: CapabilityProgramV3,
    bytes: &'a [u8],
) -> Result<RequestProfileKindV3<'a>, ProgramError> {
    let selected = descriptor.request_profile_program().to_bytes();
    let authenticated = hash(bytes).to_bytes();
    if descriptor.request_profile_schema().to_bytes() == REQUEST_PROFILE_SCHEMA_ID_V1 {
        RequestProfileV1::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::Unsigned)
            .map_err(|_| TradingSbfError::Content.into())
    } else if descriptor.request_profile_schema().to_bytes() == REQUEST_PROFILE_V2_SCHEMA_RELEASE_ID
    {
        RequestProfileV2::decode_selected(selected, authenticated, bytes)
            .map(RequestProfileKindV3::Signed)
            .map_err(|_| TradingSbfError::Content.into())
    } else {
        Err(TradingSbfError::UnsupportedContent.into())
    }
}

#[allow(clippy::too_many_arguments)]
fn require_geometry(
    account: AccountProfileV2<'_>,
    request: RequestProfileV1<'_>,
    transition: TransitionProgramV3<'_>,
    effect: EffectProgramV3<'_>,
    tail_count: u32,
    request_bytes: usize,
    runtime_accounts: usize,
) -> Result<(), ProgramError> {
    let expected_accounts = usize::from(account.fixed_account_count())
        .checked_add(
            usize::try_from(tail_count)
                .map_err(|_| TradingSbfError::Content)?
                .checked_mul(usize::from(account.item_account_stride()))
                .ok_or(TradingSbfError::Content)?,
        )
        .ok_or(TradingSbfError::Content)?;
    if request
        .request_bytes(tail_count)
        .map_err(|_| TradingSbfError::Content)?
        != request_bytes
        || expected_accounts != runtime_accounts
        || account.fixed_account_count() != effect.fixed_account_count()
        || account.item_account_stride() != effect.item_account_stride()
        || account.common_scalar_count() != request.common_scalar_count()
        || account.item_scalar_stride() != request.item_scalar_stride()
        || account.common_identity_count() != request.common_identity_count()
        || account.item_identity_stride() != request.item_identity_stride()
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

fn project_tail_count(
    profile: AccountProfileV2<'_>,
    observations: &[AccountObservationV1<'_>],
    request_digest: [u8; 32],
) -> Result<u32, ProgramError> {
    if profile
        .tail_count_projection()
        .map_err(|_| TradingSbfError::Content)?
        .is_none()
    {
        return Ok(0);
    }
    let fixed_count = usize::from(profile.fixed_account_count());
    let fixed = observations
        .get(..fixed_count)
        .ok_or(TradingSbfError::Content)?;
    let scalar_count = usize::from(profile.common_scalar_count());
    let identity_count = usize::from(profile.common_identity_count());
    if scalar_count > MAX_HOT_SCALARS_V3 || identity_count > MAX_HOT_IDENTITIES_V3 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let input_scalars = vec![0_u64; scalar_count];
    let mut input_identities = vec![[0_u8; 32]; identity_count];
    *input_identities
        .get_mut(HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3)
        .ok_or(TradingSbfError::Content)? = request_digest;
    let mut scratch_scalars = input_scalars.clone();
    let mut scratch_identities = input_identities.clone();
    let mut output_scalars = input_scalars.clone();
    let mut output_identities = input_identities.clone();
    project_tail_count_atomic(
        profile,
        fixed,
        ProjectionRegistersV2 {
            input_scalars: &input_scalars,
            input_identities: &input_identities,
            scratch_scalars: &mut scratch_scalars,
            scratch_identities: &mut scratch_identities,
            output_scalars: &mut output_scalars,
            output_identities: &mut output_identities,
        },
    )
    .map_err(|_| TradingSbfError::Content.into())
}

fn preflight_local_effects(
    effect: EffectProgramV3<'_>,
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
    effect: EffectProgramV3<'_>,
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
    descriptor: CapabilityProgramV3,
    context: TradingFamilyContextV1,
    entry: dclutch_capability_contract::CapabilityEntryV1,
) -> Result<(), ProgramError> {
    if descriptor.kind() != context.selection().kind()
        || descriptor.config_schema().to_bytes() == [0; 32]
        || descriptor.root_schema() != entry.child_schema_id()
        || descriptor.derivation_policy() != entry.child_derivation_id()
        || descriptor.capacity_profile() != entry.capacity_profile_id()
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
}

impl<'accounts, 'info> HotFrameV3<'accounts, 'info> {
    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        if accounts.len() < HOT_FIXED_ACCOUNT_COUNT_V3 {
            return Err(TradingSbfError::Content.into());
        }
        let value = Self {
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
        };
        if value.market.is_signer
            || value.market.is_writable
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
    use super::*;
    use dclutch_account_profile_contract::lifecycle_v3::CreateStatePlanV3;

    #[test]
    fn selector_is_exact_and_does_not_shadow_activation() {
        assert!(is_hot_execution_v3(b"DCLTHOT3"));
        assert!(!is_hot_execution_v3(b"DCLTHOT2"));
        assert!(!is_hot_execution_v3(b"DCLTHOT"));
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
        assert_eq!(accounts[1].lamports, 30);
        assert_eq!(accounts[1].data_len, 144);
        assert_eq!(accounts[3], accounts[1]);
        assert_eq!(accounts[2].lamports, 75);

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
    fn common_projection_bindings_and_child_reservations_are_exact() {
        let id = |tag: u8| [tag; 32];
        assert_eq!(
            require_common_projection_bindings_v3(id(1), id(1), id(2), id(2), id(3), id(3),),
            Ok(())
        );
        for hostile in [
            (id(4), id(1), id(2), id(2), id(3), id(3)),
            (id(1), id(1), id(5), id(2), id(3), id(3)),
            (id(1), id(1), id(2), id(2), id(6), id(3)),
        ] {
            assert_eq!(
                require_common_projection_bindings_v3(
                    hostile.0, hostile.1, hostile.2, hostile.3, hostile.4, hostile.5,
                ),
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
        };
        assert_eq!(
            require_no_common_projection_child_accounts_v3(invocation),
            Err(TradingSbfError::Content.into())
        );
        assert_eq!(
            require_no_common_projection_child_accounts_v3(
                dclutch_effect_kernel::v3::ResolvedInvocationV3 {
                    fixed_account_start: 4,
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
        let mut permissions = [AccountPermission::read_only(); 4];
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
}
