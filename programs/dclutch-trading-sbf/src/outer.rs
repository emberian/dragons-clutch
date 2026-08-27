//! Executable, family-neutral Core-to-Trading activation boundary.
//!
//! The outer authenticates the current Core and Trading deployments, four
//! finalized content records, and an interpreted account/effect profile. It
//! does not dispatch on a capability kind. All physical mutation is projected
//! first and committed only after the complete activation plan accepts.
//!
//! The created root is `CapabilityRootHeaderV1 || <family tail>`. The outer owns
//! the header and never decodes the tail; the tail is exactly the effect
//! program's projected request buffer, whose width the descriptor pins to
//! `root_state_bytes`. That keeps the family's initial state an artifact the
//! family authors and the Market's manifest entry binds, with no family decoder
//! and no kind branch on this path.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_account_profile_contract::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, AccountObservationV1, AccountProfileV1,
    EFFECT_PERMISSION_WRITE_DATA, ProjectionRegistersV2, derive_effect_permissions,
    project_atomic as project_accounts_atomic,
};
use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId, FUNDING_STATE_BYTES,
    FundingCustodyObservationV1, FundingStateV1,
};
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CapabilityProgramV1, CapabilityRegistersV2,
    CapabilityRootAccountV1, CapabilityRootHeaderV1, initialize_root_account_v1,
};
use dclutch_effect_kernel::v2::{
    AccountInput, ProgramV2 as EffectProgramV2, ResolvedEffect, SCHEMA_RELEASE_ID,
    project_with_aliases_and_requests_atomic,
};
use dclutch_market_core_codec::{
    CORE_EFFECT_ACK_BYTES_V1, CORE_EFFECT_DIGEST_DOMAIN_V1, CORE_EFFECT_ENVELOPE_BYTES_V1,
    CoreEffectAckV1, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState, Identity,
    MarketCoreStateSeedsV2, Role, STATE_BYTES,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::ACTIVATION_PDA_DOMAIN_V1;
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::ExecutionRoleV1;
use dclutch_transition_vm::v2::{RegisterInput, RegisterOutput};
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
use solana_system_interface::instruction::{allocate, assign};

use crate::{
    TradingSbfError,
    dispatch::{
        TradingActivationAccountsV1, TradingActivationRequestV1, TradingFamilyContextV1,
        authenticate_activation_program,
    },
};

const DESCRIPTOR_RAW: usize = 0;
const DESCRIPTOR_STAGING: usize = 1;
const CONFIG_RAW: usize = 2;
const CONFIG_STAGING: usize = 3;
const PROFILE_RAW: usize = 4;
const PROFILE_STAGING: usize = 5;
const EFFECT_RAW: usize = 6;
const EFFECT_STAGING: usize = 7;
const ACTIVATION_CACHE: usize = 8;
const CORE_PROGRAM: usize = 9;
const CORE_PROGRAMDATA: usize = 10;
const TRADING_PROGRAM: usize = 11;
const TRADING_PROGRAMDATA: usize = 12;
const REGISTRY_PROGRAM: usize = 13;
const RENT_SYSVAR: usize = 14;
const SYSTEM_PROGRAM: usize = 15;
const EFFECT_ACCOUNTS_START: usize = 16;

const COMMON_SCALARS_V2: usize = 8;
const COMMON_IDENTITIES_V2: usize = 12;
const MAX_RUNTIME_SCALARS_V2: usize = 96;
const MAX_RUNTIME_IDENTITIES_V2: usize = 32;
const MAX_RUNTIME_ACCOUNTS_V2: usize = 64;
const MAX_ROLE_REQUEST_BYTES_V2: usize = 2_048;

/// Execute one Core-signed, data-defined capability activation.
#[inline(never)]
pub fn process_activation(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let (envelope_bytes, role_request_bytes) = instruction_data
        .split_at_checked(CORE_EFFECT_ENVELOPE_BYTES_V1)
        .ok_or(TradingSbfError::Content)?;
    let envelope =
        CoreEffectEnvelopeV1::decode(envelope_bytes).map_err(|_| TradingSbfError::Content)?;
    if envelope.action() != CoreEffectActionV1::ActivateCapability
        || envelope.target_role() != Role::Trading
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let request = TradingActivationRequestV1::decode(role_request_bytes)?;
    envelope
        .validate_role_request(
            role_request_bytes.len(),
            identity(hash(role_request_bytes).to_bytes())?,
        )
        .map_err(|_| TradingSbfError::Content)?;
    let framed = TradingActivationAccountsV1::parse(accounts, request.funding())?;
    let suffix = AuthenticatedSuffixV2::parse(program_id, framed.family_accounts())?;
    let market_state = authenticate_market_and_caller(program_id, &framed, &suffix, envelope)?;
    let rent = Rent::from_account_info(suffix.rent).map_err(|_| TradingSbfError::Content)?;

    let descriptor_data = suffix
        .descriptor_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        suffix.registry.key,
        suffix.descriptor_raw,
        suffix.descriptor_staging,
        &rent,
        CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
        request.selection().capability_release().to_bytes(),
        &descriptor_data,
    )?;
    let descriptor =
        CapabilityProgramV1::decode(&descriptor_data).map_err(|_| TradingSbfError::Content)?;
    let root_bytes = descriptor
        .root_account_bytes()
        .map_err(|_| TradingSbfError::Root)?;
    let root_header = CapabilityRootHeaderV1::new(
        content(envelope.release_set().to_bytes())?,
        envelope.market().to_bytes(),
        envelope.generation(),
        request.selection(),
    )
    .map_err(|_| TradingSbfError::Root)?;
    authenticate_vacant_root(program_id, framed.child_root(), root_header, root_bytes)?;

    let core_receipt = reauthenticate_role(
        &suffix,
        ExecutionRoleV1::Core,
        suffix.core_program,
        suffix.core_programdata,
        market_state.identity.selected_release_set.to_bytes(),
    )?;
    if core_receipt.program().to_bytes() != suffix.core_program.key.to_bytes() {
        return Err(TradingSbfError::Release.into());
    }
    let trading_receipt = reauthenticate_role(
        &suffix,
        ExecutionRoleV1::Trading,
        suffix.trading_program,
        suffix.trading_programdata,
        market_state.identity.selected_release_set.to_bytes(),
    )?;
    let context = TradingFamilyContextV1::authenticate_activation(
        program_id,
        framed.child_root().key,
        root_header,
        root_bytes,
        trading_receipt,
    )
    .inspect_err(|_| {
        solana_program::msg!("Trading activation: authenticated root/receipt join refused");
    })?;

    let manifest_data = framed
        .manifest()
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let config_data = suffix
        .config_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        suffix.registry.key,
        suffix.config_raw,
        suffix.config_staging,
        &rent,
        descriptor.config_schema().to_bytes(),
        request.selection().config().to_bytes(),
        &config_data,
    )?;
    let descriptor =
        authenticate_activation_program(context, &manifest_data, &descriptor_data, &config_data)?;

    let profile_data = suffix
        .profile_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        suffix.registry.key,
        suffix.profile_raw,
        suffix.profile_staging,
        &rent,
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

    let effect_data = suffix
        .effect_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        suffix.registry.key,
        suffix.effect_raw,
        suffix.effect_staging,
        &rent,
        SCHEMA_RELEASE_ID,
        descriptor.effect_schema().to_bytes(),
        &effect_data,
    )?;
    let effect = EffectProgramV2::decode_selected(
        descriptor.effect_schema().to_bytes(),
        hash(&effect_data).to_bytes(),
        &effect_data,
    )
    .map_err(|_| TradingSbfError::Content)?;

    let runtime = RuntimeFrameV2::new(&framed, suffix.effect_accounts)?;
    let mut input_scalars = vec![0_u64; usize::from(profile.scalar_count())];
    let mut input_identities = vec![[0_u8; 32]; usize::from(profile.identity_count())];
    seed_common_registers(
        &mut input_scalars,
        &mut input_identities,
        program_id,
        &suffix,
        envelope,
        request,
        descriptor,
        framed.child_root().key,
    )?;
    let mut projected_scalars = input_scalars.clone();
    let mut projected_identities = input_identities.clone();
    let mut projection_scratch_scalars = input_scalars.clone();
    let mut projection_scratch_identities = input_identities.clone();
    runtime.project_accounts(
        profile,
        &input_scalars,
        &input_identities,
        &mut projection_scratch_scalars,
        &mut projection_scratch_identities,
        &mut projected_scalars,
        &mut projected_identities,
    )?;

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

    let plan = runtime.prepare_effects(
        program_id,
        profile,
        effect,
        &transition_output_scalars,
        &transition_output_identities,
        &manifest_data,
        request,
        &rent,
        Clock::get().map_err(|_| TradingSbfError::Content)?.slot,
        root_header,
        descriptor,
    )?;
    drop(effect_data);
    drop(profile_data);
    drop(config_data);
    drop(descriptor_data);
    drop(manifest_data);

    commit_activation(
        program_id,
        &framed,
        &suffix,
        root_header,
        &runtime,
        plan.clone(),
    )?;
    emit_ack(
        program_id,
        envelope,
        envelope_bytes,
        role_request_bytes,
        plan,
    )
}

struct AuthenticatedSuffixV2<'accounts, 'info> {
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
    system: &'accounts AccountInfo<'info>,
    effect_accounts: &'accounts [AccountInfo<'info>],
}

impl<'accounts, 'info> AuthenticatedSuffixV2<'accounts, 'info> {
    fn parse(
        program_id: &Pubkey,
        accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        let value = Self {
            descriptor_raw: get(accounts, DESCRIPTOR_RAW)?,
            descriptor_staging: get(accounts, DESCRIPTOR_STAGING)?,
            config_raw: get(accounts, CONFIG_RAW)?,
            config_staging: get(accounts, CONFIG_STAGING)?,
            profile_raw: get(accounts, PROFILE_RAW)?,
            profile_staging: get(accounts, PROFILE_STAGING)?,
            effect_raw: get(accounts, EFFECT_RAW)?,
            effect_staging: get(accounts, EFFECT_STAGING)?,
            cache: get(accounts, ACTIVATION_CACHE)?,
            core_program: get(accounts, CORE_PROGRAM)?,
            core_programdata: get(accounts, CORE_PROGRAMDATA)?,
            trading_program: get(accounts, TRADING_PROGRAM)?,
            trading_programdata: get(accounts, TRADING_PROGRAMDATA)?,
            registry: get(accounts, REGISTRY_PROGRAM)?,
            rent: get(accounts, RENT_SYSVAR)?,
            system: get(accounts, SYSTEM_PROGRAM)?,
            effect_accounts: accounts
                .get(EFFECT_ACCOUNTS_START..)
                .ok_or(TradingSbfError::Content)?,
        };
        if value.trading_program.key != program_id
            || value.rent.key != &sysvar::rent::ID
            || value.system.key != &system_program::ID
            || !value.core_program.executable
            || !value.trading_program.executable
            || !value.registry.executable
            || !value.system.executable
            || value.core_program.is_writable
            || value.trading_program.is_writable
            || value.registry.is_writable
            || value.system.is_writable
            || value.core_programdata.is_writable
            || value.trading_programdata.is_writable
            || value.rent.is_writable
            || accounts.iter().any(|account| account.is_signer)
        {
            return Err(TradingSbfError::Content.into());
        }
        require_authentication_accounts_distinct(accounts)?;
        Ok(value)
    }
}

fn require_authentication_accounts_distinct(
    accounts: &[AccountInfo<'_>],
) -> Result<(), ProgramError> {
    let fixed = accounts
        .get(..EFFECT_ACCOUNTS_START)
        .ok_or(TradingSbfError::Content)?;
    for (index, account) in fixed.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .ok_or(TradingSbfError::Content)?
            .iter()
            .any(|other| account.key == other.key)
        {
            return Err(TradingSbfError::Content.into());
        }
    }
    Ok(())
}

fn authenticate_market_and_caller(
    program_id: &Pubkey,
    framed: &TradingActivationAccountsV1<'_, '_>,
    suffix: &AuthenticatedSuffixV2<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
) -> Result<CoreState, ProgramError> {
    let market = framed.market();
    if market.owner != suffix.core_program.key
        || market.data_len() != STATE_BYTES
        || market.is_writable
        || market.executable
        || envelope.caller_program().to_bytes() != suffix.core_program.key.to_bytes()
        || envelope.caller_authority().to_bytes() != framed.core_authority().key.to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    let bytes = market
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let state = CoreState::decode(&bytes).map_err(|_| TradingSbfError::Content)?;
    let canonical = state.encode().map_err(|_| TradingSbfError::Content)?;
    let seeds = MarketCoreStateSeedsV2::new(state.identity);
    let expected_market =
        Pubkey::find_program_address(&seeds.as_slices(), suffix.core_program.key).0;
    if canonical.as_slice() != bytes.as_ref()
        || expected_market != *market.key
        || state.identity.market_id.to_bytes() != market.key.to_bytes()
        || state.identity.registry_program.to_bytes() != suffix.registry.key.to_bytes()
        || state.identity.selected_release_set.to_bytes() != envelope.release_set().to_bytes()
        || state.identity.generation != envelope.generation()
        || envelope.market().to_bytes() != market.key.to_bytes()
        || envelope.parent_state_digest().to_bytes() != hash(&bytes).to_bytes()
    {
        return Err(TradingSbfError::Content.into());
    }
    let authority_seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| TradingSbfError::Content)?;
    let expected_authority =
        Pubkey::find_program_address(&authority_seeds.as_slices(), suffix.core_program.key).0;
    if expected_authority != *framed.core_authority().key {
        return Err(TradingSbfError::Release.into());
    }
    let _ = program_id;
    Ok(state)
}

fn reauthenticate_role<'info>(
    suffix: &AuthenticatedSuffixV2<'_, 'info>,
    role: ExecutionRoleV1,
    role_program: &AccountInfo<'info>,
    role_programdata: &AccountInfo<'info>,
    release_set: [u8; 32],
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    let expected_cache = Pubkey::find_program_address(
        &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
        suffix.registry.key,
    )
    .0;
    if suffix.cache.key != &expected_cache || suffix.cache.owner != suffix.registry.key {
        return Err(TradingSbfError::Release.into());
    }
    let instruction = Instruction {
        program_id: *suffix.registry.key,
        accounts: vec![
            AccountMeta::new_readonly(*suffix.cache.key, false),
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
            suffix.cache.clone(),
            role_program.clone(),
            role_programdata.clone(),
            suffix.registry.clone(),
        ],
    )
    .map_err(|_| TradingSbfError::Release)?;
    let (producer, bytes) = get_return_data().ok_or(TradingSbfError::Release)?;
    let receipt =
        AuthenticatedRoleReceiptV1::decode(&bytes).map_err(|_| TradingSbfError::Release)?;
    if producer != *suffix.registry.key {
        solana_program::msg!("Trading activation: Registry return producer mismatch");
        return Err(TradingSbfError::Release.into());
    }
    if receipt.role() != role {
        solana_program::msg!("Trading activation: Registry receipt role mismatch");
        return Err(TradingSbfError::Release.into());
    }
    if receipt.execution_release_set_id().to_bytes() != release_set {
        solana_program::msg!("Trading activation: Registry receipt release-set mismatch");
        return Err(TradingSbfError::Release.into());
    }
    if receipt.program().to_bytes() != role_program.key.to_bytes() {
        solana_program::msg!("Trading activation: Registry receipt Program mismatch");
        return Err(TradingSbfError::Release.into());
    }
    Ok(receipt)
}

fn authenticate_vacant_root(
    program_id: &Pubkey,
    root: &AccountInfo<'_>,
    header: CapabilityRootHeaderV1,
    root_bytes: usize,
) -> Result<(), ProgramError> {
    let expected = Pubkey::find_program_address(&header.seeds().as_slices(), program_id).0;
    if root.key != &expected
        || root.owner != &system_program::ID
        || root.data_len() != 0
        || root.executable
        || !root.is_writable
        || root_bytes == 0
    {
        return Err(TradingSbfError::Root.into());
    }
    Ok(())
}

fn authenticate_finalized_record(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
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
        || raw.is_writable
        || raw.executable
        || hash(bytes).to_bytes() != digest
        || !rent.is_exempt(raw.lamports(), bytes.len())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.is_writable
        || staging.executable
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

struct RuntimeFrameV2<'accounts, 'info> {
    accounts: Vec<&'accounts AccountInfo<'info>>,
    funding_count: usize,
}

impl<'accounts, 'info> RuntimeFrameV2<'accounts, 'info> {
    fn new(
        framed: &TradingActivationAccountsV1<'accounts, 'info>,
        effect_accounts: &'accounts [AccountInfo<'info>],
    ) -> Result<Self, ProgramError> {
        let count = 1_usize
            .checked_add(framed.funding().len())
            .and_then(|value| value.checked_add(effect_accounts.len()))
            .ok_or(TradingSbfError::Content)?;
        if count == 0 || count > MAX_RUNTIME_ACCOUNTS_V2 {
            return Err(TradingSbfError::Content.into());
        }
        let mut accounts = Vec::with_capacity(count);
        accounts.push(framed.child_root());
        accounts.extend(framed.funding().iter());
        accounts.extend(effect_accounts.iter());
        Ok(Self {
            accounts,
            funding_count: framed.funding().len(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn project_accounts(
        &self,
        profile: AccountProfileV1<'_>,
        input_scalars: &[u64],
        input_identities: &[[u8; 32]],
        scratch_scalars: &mut [u64],
        scratch_identities: &mut [[u8; 32]],
        output_scalars: &mut [u64],
        output_identities: &mut [[u8; 32]],
    ) -> Result<(), ProgramError> {
        if usize::from(profile.account_count()) != self.accounts.len() {
            return Err(TradingSbfError::Content.into());
        }
        let data = self
            .accounts
            .iter()
            .map(|account| {
                account
                    .try_borrow_data()
                    .map_err(|_| TradingSbfError::Content)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let observations = self
            .accounts
            .iter()
            .zip(data.iter())
            .map(|(account, bytes)| {
                AccountObservationV1::new(
                    account.key.as_array(),
                    account.owner.as_array(),
                    account.lamports(),
                    bytes.as_ref(),
                    account.is_signer,
                    account.is_writable,
                    account.executable,
                )
            })
            .collect::<Vec<_>>();
        project_accounts_atomic(
            profile,
            &observations,
            ProjectionRegistersV2::new(
                RegisterInput {
                    scalars: input_scalars,
                    identities: input_identities,
                },
                RegisterOutput {
                    scalars: scratch_scalars,
                    identities: scratch_identities,
                },
                RegisterOutput {
                    scalars: output_scalars,
                    identities: output_identities,
                },
            ),
        )
        .map_err(|_| TradingSbfError::Content.into())
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_effects(
        &self,
        program_id: &Pubkey,
        profile: AccountProfileV1<'_>,
        effect: EffectProgramV2<'_>,
        scalars: &[u64],
        identities: &[[u8; 32]],
        manifest_bytes: &[u8],
        request: TradingActivationRequestV1<'_>,
        rent: &Rent,
        current_slot: u64,
        root_header: CapabilityRootHeaderV1,
        descriptor: CapabilityProgramV1<'_>,
    ) -> Result<ActivationPlanV2, ProgramError> {
        let root_state_bytes =
            usize::try_from(descriptor.root_state_bytes()).map_err(|_| TradingSbfError::Root)?;
        if usize::from(effect.account_count()) != self.accounts.len()
            || effect.scalar_count() != profile.scalar_count()
            || effect.identity_count() != profile.identity_count()
            || usize::from(effect.request_bytes()) > MAX_ROLE_REQUEST_BYTES_V2
        {
            return Err(TradingSbfError::Content.into());
        }
        // The effect program's projected request buffer IS the family root tail.
        // The activation outer never decodes a family root -- it has no family
        // decoder and must not acquire one -- so the only family-neutral channel
        // for the initial tail is an artifact the family already authors and the
        // manifest entry already binds. Declaring a different width is refused
        // rather than truncated or zero-padded.
        if usize::from(effect.request_bytes()) != root_state_bytes {
            return Err(TradingSbfError::Root.into());
        }
        let account_inputs = self
            .accounts
            .iter()
            .map(|account| AccountInput {
                lamports: account.lamports(),
                data_len: account.data_len(),
            })
            .collect::<Vec<_>>();
        let mut permissions =
            vec![dclutch_effect_kernel::v2::AccountPermission::read_only(); self.accounts.len()];
        derive_effect_permissions(profile, &mut permissions)
            .map_err(|_| TradingSbfError::Content)?;
        let mut scratch_lamports = vec![0_u64; self.accounts.len()];
        let mut output_lamports = vec![0_u64; self.accounts.len()];
        let mut scratch_request = vec![0_u8; usize::from(effect.request_bytes())];
        let mut output_request = vec![0_u8; usize::from(effect.request_bytes())];
        let aliases = (0..self.accounts.len())
            .map(|index| {
                profile
                    .rule(u16::try_from(index).map_err(|_| TradingSbfError::Content)?)
                    .map(|rule| rule.alias_of())
                    .map_err(|_| TradingSbfError::Content)
            })
            .collect::<Result<Vec<_>, _>>()?;
        project_with_aliases_and_requests_atomic(
            effect,
            scalars,
            identities,
            &aliases,
            &account_inputs,
            &permissions,
            &mut scratch_lamports,
            &mut output_lamports,
            &mut scratch_request,
            &mut output_request,
        )
        .map_err(|_| TradingSbfError::Content)?;
        require_activation_local_effects(effect, scalars, identities, self.funding_count)?;

        let manifest =
            CapabilityManifestV1::decode(manifest_bytes).map_err(|_| TradingSbfError::Content)?;
        let manifest_id = content(request.selection().manifest().to_bytes())?;
        let mut funding_after = Vec::with_capacity(self.funding_count);
        for (index, account) in self
            .accounts
            .iter()
            .enumerate()
            .skip(1)
            .take(self.funding_count)
        {
            if account.owner != program_id
                || account.data_len() != FUNDING_STATE_BYTES
                || profile
                    .rule(u16::try_from(index).map_err(|_| TradingSbfError::Content)?)
                    .map_err(|_| TradingSbfError::Content)?
                    .effect_permissions()
                    & EFFECT_PERMISSION_WRITE_DATA
                    == 0
            {
                return Err(TradingSbfError::Content.into());
            }
            let bytes = account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?;
            let mut funding =
                FundingStateV1::decode(&bytes).map_err(|_| TradingSbfError::Content)?;
            let derivation = CapabilityFundingDerivationV1::new(
                root_header.market(),
                root_header.generation(),
                manifest_id,
                manifest,
                funding,
            )
            .map_err(|_| TradingSbfError::Content)?;
            let expected =
                Pubkey::find_program_address(&derivation.seed_components(), program_id).0;
            if expected != *account.key {
                return Err(TradingSbfError::Content.into());
            }
            let funding_rent = rent.minimum_balance(FUNDING_STATE_BYTES);
            let custody =
                FundingCustodyObservationV1::native_only(account.lamports(), funding_rent)
                    .map_err(|_| TradingSbfError::Content)?;
            funding
                .activate(manifest_id, manifest, custody, current_slot)
                .map_err(|_| TradingSbfError::Content)?;
            let expected_lamports = funding_rent
                .checked_add(funding.remaining().native_lamports_total())
                .ok_or(TradingSbfError::Content)?;
            if output_lamports.get(index).copied() != Some(expected_lamports) {
                return Err(TradingSbfError::Content.into());
            }
            funding_after.push(funding.to_bytes());
        }
        let root_rent = rent.minimum_balance(
            descriptor
                .root_account_bytes()
                .map_err(|_| TradingSbfError::Root)?,
        );
        if output_lamports.first().copied() != Some(root_rent) {
            return Err(TradingSbfError::Root.into());
        }
        // An activation that projects no family state at all creates a root whose
        // tail no family can decode -- every in-tree family root refuses all-zero
        // at its magic. That is a bricked capability, not a successful activation,
        // so it refuses here instead of committing.
        if root_state_bytes != 0 && output_request.iter().all(|byte| *byte == 0) {
            return Err(TradingSbfError::Root.into());
        }
        let mut root_data = vec![
            0_u8;
            descriptor
                .root_account_bytes()
                .map_err(|_| TradingSbfError::Root)?
        ];
        initialize_root_account_v1(&mut root_data, root_header, descriptor, &output_request)
            .map_err(|_| TradingSbfError::Root)?;
        CapabilityRootAccountV1::decode(&root_data, descriptor)
            .map_err(|_| TradingSbfError::Root)?;

        let post_digest = poststate_digest(&root_data, &funding_after, &output_lamports)?;
        Ok(ActivationPlanV2 {
            output_lamports,
            root_data,
            funding_after,
            post_digest,
        })
    }
}

#[derive(Clone)]
struct ActivationPlanV2 {
    output_lamports: Vec<u64>,
    root_data: Vec<u8>,
    funding_after: Vec<[u8; FUNDING_STATE_BYTES]>,
    post_digest: Identity,
}

fn require_activation_local_effects(
    effect: EffectProgramV2<'_>,
    scalars: &[u64],
    identities: &[[u8; 32]],
    funding_count: usize,
) -> Result<(), ProgramError> {
    let first_nonfunding = 1_usize
        .checked_add(funding_count)
        .ok_or(TradingSbfError::Content)?;
    let mut index = 0_u16;
    while index < effect.instruction_count() {
        match effect
            .resolved_effect(index, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?
        {
            ResolvedEffect::WriteScalar { account, .. }
            | ResolvedEffect::WriteIdentity { account, .. }
                if usize::from(account) < first_nonfunding =>
            {
                return Err(TradingSbfError::Content.into());
            }
            ResolvedEffect::WriteScalar { .. } | ResolvedEffect::WriteIdentity { .. } => {
                return Err(TradingSbfError::UnsupportedContent.into());
            }
            ResolvedEffect::InvokeRole { enabled: true, .. } => {
                return Err(TradingSbfError::UnsupportedContent.into());
            }
            // A request write composes the family root tail; a lamport move and a
            // balance requirement are the funding semantics. A disabled invoke is
            // a no-op the projection already resolved away.
            ResolvedEffect::WriteRequestScalar { .. }
            | ResolvedEffect::WriteRequestIdentity { .. }
            | ResolvedEffect::TransferLamports { .. }
            | ResolvedEffect::RequireLamportsEq { .. }
            | ResolvedEffect::InvokeRole { enabled: false, .. } => {}
        }
        index = index.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn seed_common_registers(
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
    program_id: &Pubkey,
    suffix: &AuthenticatedSuffixV2<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    request: TradingActivationRequestV1<'_>,
    descriptor: CapabilityProgramV1<'_>,
    root: &Pubkey,
) -> Result<(), ProgramError> {
    if scalars.len() < COMMON_SCALARS_V2
        || identities.len() < COMMON_IDENTITIES_V2
        || scalars.len() > MAX_RUNTIME_SCALARS_V2
        || identities.len() > MAX_RUNTIME_IDENTITIES_V2
        || descriptor.transition_program().scalar_count() as usize != scalars.len()
        || descriptor.transition_program().identity_count() as usize != identities.len()
    {
        return Err(TradingSbfError::Content.into());
    }
    for (slot, value) in [
        CoreEffectActionV1::ActivateCapability as u64,
        envelope.generation(),
        u64::from(request.selection().entry_index()),
        u64::from(request.funding().funding_count()),
        u64::from(envelope.role_request_bytes()),
        u64::from(descriptor.root_state_bytes()),
        envelope.expected_resource_a_revision(),
        envelope.expected_resource_b_revision(),
    ]
    .into_iter()
    .enumerate()
    {
        *scalars.get_mut(slot).ok_or(TradingSbfError::Content)? = value;
    }
    for (slot, value) in [
        program_id.to_bytes(),
        suffix.core_program.key.to_bytes(),
        suffix.registry.key.to_bytes(),
        envelope.release_set().to_bytes(),
        envelope.market().to_bytes(),
        envelope.context().to_bytes(),
        request.selection().manifest().to_bytes(),
        request.selection().capability_release().to_bytes(),
        request.selection().config().to_bytes(),
        descriptor.account_profile().to_bytes(),
        descriptor.effect_schema().to_bytes(),
        root.to_bytes(),
    ]
    .into_iter()
    .enumerate()
    {
        *identities.get_mut(slot).ok_or(TradingSbfError::Content)? = value;
    }
    Ok(())
}

fn commit_activation<'accounts, 'info>(
    program_id: &Pubkey,
    framed: &TradingActivationAccountsV1<'accounts, 'info>,
    suffix: &AuthenticatedSuffixV2<'accounts, 'info>,
    root_header: CapabilityRootHeaderV1,
    runtime: &RuntimeFrameV2<'_, '_>,
    plan: ActivationPlanV2,
) -> Result<(), ProgramError> {
    let root_space = u64::try_from(plan.root_data.len()).map_err(|_| TradingSbfError::Root)?;
    let seeds = root_header.seeds();
    let base = seeds.as_slices();
    let (expected, bump) = Pubkey::find_program_address(&base, program_id);
    if expected != *framed.child_root().key {
        return Err(TradingSbfError::Root.into());
    }
    let bump_seed = [bump];
    let signer = [
        base[0], base[1], base[2], base[3], base[4], base[5], base[6], base[7], &bump_seed,
    ];
    invoke_signed(
        &allocate(framed.child_root().key, root_space),
        &[framed.child_root().clone(), suffix.system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Commit)?;
    invoke_signed(
        &assign(framed.child_root().key, program_id),
        &[framed.child_root().clone(), suffix.system.clone()],
        &[&signer],
    )
    .map_err(|_| TradingSbfError::Commit)?;

    for (account, lamports) in runtime.accounts.iter().zip(plan.output_lamports.iter()) {
        **account
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)? = *lamports;
    }
    {
        let mut root = framed
            .child_root()
            .try_borrow_mut_data()
            .map_err(|_| TradingSbfError::Commit)?;
        if root.len() != plan.root_data.len() {
            return Err(TradingSbfError::Commit.into());
        }
        root.copy_from_slice(&plan.root_data);
    }
    for (account, bytes) in framed.funding().iter().zip(plan.funding_after.iter()) {
        let mut data = account
            .try_borrow_mut_data()
            .map_err(|_| TradingSbfError::Commit)?;
        if data.len() != FUNDING_STATE_BYTES {
            return Err(TradingSbfError::Commit.into());
        }
        data.copy_from_slice(bytes);
    }
    Ok(())
}

fn emit_ack(
    program_id: &Pubkey,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
    plan: ActivationPlanV2,
) -> Result<(), ProgramError> {
    let envelope_len = u32::try_from(envelope_bytes.len()).map_err(|_| TradingSbfError::Content)?;
    let request_len = u32::try_from(role_request.len()).map_err(|_| TradingSbfError::Content)?;
    let digest = hashv(&[
        &CORE_EFFECT_DIGEST_DOMAIN_V1,
        &envelope_len.to_le_bytes(),
        envelope_bytes,
        &request_len.to_le_bytes(),
        role_request,
    ]);
    let ack = CoreEffectAckV1::new(
        envelope.action(),
        envelope.target_role(),
        identity(program_id.to_bytes())?,
        envelope.release_set(),
        envelope.market(),
        envelope.context(),
        identity(digest.to_bytes())?,
        plan.post_digest,
        envelope.expected_resource_a_revision(),
        envelope.expected_resource_a_revision(),
        envelope.expected_resource_b_revision(),
        envelope.expected_resource_b_revision(),
    )
    .map_err(|_| TradingSbfError::Commit)?;
    let bytes = ack.encode().map_err(|_| TradingSbfError::Commit)?;
    let _: [u8; CORE_EFFECT_ACK_BYTES_V1] = bytes;
    set_return_data(&bytes);
    Ok(())
}

fn poststate_digest(
    root: &[u8],
    funding: &[[u8; FUNDING_STATE_BYTES]],
    lamports: &[u64],
) -> Result<Identity, ProgramError> {
    let mut parts: Vec<&[u8]> = Vec::with_capacity(1 + funding.len() + lamports.len());
    parts.push(root);
    for bytes in funding {
        parts.push(bytes);
    }
    let encoded_lamports = lamports
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    parts.push(&encoded_lamports);
    identity(hashv(&parts).to_bytes())
}

fn content(bytes: [u8; 32]) -> Result<ContentId, ProgramError> {
    ContentId::new(bytes).map_err(|_| TradingSbfError::Content.into())
}

fn identity(bytes: [u8; 32]) -> Result<Identity, ProgramError> {
    Identity::new(bytes).map_err(|_| TradingSbfError::Content.into())
}

fn get<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, ProgramError> {
    accounts
        .get(index)
        .ok_or_else(|| TradingSbfError::Content.into())
}
