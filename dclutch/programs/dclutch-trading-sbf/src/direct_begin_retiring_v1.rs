//! Permissionless Direct root Open-to-Retiring transition.
//!
//! This is not Core `CloseCapability`: it does not CPI to Core, close a
//! FundingLedger, move lamports, or decrement outstanding capabilities. It
//! authenticates an already-`Retiring` canonical Core Market, the current Core
//! and Trading deployments, the root-selected ProgramSet/config/manifest, and
//! the selected begin-retiring descriptor/profile/effect before committing the
//! sole 24-byte Direct-tail transition.

extern crate alloc;

use alloc::vec;

use dclutch_account_profile_contract::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, AccountObservationV1, AccountProfileV1,
    ProjectionRegistersV2, derive_effect_permissions, project_atomic,
};
use dclutch_capability_contract::funding::funded_rent_persists_v1;
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    CapabilityRegistersV2, CapabilityRootHeaderV1,
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
};
use dclutch_direct_codec::{
    retirement_v1,
    retirement_v1::{
        DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1, DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_ID_V1,
        DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1, DIRECT_BEGIN_RETIRING_ROOT_IDENTITY_V1,
        DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1, DIRECT_BEGIN_RETIRING_SELECTOR_SCALAR_V1,
        DIRECT_BEGIN_RETIRING_SELECTOR_V1, DIRECT_BEGIN_RETIRING_TRADING_IDENTITY_V1,
        DirectBeginRetiringReceiptV1, DirectBeginRetiringRequestV1,
        direct_begin_retiring_context_v1,
    },
    successor::{
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, DIRECT_ROOT_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1,
        DirectRootStateLayoutV1, DirectRootStateV1,
    },
};
use dclutch_effect_kernel::v2::{
    AccountInput, AccountPermission, ProgramV2 as EffectProgramV2, ResolvedEffect,
    SCHEMA_RELEASE_ID as EFFECT_SCHEMA_RELEASE_ID_V2, project_with_aliases_and_requests_atomic,
};
use dclutch_market_core_codec::{CoreState, MarketCoreStateSeedsV2, STATE_BYTES};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_activation_auth_v1::{
    authenticate_activated_role_in_frame_v1, authenticate_activation_cache_identity_v1,
    require_cache_account,
};
use dclutch_registry_contract::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry_svm::AuthenticatedRoleReceiptV1;
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_transition_vm::v2::{RegisterInput, RegisterOutput};
use solana_program::{
    account_info::AccountInfo, hash::hash, program::set_return_data, program_error::ProgramError,
    pubkey::Pubkey,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{TradingSbfError, dispatch::TradingFamilyContextV1};

use crate::market_admission_v1::TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1;
pub use dclutch_direct_codec::retirement_v1::DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1;

struct Accounts<'accounts, 'info> {
    root: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    manifest_raw: &'accounts AccountInfo<'info>,
    program_set_raw: &'accounts AccountInfo<'info>,
    program_set_staging: &'accounts AccountInfo<'info>,
    descriptor_raw: &'accounts AccountInfo<'info>,
    descriptor_staging: &'accounts AccountInfo<'info>,
    config_raw: &'accounts AccountInfo<'info>,
    config_staging: &'accounts AccountInfo<'info>,
    profile_raw: &'accounts AccountInfo<'info>,
    profile_staging: &'accounts AccountInfo<'info>,
    effect_raw: &'accounts AccountInfo<'info>,
    effect_staging: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    trading_program: &'accounts AccountInfo<'info>,
    trading_programdata: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> Accounts<'accounts, 'info> {
    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        if accounts.len() != DIRECT_BEGIN_RETIRING_ACCOUNT_COUNT_V1
            || accounts.iter().any(|account| account.is_signer)
        {
            return Err(TradingSbfError::Content.into());
        }
        for (index, account) in accounts.iter().enumerate() {
            let (expected_writable, expected_executable) =
                retirement_v1::direct_begin_retiring_account_privileges_v1(index)
                    .ok_or(TradingSbfError::Content)?;
            if account.is_writable != expected_writable || account.executable != expected_executable
            {
                return Err(TradingSbfError::Content.into());
            }
            if accounts
                .get(index.saturating_add(1)..)
                .is_some_and(|suffix| suffix.iter().any(|other| other.key == account.key))
            {
                return Err(TradingSbfError::Content.into());
            }
        }
        let value = Self {
            root: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_ROOT_TOP_ACCOUNT_V1,
            )?,
            market: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_MARKET_ACCOUNT_V1,
            )?,
            manifest_raw: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_MANIFEST_RAW_ACCOUNT_V1,
            )?,
            program_set_raw: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_PROGRAM_SET_RAW_ACCOUNT_V1,
            )?,
            program_set_staging: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_PROGRAM_SET_STAGING_ACCOUNT_V1,
            )?,
            descriptor_raw: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_DESCRIPTOR_RAW_ACCOUNT_V1,
            )?,
            descriptor_staging: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_DESCRIPTOR_STAGING_ACCOUNT_V1,
            )?,
            config_raw: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_CONFIG_RAW_ACCOUNT_V1,
            )?,
            config_staging: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_CONFIG_STAGING_ACCOUNT_V1,
            )?,
            profile_raw: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_PROFILE_RAW_ACCOUNT_V1,
            )?,
            profile_staging: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_PROFILE_STAGING_ACCOUNT_V1,
            )?,
            effect_raw: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_EFFECT_RAW_ACCOUNT_V1,
            )?,
            effect_staging: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_EFFECT_STAGING_ACCOUNT_V1,
            )?,
            cache: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_ACTIVATION_CACHE_ACCOUNT_V1,
            )?,
            core_program: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_CORE_PROGRAM_ACCOUNT_V1,
            )?,
            core_programdata: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_CORE_PROGRAMDATA_ACCOUNT_V1,
            )?,
            trading_program: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_TRADING_PROGRAM_ACCOUNT_V1,
            )?,
            trading_programdata: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_TRADING_PROGRAMDATA_ACCOUNT_V1,
            )?,
            registry: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_REGISTRY_ACCOUNT_V1,
            )?,
            rent: get(
                accounts,
                retirement_v1::DIRECT_BEGIN_RETIRING_RENT_ACCOUNT_V1,
            )?,
        };
        if value.trading_program.key != program_id
            || value.rent.key != &sysvar::rent::ID
            || value.cache.owner != value.registry.key
        {
            return Err(TradingSbfError::Content.into());
        }
        Ok(value)
    }
}

/// Execute one exact permissionless begin-retiring request.
#[inline(never)]
pub fn process_direct_begin_retiring_v1(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let request = DirectBeginRetiringRequestV1::decode(instruction_data)
        .map_err(|_| TradingSbfError::Content)?;
    let accounts = Accounts::parse(program_id, account_infos)?;
    let trading_receipt = reauthenticate_roles(&accounts, request.release_set)?;
    authenticate_market(&accounts, request)?;

    let root_data = accounts
        .root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    if hash(&root_data).to_bytes() != request.expected_root_digest
        || root_data.len() != CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1
    {
        return Err(TradingSbfError::Root.into());
    }
    let context = TradingFamilyContextV1::authenticate(
        program_id,
        accounts.root.key,
        accounts.root.owner,
        &root_data,
        trading_receipt,
    )?;
    let selection = context.selection();
    if context.market() != request.market
        || context.generation() != request.generation
        || context.release_set().to_bytes() != request.release_set
        || context.child_root_key() != request.root
        || selection.entry_index() != request.entry_index
        || selection.manifest().to_bytes() != request.manifest
        || selection.capability_release().to_bytes() != request.program_set
        || selection.config().to_bytes() != request.config
        || request.context
            != direct_begin_retiring_context_v1(
                request.release_set,
                request.market,
                request.root,
                request.manifest,
                request.program_set,
                request.config,
                request.generation,
                request.entry_index,
            )
    {
        return Err(TradingSbfError::Content.into());
    }
    let header = CapabilityRootHeaderV1::decode(
        root_data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(TradingSbfError::Root)?,
    )
    .map_err(|_| TradingSbfError::Root)?;
    let direct_post = prepare_retiring_tail(
        root_data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(TradingSbfError::Root)?,
    )?;

    let manifest_data = accounts
        .manifest_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_persisted_raw(
        accounts.registry.key,
        accounts.manifest_raw,
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        request.manifest,
        header.record_bumps().manifest_raw(),
        &manifest_data,
    )?;
    let program_set_data = accounts
        .program_set_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.program_set_raw,
        accounts.program_set_staging,
        CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        request.program_set,
        &program_set_data,
    )?;
    let set = CapabilityProgramSetV2::decode_selected(
        request.program_set,
        request.program_set,
        &program_set_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let selected = set
        .select_descriptor(instruction_data)
        .map_err(|_| TradingSbfError::Content)?;
    if selected.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let descriptor_data = accounts
        .descriptor_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.descriptor_raw,
        accounts.descriptor_staging,
        selected.schema().to_bytes(),
        selected.program().to_bytes(),
        &descriptor_data,
    )?;
    let config_data = accounts
        .config_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.config_raw,
        accounts.config_staging,
        DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
        request.config,
        &config_data,
    )?;
    let descriptor = crate::dispatch::authenticate_activation_program(
        context,
        selected.program(),
        &manifest_data,
        &descriptor_data,
        &config_data,
    )?;
    if descriptor.request_schema().to_bytes() != DIRECT_BEGIN_RETIRING_REQUEST_SCHEMA_ID_V1
        || descriptor.root_schema().to_bytes() != DIRECT_ROOT_SCHEMA_ID_V1
        || descriptor.config_schema().to_bytes() != DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1
        || usize::try_from(descriptor.root_state_bytes()).ok() != Some(DIRECT_ROOT_STATE_BYTES_V1)
    {
        return Err(TradingSbfError::Content.into());
    }

    let profile_data = accounts
        .profile_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.profile_raw,
        accounts.profile_staging,
        ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1,
        descriptor.account_profile().to_bytes(),
        &profile_data,
    )?;
    let profile = AccountProfileV1::decode_selected(
        descriptor.account_profile().to_bytes(),
        hash(&profile_data).to_bytes(),
        &profile_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let effect_data = accounts
        .effect_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        accounts.registry.key,
        accounts.effect_raw,
        accounts.effect_staging,
        EFFECT_SCHEMA_RELEASE_ID_V2,
        descriptor.effect_schema().to_bytes(),
        &effect_data,
    )?;
    let effect = EffectProgramV2::decode_selected(
        descriptor.effect_schema().to_bytes(),
        hash(&effect_data).to_bytes(),
        &effect_data,
    )
    .map_err(|_| TradingSbfError::Content)?;

    authenticate_artifact_transition(
        program_id,
        accounts.root,
        &root_data,
        profile,
        descriptor,
        effect,
        direct_post,
    )?;
    let mut post_root = root_data.to_vec();
    post_root
        .get_mut(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(TradingSbfError::Root)?
        .copy_from_slice(&direct_post);
    let post_root_digest = hash(&post_root).to_bytes();
    let request_digest = hash(instruction_data).to_bytes();
    let receipt = DirectBeginRetiringReceiptV1::new(
        request,
        request_digest,
        post_root_digest,
        program_id.to_bytes(),
    )
    .and_then(DirectBeginRetiringReceiptV1::to_bytes)
    .map_err(|_| TradingSbfError::Content)?;
    drop(effect_data);
    drop(profile_data);
    drop(config_data);
    drop(descriptor_data);
    drop(program_set_data);
    drop(manifest_data);
    drop(root_data);

    let mut root_commit = accounts
        .root
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if hash(&root_commit).to_bytes() != request.expected_root_digest
        || root_commit.len() != post_root.len()
    {
        return Err(TradingSbfError::Commit.into());
    }
    root_commit.copy_from_slice(&post_root);
    drop(root_commit);
    set_return_data(&receipt);
    Ok(())
}

/// Hold the Market to the request, out of line, in a frame of its own.
///
/// # Why the attribute, and why it is the thing that fixed the wall
///
/// This decodes a `CoreState` and RE-ENCODES it to compare byte for byte, so it
/// carries the widest stack object on the route: 2,304 bytes of the 4,096 an
/// SBPF v0 frame gets. Its caller carries a borrowed root, a context and a
/// request of its own, and the two do not fit together.
///
/// LLVM kept them apart on its own until the activation-cache conversion, then
/// stopped. The sequence is worth writing down because the second half is the
/// counter-intuitive one:
///
/// ```text
///   f6596ffb   caller 3,712   authenticate_market 2,304 out of line, reauthenticate_role 576
///   converted  caller 4,096   the two-role read inlined into the caller     -- 43 diagnostics
///   +inline(never) on the read
///              caller 4,352   the read left, and authenticate_market CAME IN -- 48 diagnostics
///   +inline(never) here
///              caller 3,392   both out of line
/// ```
///
/// Splitting one frame made the caller look cheaper to the inliner, so it
/// swallowed a 2,304-byte callee it had previously declined and the number got
/// WORSE. Nothing about either function changed; the heuristic simply re-scored
/// a body it was now measuring differently. A frame that has to stay split must
/// say so, because the inliner is not a party to the constraint.
#[inline(never)]
fn authenticate_market(
    accounts: &Accounts<'_, '_>,
    request: DirectBeginRetiringRequestV1,
) -> Result<(), ProgramError> {
    let data = accounts
        .market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_market_bytes(
        accounts.market.key,
        accounts.market.owner,
        accounts.core_program.key,
        accounts.registry.key,
        &data,
        request,
    )
}

fn authenticate_market_bytes(
    market_key: &Pubkey,
    market_owner: &Pubkey,
    core_program: &Pubkey,
    registry: &Pubkey,
    data: &[u8],
    request: DirectBeginRetiringRequestV1,
) -> Result<(), ProgramError> {
    if market_owner != core_program
        || market_key.to_bytes() != request.market
        || data.len() != STATE_BYTES
    {
        return Err(TradingSbfError::Content.into());
    }
    let state = CoreState::decode(data).map_err(|_| TradingSbfError::Content)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        core_program,
    )
    .0;
    if expected != *market_key
        || state
            .encode()
            .map_err(|_| TradingSbfError::Content)?
            .as_slice()
            != data
        || !TRADING_RETIRING_MARKET_ADMISSIBLE_PRESTATES_V1.admits_phase(state.phase)
        || state.identity.market_id.to_bytes() != request.market
        || state.identity.selected_release_set.to_bytes() != request.release_set
        || state.identity.registry_program.to_bytes() != registry.to_bytes()
        || state.identity.generation != request.generation
        || hash(data).to_bytes() != request.expected_market_digest
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn prepare_retiring_tail(tail: &[u8]) -> Result<[u8; DIRECT_ROOT_STATE_BYTES_V1], ProgramError> {
    // Deliberately NO open-maker-root-count gate (cohort-9 review item 1,
    // amendment 1): retirement begins over standing maker roots, which wind
    // down INSIDE Retiring -- `consume_nonce_v2` refuses every non-Open phase,
    // and the count gate that protects Retired lives at both physical-close
    // sites. Gating count here made `close_maker_replay_v2` unreachable for
    // every filled market (wall 22).
    let pre = DirectRootStateV1::decode(tail).map_err(|_| TradingSbfError::Root)?;
    pre.begin_retiring()
        .map(DirectRootStateV1::encode)
        .map_err(|_| TradingSbfError::Root.into())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_artifact_transition(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    root_data: &[u8],
    profile: AccountProfileV1<'_>,
    descriptor: CapabilityProgramV1<'_>,
    effect: EffectProgramV2<'_>,
    expected_tail: [u8; DIRECT_ROOT_STATE_BYTES_V1],
) -> Result<(), ProgramError> {
    if profile.account_count() != 1
        || profile.scalar_count() != DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1
        || profile.identity_count() != DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1
        || effect.account_count() != 1
        || effect.scalar_count() != DIRECT_BEGIN_RETIRING_SCALAR_COUNT_V1
        || effect.identity_count() != DIRECT_BEGIN_RETIRING_IDENTITY_COUNT_V1
        || effect.request_bytes() != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    let mut input_scalars = vec![0_u64; usize::from(profile.scalar_count())];
    let mut input_identities = vec![[0_u8; 32]; usize::from(profile.identity_count())];
    *input_scalars
        .get_mut(usize::from(DIRECT_BEGIN_RETIRING_SELECTOR_SCALAR_V1))
        .ok_or(TradingSbfError::Content)? = u64::from(DIRECT_BEGIN_RETIRING_SELECTOR_V1);
    *input_identities
        .get_mut(usize::from(DIRECT_BEGIN_RETIRING_TRADING_IDENTITY_V1))
        .ok_or(TradingSbfError::Content)? = program_id.to_bytes();
    *input_identities
        .get_mut(usize::from(DIRECT_BEGIN_RETIRING_ROOT_IDENTITY_V1))
        .ok_or(TradingSbfError::Content)? = root.key.to_bytes();
    let observation = [AccountObservationV1::new(
        root.key.as_array(),
        root.owner.as_array(),
        root.lamports(),
        root_data,
        root.is_signer,
        root.is_writable,
        root.executable,
    )];
    let mut projection_scratch_scalars = input_scalars.clone();
    let mut projection_scratch_identities = input_identities.clone();
    let mut projected_scalars = input_scalars.clone();
    let mut projected_identities = input_identities.clone();
    project_atomic(
        profile,
        &observation,
        ProjectionRegistersV2::new(
            RegisterInput {
                scalars: &input_scalars,
                identities: &input_identities,
            },
            RegisterOutput {
                scalars: &mut projection_scratch_scalars,
                identities: &mut projection_scratch_identities,
            },
            RegisterOutput {
                scalars: &mut projected_scalars,
                identities: &mut projected_identities,
            },
        ),
    )
    .map_err(|_| TradingSbfError::Content)?;
    let mut transition_scratch_scalars = projected_scalars.clone();
    let mut transition_scratch_identities = projected_identities.clone();
    let mut transition_output_scalars = projected_scalars.clone();
    let mut transition_output_identities = projected_identities.clone();
    descriptor
        .execute(CapabilityRegistersV2::new(
            RegisterInput {
                scalars: &projected_scalars,
                identities: &projected_identities,
            },
            RegisterOutput {
                scalars: &mut transition_scratch_scalars,
                identities: &mut transition_scratch_identities,
            },
            RegisterOutput {
                scalars: &mut transition_output_scalars,
                identities: &mut transition_output_identities,
            },
        ))
        .map_err(|_| TradingSbfError::Transition)?;
    let account_inputs = [AccountInput {
        lamports: root.lamports(),
        data_len: root_data.len(),
    }];
    let mut permissions = [AccountPermission::read_only()];
    derive_effect_permissions(profile, &mut permissions).map_err(|_| TradingSbfError::Content)?;
    let aliases = [profile
        .rule(0)
        .map_err(|_| TradingSbfError::Content)?
        .alias_of()];
    let mut scratch_lamports = [0_u64];
    let mut output_lamports = [0_u64];
    let mut scratch_request = [];
    let mut output_request = [];
    project_with_aliases_and_requests_atomic(
        effect,
        &transition_output_scalars,
        &transition_output_identities,
        &aliases,
        &account_inputs,
        &permissions,
        &mut scratch_lamports,
        &mut output_lamports,
        &mut scratch_request,
        &mut output_request,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if output_lamports != [root.lamports()] || effect.instruction_count() != 2 {
        return Err(TradingSbfError::Content.into());
    }
    let expected_header = u64::from_le_bytes(
        expected_tail
            .get(DirectRootStateLayoutV1::VERSION..DirectRootStateLayoutV1::VERSION + 8)
            .ok_or(TradingSbfError::Content)?
            .try_into()
            .map_err(|_| TradingSbfError::Content)?,
    );
    match (
        effect
            .resolved_effect(0, &transition_output_scalars, &transition_output_identities)
            .map_err(|_| TradingSbfError::Content)?,
        effect
            .resolved_effect(1, &transition_output_scalars, &transition_output_identities)
            .map_err(|_| TradingSbfError::Content)?,
    ) {
        (
            ResolvedEffect::WriteScalar {
                account: DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
                offset,
                value,
            },
            ResolvedEffect::RequireLamportsEq {
                account: DIRECT_BEGIN_RETIRING_ROOT_ACCOUNT_V1,
                value: lamports,
            },
        ) if usize::try_from(offset).ok()
            == Some(CAPABILITY_ROOT_HEADER_BYTES_V1 + DirectRootStateLayoutV1::VERSION)
            && value == expected_header
            && lamports == root.lamports() => {}
        _ => return Err(TradingSbfError::UnsupportedContent.into()),
    }
    Ok(())
}

/// Authenticate Core and Trading for the request's release set, from ONE read
/// of the Registry-owned activation cache.
///
/// Decision 0017's option B. This route paid two
/// `RegistryInstructionV1::Reauthenticate` CPIs -- 26,296 CU each, SEALWIDE's
/// measurement, invariant across keys and builds -- for two facts written in a
/// Registry-OWNED account at a Registry-DERIVED address that `Accounts::parse`
/// already required this frame to carry. `outer.rs::reauthenticate_roles` states
/// the conjunction and where each half of it comes from; this is the same
/// function set over a different account frame.
///
/// # The release set here is CALLER-NAMED, and that is unchanged
///
/// `request.release_set` is instruction data. It always was: the CPI derived the
/// cache address from it too. What binds it is `authenticate_market_bytes`, which
/// refuses unless `state.identity.selected_release_set` equals it -- so a caller
/// who names another Market's generation reaches a cache that is real and then
/// fails to join it to the Market it is retiring. The call order below is the
/// order this route already had, and this change does not move it.
///
/// # `inline(never)` is the frame, and it is load-bearing
///
/// SBPF v0 gives every call frame exactly 4,096 bytes and does not grow one: a
/// function whose locals plus outgoing arguments exceed it gets a diagnostic and
/// a call that writes over its own locals. This function holds a `Ref` over the
/// cache, a decoded view into it and two receipts, and it measures 576 bytes.
///
/// While the CPI form existed there were TWO call sites here, so LLVM kept it
/// out of line for its own reasons and the frames never met. Folding the two
/// roles into one read left ONE call site, LLVM inlined it, and those bytes
/// landed on a caller that had exactly 384 spare:
/// `process_direct_begin_retiring_v1` went 3,712 -> 4,096 of 4,096 and the link
/// emitted 43 frame-overwrite diagnostics. Measured both ways with
/// `tools/sbf-frame-sizes.py`.
///
/// So the split is not a hint here, it is the frame. The inliner's heuristic was
/// the only thing holding these apart, and it stopped applying the moment the
/// call count changed -- which is exactly the kind of thing that must not be
/// left to a heuristic on a route whose caller has three digits of headroom.
#[inline(never)]
fn reauthenticate_roles<'info>(
    accounts: &Accounts<'_, 'info>,
    release_set: [u8; 32],
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    require_cache_account(accounts.registry.key, accounts.cache).map_err(TradingSbfError::from)?;
    let data = accounts
        .cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| TradingSbfError::Release)?;
    authenticate_activation_cache_identity_v1(
        accounts.registry,
        accounts.cache,
        &release_set,
        activated,
    )
    .map_err(TradingSbfError::from)?;
    let core_receipt = authenticate_activated_role_in_frame_v1(
        accounts.cache,
        activated,
        ExecutionRoleV1::Core,
        accounts.core_program,
        accounts.core_programdata,
    )
    .map_err(TradingSbfError::from)?;
    if core_receipt.program().to_bytes() != accounts.core_program.key.to_bytes() {
        return Err(TradingSbfError::Release.into());
    }
    authenticate_activated_role_in_frame_v1(
        accounts.cache,
        activated,
        ExecutionRoleV1::Trading,
        accounts.trading_program,
        accounts.trading_programdata,
    )
    .map_err(|error| TradingSbfError::from(error).into())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_finalized_record(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], registry).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], registry).0;
    if raw.key != &expected_raw
        || raw.owner != registry
        || hash(bytes).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn authenticate_persisted_raw(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
    bump: u8,
    bytes: &[u8],
) -> Result<(), ProgramError> {
    let bump = [bump];
    let expected = Pubkey::create_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest, &bump],
        registry,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if raw.key != &expected
        || raw.owner != registry
        || hash(bytes).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn get<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts.get(index).ok_or(TradingSbfError::Content.into())
}

#[cfg(test)]
mod tests {
    use dclutch_direct_codec::{
        retirement_v1::direct_begin_retiring_context_v1,
        successor::{DirectRootPhaseV1, DirectRootStateLayoutV1},
    };
    use dclutch_market_core_codec::Phase;
    use dclutch_market_core_codec::{Identity, MarketIdentity, Readiness};

    use super::*;
    use dclutch_market_core_codec::StateBumpsV1;

    fn identity(byte: u8) -> Identity {
        Identity::new([byte; 32]).expect("identity")
    }

    fn market_fixture(
        phase: Phase,
        core_program: Pubkey,
        registry: Pubkey,
    ) -> (Pubkey, [u8; STATE_BYTES], DirectBeginRetiringRequestV1) {
        let mut market_identity = MarketIdentity {
            market_id: identity(1),
            realm_id: identity(2),
            product_record: identity(3),
            product_id: identity(4),
            resolution_policy: identity(5),
            capability_manifest: identity(6),
            selected_release_set: identity(7),
            registry_program: Identity::new(registry.to_bytes()).expect("registry"),
            generation: 8,
        };
        let market = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
            &core_program,
        )
        .0;
        market_identity.market_id = Identity::new(market.to_bytes()).expect("market");
        let state = CoreState {
            phase,
            readiness: Readiness::Consumed,
            terminal_winner: if phase == Phase::Open { 0 } else { 1 },
            identity: market_identity,
            outstanding_capabilities: 1,
            principal_cap_sets: 10,
            rent_beneficiary: identity(9),
            terminal_receipt: if phase == Phase::Open {
                None
            } else {
                Some(identity(10))
            },
            bumps: StateBumpsV1::UNRECORDED,
        };
        let bytes = state.encode().expect("market bytes");
        let root = [11; 32];
        let manifest = [12; 32];
        let program_set = [13; 32];
        let config = [14; 32];
        let release_set = market_identity.selected_release_set.to_bytes();
        let context = direct_begin_retiring_context_v1(
            release_set,
            market.to_bytes(),
            root,
            manifest,
            program_set,
            config,
            market_identity.generation,
            3,
        );
        let request = DirectBeginRetiringRequestV1 {
            release_set,
            market: market.to_bytes(),
            context,
            root,
            manifest,
            program_set,
            config,
            expected_market_digest: hash(&bytes).to_bytes(),
            expected_root_digest: [15; 32],
            generation: market_identity.generation,
            entry_index: 3,
        }
        .new()
        .expect("request");
        (market, bytes, request)
    }

    #[test]
    fn only_canonical_retiring_market_authenticates() {
        let core = Pubkey::new_from_array([21; 32]);
        let registry = Pubkey::new_from_array([22; 32]);
        let (market, bytes, request) = market_fixture(Phase::Retiring, core, registry);
        assert_eq!(
            authenticate_market_bytes(&market, &core, &core, &registry, &bytes, request),
            Ok(())
        );

        for phase in [Phase::Open, Phase::Terminal] {
            let (hostile_market, hostile_bytes, hostile_request) =
                market_fixture(phase, core, registry);
            assert!(
                authenticate_market_bytes(
                    &hostile_market,
                    &core,
                    &core,
                    &registry,
                    &hostile_bytes,
                    hostile_request,
                )
                .is_err()
            );
        }
        let wrong = Pubkey::new_from_array([23; 32]);
        assert!(
            authenticate_market_bytes(&market, &wrong, &core, &registry, &bytes, request).is_err()
        );
        assert!(authenticate_market_bytes(&market, &core, &core, &wrong, &bytes, request).is_err());
        assert!(
            authenticate_market_bytes(&wrong, &core, &core, &registry, &bytes, request).is_err()
        );
    }

    #[test]
    fn market_digest_and_one_byte_state_drift_refuse() {
        let core = Pubkey::new_from_array([31; 32]);
        let registry = Pubkey::new_from_array([32; 32]);
        let (market, bytes, request) = market_fixture(Phase::Retiring, core, registry);
        let mut wrong_digest = request;
        wrong_digest.expected_market_digest[0] ^= 1;
        assert!(
            authenticate_market_bytes(&market, &core, &core, &registry, &bytes, wrong_digest,)
                .is_err()
        );
        for offset in [0, STATE_BYTES - 1] {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("hostile Market byte") ^= 1;
            assert!(
                authenticate_market_bytes(&market, &core, &core, &registry, &hostile, request,)
                    .is_err()
            );
        }
    }

    #[test]
    fn root_transition_is_exact_and_replay_has_no_drift() {
        let open = DirectRootStateV1::new().encode();
        let post = prepare_retiring_tail(&open).expect("retiring poststate");
        let decoded = DirectRootStateV1::decode(&post).expect("poststate");
        assert_eq!(decoded.phase(), DirectRootPhaseV1::Retiring);
        assert_eq!(decoded.open_maker_root_count(), 0);
        let replay_preimage = post;
        assert!(prepare_retiring_tail(&replay_preimage).is_err());
        assert_eq!(post, replay_preimage);

        // The intentional flip (cohort-9 review item 1, amendment 1): a
        // standing maker root no longer blocks begin-retiring. The transition
        // preserves the count exactly and moves only the phase; the count
        // drains inside Retiring via the maker-replay close.
        let mut maker_live = open;
        maker_live
            .get_mut(
                DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT
                    ..DirectRootStateLayoutV1::OPEN_MAKER_ROOT_COUNT + 8,
            )
            .expect("maker count word")
            .copy_from_slice(&1_u64.to_le_bytes());
        let over_makers = prepare_retiring_tail(&maker_live).expect("retiring over makers");
        let decoded_over_makers = DirectRootStateV1::decode(&over_makers).expect("poststate");
        assert_eq!(decoded_over_makers.phase(), DirectRootPhaseV1::Retiring);
        assert_eq!(decoded_over_makers.open_maker_root_count(), 1);
        let mut hostile_reserved = open;
        *hostile_reserved
            .get_mut(DirectRootStateLayoutV1::RESERVED)
            .expect("reserved byte") = 1;
        assert!(prepare_retiring_tail(&hostile_reserved).is_err());
    }
}
