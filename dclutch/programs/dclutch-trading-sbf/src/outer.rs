//! Executable, family-neutral Core-to-Trading capability lifecycle boundary.
//!
//! The outer authenticates the current Core and Trading deployments, four
//! finalized content records, and an interpreted account/effect profile. It
//! does not dispatch on a capability kind. All physical mutation is projected
//! first and committed only after the complete activation or native-close plan
//! accepts.
//!
//! The created root is `CapabilityRootHeaderV1 || <family tail>`. The outer owns
//! the header and never decodes the tail; the tail is exactly the effect
//! program's projected request buffer, whose width the descriptor pins to
//! `root_state_bytes`. That keeps the family's initial state an artifact the
//! family authors and the Market's manifest entry binds, with no family decoder
//! and no kind branch on this path. Close requires a ProgramSet-selected
//! descriptor, authenticates the existing root and exact Market RentCredit,
//! leaves foreign dependency ledgers byte/lamport-identical, and never admits
//! Realm/token custody without an ordered-vault adapter.

extern crate alloc;

use alloc::{vec, vec::Vec};
use core::cell::Ref;

use dclutch_account_profile_contract::{
    ACCOUNT_PROFILE_SCHEMA_RELEASE_ID_V1, AccountObservationV1, AccountProfileV1,
    EFFECT_PERMISSION_CREDIT_LAMPORTS, EFFECT_PERMISSION_DEBIT_LAMPORTS,
    EFFECT_PERMISSION_WRITE_DATA, ProjectionRegistersV2, derive_effect_permissions,
    project_atomic as project_accounts_atomic,
};
use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingLedgerDerivationV2,
    CapabilityManifestV1, ContentId, FundingLedgerCloseCustodyV2, FundingLedgerStatusV2,
    FundingLedgerV2, manifest_entry_for_ledger_row_v2, validate_funding_ledger_masks_v2,
};
use dclutch_capability_program_contract::{
    CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1, CapabilityProgramV1, CapabilityRegistersV2,
    CapabilityRootAccountV1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    activation_registers_v2::{
        ACTIVATION_ACCOUNT_PROFILE_IDENTITY_V2, ACTIVATION_ACTION_SCALAR_V2,
        ACTIVATION_CAPABILITY_RELEASE_IDENTITY_V2, ACTIVATION_COMMON_IDENTITIES_V2,
        ACTIVATION_COMMON_SCALARS_V2, ACTIVATION_CONFIG_IDENTITY_V2,
        ACTIVATION_CONTEXT_IDENTITY_V2, ACTIVATION_CORE_PROGRAM_IDENTITY_V2,
        ACTIVATION_EFFECT_SCHEMA_IDENTITY_V2, ACTIVATION_ENTRY_INDEX_SCALAR_V2,
        ACTIVATION_FUNDING_COUNT_SCALAR_V2, ACTIVATION_GENERATION_SCALAR_V2,
        ACTIVATION_MANIFEST_IDENTITY_V2, ACTIVATION_MARKET_IDENTITY_V2,
        ACTIVATION_REGISTRY_PROGRAM_IDENTITY_V2, ACTIVATION_RELEASE_SET_IDENTITY_V2,
        ACTIVATION_RESOURCE_A_REVISION_SCALAR_V2, ACTIVATION_RESOURCE_B_REVISION_SCALAR_V2,
        ACTIVATION_ROLE_REQUEST_BYTES_SCALAR_V2, ACTIVATION_ROOT_IDENTITY_V2,
        ACTIVATION_ROOT_STATE_BYTES_SCALAR_V2, ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
    },
    initialize_root_account_v1,
    set_v2::{CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, CapabilityProgramSetV2},
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
use dclutch_rent_contract::lifecycle_v2::{LIFECYCLE_RENT_CREDIT_BYTES_V2, LifecycleRentCreditV2};
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
        TRADING_CLOSE_RENT_CREDIT_IDENTITY_V2, TradingActivationAccountsV2,
        TradingActivationRequestV2, TradingCloseRequestV2, TradingFamilyContextV1,
        authenticate_activation_program,
    },
};

const RELEASE_RAW: usize = 0;
const RELEASE_STAGING: usize = 1;
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
/// Fixed authentication accounts every activation carries.
const AUTHENTICATION_ACCOUNTS_V1: usize = 16;
/// Selected activation descriptor, present only for a `ProgramSet` release.
const SET_DESCRIPTOR_RAW: usize = 16;
/// Its staging cursor.
const SET_DESCRIPTOR_STAGING: usize = 17;

const MAX_RUNTIME_SCALARS_V2: usize = 96;
const MAX_RUNTIME_IDENTITIES_V2: usize = 32;
const MAX_RUNTIME_ACCOUNTS_V2: usize = 64;
const MAX_ROLE_REQUEST_BYTES_V2: usize = 2_048;

/// Close-only runtime suffix index of the current Rent Program.
const CLOSE_RENT_PROGRAM: usize = 0;
/// Close-only runtime suffix index of the Market's writable RentCredit.
const CLOSE_RENT_CREDIT: usize = 1;
/// Fixed close-only accounts before any family validation observations.
const CLOSE_RUNTIME_PREFIX_ACCOUNTS_V2: usize = 2;

/// Domain for the activation poststate commitment in [`poststate_digest`].
const ACTIVATION_POSTSTATE_DIGEST_DOMAIN_V2: &[u8] = b"dclutch:activation-poststate:v2";

/// Which generation of capability release `selection.capability_release()` names.
///
/// This is not a dispatch on a capability kind. It is a fact about one finalized
/// record, decided the only way a raw record can be identified before it is read:
/// its raw-record PDA is `[RAW_RECORD_PDA_SEED_V1, schema, digest]`, so the
/// supplied account's own address says which schema the Registry finalized it
/// under. Exactly one of the two derivations can match a given account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CapabilityReleaseGenerationV1 {
    /// The record at `capability_release` IS the activation descriptor.
    FlatDescriptor,
    /// The record at `capability_release` is a `CapabilityProgramSetV2`, and the
    /// activation descriptor is the entry its family request selects.
    ProgramSet,
}

impl CapabilityReleaseGenerationV1 {
    /// Extra finalized-record accounts this generation carries after the fixed 16.
    const fn extra_accounts(self) -> usize {
        match self {
            Self::FlatDescriptor => 0,
            Self::ProgramSet => 2,
        }
    }

    /// Schema the record at `capability_release` is authenticated under.
    const fn release_schema(self) -> [u8; 32] {
        match self {
            Self::FlatDescriptor => CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1,
            Self::ProgramSet => CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        }
    }
}

/// Decide the release generation from the supplied raw record's own address.
///
/// It returns the canonical raw bump alongside the generation because it has
/// just paid for it: the search that identifies the generation IS the search
/// the record authentication would otherwise repeat from identical seeds.
fn select_release_generation(
    registry: &Pubkey,
    release_raw: &AccountInfo<'_>,
    capability_release: [u8; 32],
) -> Result<(CapabilityReleaseGenerationV1, (Pubkey, u8)), ProgramError> {
    for generation in [
        CapabilityReleaseGenerationV1::FlatDescriptor,
        CapabilityReleaseGenerationV1::ProgramSet,
    ] {
        let coordinate = Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &generation.release_schema(),
                &capability_release,
            ],
            registry,
        );
        if release_raw.key == &coordinate.0 {
            return Ok((generation, coordinate));
        }
    }
    Err(TradingSbfError::Content.into())
}

/// Execute the versioned capability activation/close family selected by Core.
///
/// Unknown actions remain on the activation decoder and refuse; only the exact
/// V1 Core close action enters the independently authenticated close route.
#[inline(never)]
pub fn process_capability_lifecycle(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let envelope = CoreEffectEnvelopeV1::decode(
        instruction_data
            .get(..CORE_EFFECT_ENVELOPE_BYTES_V1)
            .ok_or(TradingSbfError::Content)?,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if envelope.action() == CoreEffectActionV1::CloseCapability {
        process_close(program_id, accounts, instruction_data)
    } else {
        process_activation(program_id, accounts, instruction_data)
    }
}

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
    let request = TradingActivationRequestV2::decode(role_request_bytes)?;
    envelope
        .validate_role_request(
            role_request_bytes.len(),
            identity(hash(role_request_bytes).to_bytes())?,
        )
        .map_err(|_| TradingSbfError::Content)?;
    let framed = TradingActivationAccountsV2::parse(accounts, request.funding())?;
    let family_accounts = framed.family_accounts();
    let capability_release = request.selection().capability_release().to_bytes();
    let (generation, release_raw_coordinate) = select_release_generation(
        get(family_accounts, REGISTRY_PROGRAM)?.key,
        get(family_accounts, RELEASE_RAW)?,
        capability_release,
    )?;
    let suffix = AuthenticatedSuffixV2::parse(program_id, family_accounts, generation)?;
    let market_state = authenticate_market_and_caller(program_id, &framed, &suffix, envelope)?;
    let rent = Rent::from_account_info(suffix.rent).map_err(|_| TradingSbfError::Content)?;

    let release_data = suffix
        .release_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    // Activation is the one route entitled to SEARCH for these coordinates. It
    // hands what it finds to the root it is about to write, and every hot
    // reader derives from that instead. See `hot_v3::borrow_finalized_record_at`.
    //
    // The raw bump is NOT searched for again: `select_release_generation` has
    // already found it, from these exact seeds, and matched the account.
    let release_staging_coordinate = finalized_staging_coordinate(
        suffix.registry.key,
        generation.release_schema(),
        capability_release,
    );
    let release_record_bumps = (release_raw_coordinate.1, release_staging_coordinate.1);
    authenticate_finalized_record_against(
        suffix.registry.key,
        suffix.release_raw,
        suffix.release_staging,
        &rent,
        capability_release,
        &release_data,
        release_raw_coordinate.0,
        release_staging_coordinate.0,
    )?;
    let set_descriptor_data = authenticate_set_descriptor(
        &suffix,
        generation,
        &rent,
        capability_release,
        release_data.as_ref(),
        request.family_request(),
    )?;
    let descriptor_data: &[u8] = match &set_descriptor_data {
        Some(data) => data.as_ref(),
        None => release_data.as_ref(),
    };
    let descriptor_id = content(hash(descriptor_data).to_bytes())?;
    let descriptor =
        CapabilityProgramV1::decode(descriptor_data).map_err(|_| TradingSbfError::Content)?;
    let root_bytes = descriptor
        .root_account_bytes()
        .map_err(|_| TradingSbfError::Root)?;
    // The manifest raw record had NO address authentication on this route at
    // all -- it was admitted on `hash(bytes) == selection.manifest()` alone.
    // Deriving its coordinate for the root's sake makes that check free, so it
    // is taken: the account this activation reads and the account every later
    // hot action will read are now required to be the same one.
    let (expected_manifest_raw, manifest_raw_bump) = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &request.selection().manifest().to_bytes(),
        ],
        suffix.registry.key,
    );
    if framed.manifest().key != &expected_manifest_raw
        || framed.manifest().owner != suffix.registry.key
    {
        return Err(TradingSbfError::Content.into());
    }
    // The staging cursor is not in this frame, so its bump is derived and not
    // observed. That is sound because a bump is a pure function of the seeds:
    // what the root records is a memo of a computation, and the hot reader
    // still requires the account it is handed to sit at the address that memo
    // reproduces, Registry-owned, with a closed cursor beside it.
    let manifest_record_bumps = (
        manifest_raw_bump,
        finalized_staging_coordinate(
            suffix.registry.key,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            request.selection().manifest().to_bytes(),
        )
        .1,
    );
    let (config_raw_coordinate, config_staging_coordinate) = finalized_record_coordinates(
        suffix.registry.key,
        descriptor.config_schema().to_bytes(),
        request.selection().config().to_bytes(),
    );
    let config_record_bumps = (config_raw_coordinate.1, config_staging_coordinate.1);
    let root_header = CapabilityRootHeaderV1::new(
        content(envelope.release_set().to_bytes())?,
        envelope.market().to_bytes(),
        envelope.generation(),
        request
            .selection()
            .with_capability_release_record_bumps(release_record_bumps.0, release_record_bumps.1),
        SelectedRecordBumpsV1::new(
            manifest_record_bumps.0,
            manifest_record_bumps.1,
            config_record_bumps.0,
            config_record_bumps.1,
        ),
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
    authenticate_finalized_record_against(
        suffix.registry.key,
        suffix.config_raw,
        suffix.config_staging,
        &rent,
        request.selection().config().to_bytes(),
        &config_data,
        config_raw_coordinate.0,
        config_staging_coordinate.0,
    )?;
    let descriptor = authenticate_activation_program(
        context,
        descriptor_id,
        &manifest_data,
        descriptor_data,
        &config_data,
    )?;

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
    drop(set_descriptor_data);
    drop(release_data);
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

/// Execute one Core-signed, ProgramSet-versioned native capability close.
///
/// The authenticated descriptor/profile/transition authorizes the family's
/// terminal state. The outer then owns the physical close: the selected
/// one-row Trading ledger and composite root are the only debits, the exact
/// Market RentCredit is the only credit, dependency ledgers are immutable, and
/// Realm/token custody refuses until its ordered-vault adapter lands.
#[inline(never)]
pub fn process_close(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> Result<(), ProgramError> {
    let (envelope_bytes, role_request_bytes) = instruction_data
        .split_at_checked(CORE_EFFECT_ENVELOPE_BYTES_V1)
        .ok_or(TradingSbfError::Content)?;
    let envelope =
        CoreEffectEnvelopeV1::decode(envelope_bytes).map_err(|_| TradingSbfError::Content)?;
    if envelope.action() != CoreEffectActionV1::CloseCapability
        || envelope.target_role() != Role::Trading
    {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let request = TradingCloseRequestV2::decode(role_request_bytes)?;
    envelope
        .validate_role_request(
            role_request_bytes.len(),
            identity(hash(role_request_bytes).to_bytes())?,
        )
        .map_err(|_| TradingSbfError::Content)?;
    let framed = TradingActivationAccountsV2::parse(accounts, request.funding())?;
    let family_accounts = framed.family_accounts();
    let capability_release = request.selection().capability_release().to_bytes();
    let (generation, release_raw_coordinate) = select_release_generation(
        get(family_accounts, REGISTRY_PROGRAM)?.key,
        get(family_accounts, RELEASE_RAW)?,
        capability_release,
    )?;
    // A flat activation descriptor has no independent close selector. Never
    // reinterpret it under a second action.
    if generation != CapabilityReleaseGenerationV1::ProgramSet {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let suffix = AuthenticatedSuffixV2::parse(program_id, family_accounts, generation)?;
    let market_state = authenticate_market_and_caller(program_id, &framed, &suffix, envelope)?;
    let rent = Rent::from_account_info(suffix.rent).map_err(|_| TradingSbfError::Content)?;

    let release_data = suffix
        .release_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let release_staging_coordinate = finalized_staging_coordinate(
        suffix.registry.key,
        generation.release_schema(),
        capability_release,
    );
    authenticate_finalized_record_against(
        suffix.registry.key,
        suffix.release_raw,
        suffix.release_staging,
        &rent,
        capability_release,
        &release_data,
        release_raw_coordinate.0,
        release_staging_coordinate.0,
    )?;
    let set_descriptor_data = authenticate_set_descriptor(
        &suffix,
        generation,
        &rent,
        capability_release,
        release_data.as_ref(),
        request.family_request(),
    )?
    .ok_or(TradingSbfError::UnsupportedContent)?;
    let descriptor_id = content(hash(set_descriptor_data.as_ref()).to_bytes())?;
    let descriptor = CapabilityProgramV1::decode(set_descriptor_data.as_ref())
        .map_err(|_| TradingSbfError::Content)?;

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
    let root_data = framed
        .child_root()
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    let root = CapabilityRootAccountV1::decode(&root_data, descriptor)
        .map_err(|_| TradingSbfError::Root)?;
    let root_header = root.header();
    let context = TradingFamilyContextV1::authenticate(
        program_id,
        framed.child_root().key,
        framed.child_root().owner,
        &root_data,
        trading_receipt,
    )?;
    require_close_selection(context, request, envelope)?;
    drop(root_data);

    let (expected_manifest_raw, _) = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            &context.selection().manifest().to_bytes(),
        ],
        suffix.registry.key,
    );
    if framed.manifest().key != &expected_manifest_raw
        || framed.manifest().owner != suffix.registry.key
    {
        return Err(TradingSbfError::Content.into());
    }
    let manifest_data = framed
        .manifest()
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let (config_raw_coordinate, config_staging_coordinate) = finalized_record_coordinates(
        suffix.registry.key,
        descriptor.config_schema().to_bytes(),
        context.selection().config().to_bytes(),
    );
    let config_data = suffix
        .config_raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record_against(
        suffix.registry.key,
        suffix.config_raw,
        suffix.config_staging,
        &rent,
        context.selection().config().to_bytes(),
        &config_data,
        config_raw_coordinate.0,
        config_staging_coordinate.0,
    )?;
    let descriptor = authenticate_activation_program(
        context,
        descriptor_id,
        &manifest_data,
        set_descriptor_data.as_ref(),
        &config_data,
    )?;

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

    let credit = authenticate_close_rent_credit(&suffix, market_state, &rent)?;
    // The Rent Program authenticates the credit's owner/PDA but is not a
    // family runtime account: no CPI executes and no profile may grant it an
    // effect. The RentCredit and any later validation observations are the
    // descriptor-owned runtime suffix.
    let close_runtime_accounts = suffix
        .effect_accounts
        .get(CLOSE_RENT_CREDIT..)
        .ok_or(TradingSbfError::Content)?;
    let runtime = RuntimeFrameV2::new_close(
        &framed,
        close_runtime_accounts,
        request.selection().entry_index(),
    )?;
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
    *input_identities
        .get_mut(usize::from(TRADING_CLOSE_RENT_CREDIT_IDENTITY_V2))
        .ok_or(TradingSbfError::Content)? = credit.key;
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
    let plan = runtime.prepare_close(
        program_id,
        profile,
        effect,
        &transition_output_scalars,
        &transition_output_identities,
        &manifest_data,
        request,
        &rent,
        root_header,
        framed.child_root().lamports(),
        credit,
    )?;
    drop(effect_data);
    drop(profile_data);
    drop(config_data);
    drop(set_descriptor_data);
    drop(release_data);
    drop(manifest_data);

    commit_close(program_id, &framed, &suffix, &plan)?;
    emit_ack_for_post(
        program_id,
        envelope,
        envelope_bytes,
        role_request_bytes,
        plan.post_digest,
    )
}

struct AuthenticatedSuffixV2<'accounts, 'info> {
    release_raw: &'accounts AccountInfo<'info>,
    release_staging: &'accounts AccountInfo<'info>,
    set_descriptor_raw: Option<&'accounts AccountInfo<'info>>,
    set_descriptor_staging: Option<&'accounts AccountInfo<'info>>,
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
        generation: CapabilityReleaseGenerationV1,
    ) -> Result<Self, ProgramError> {
        let authentication_accounts = AUTHENTICATION_ACCOUNTS_V1
            .checked_add(generation.extra_accounts())
            .ok_or(TradingSbfError::Content)?;
        let (set_descriptor_raw, set_descriptor_staging) = match generation {
            CapabilityReleaseGenerationV1::FlatDescriptor => (None, None),
            CapabilityReleaseGenerationV1::ProgramSet => (
                Some(get(accounts, SET_DESCRIPTOR_RAW)?),
                Some(get(accounts, SET_DESCRIPTOR_STAGING)?),
            ),
        };
        let value = Self {
            release_raw: get(accounts, RELEASE_RAW)?,
            release_staging: get(accounts, RELEASE_STAGING)?,
            set_descriptor_raw,
            set_descriptor_staging,
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
                .get(authentication_accounts..)
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
        require_authentication_accounts_distinct(accounts, authentication_accounts)?;
        Ok(value)
    }
}

/// Authenticate the activation descriptor a `ProgramSet` release selects.
///
/// A flat release IS its own activation descriptor and this returns `None`. A
/// `CapabilityProgramSetV2` release is a selector table, so the descriptor is a
/// second finalized record, named by the entry the family request selects and
/// authenticated under the schema that entry states. Requiring
/// `CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1` there is what keeps this seam
/// family-neutral without acquiring a second descriptor decoder: a hot-action
/// `CapabilityProgramV4` entry can never be presented here, because the raw
/// record it names is finalized under a different schema and therefore lives at
/// a different address.
///
/// The caller choosing the entry is the same trust structure the hot path
/// already has, and it is bounded twice over: every entry is inside the set
/// whose digest the Market's manifest binds, and `validate_selection` then
/// requires the selected descriptor's kind, capacity profile, root schema and
/// derivation policy to equal the manifest entry's own.
fn authenticate_set_descriptor<'accounts, 'info>(
    suffix: &AuthenticatedSuffixV2<'accounts, 'info>,
    generation: CapabilityReleaseGenerationV1,
    rent: &Rent,
    capability_release: [u8; 32],
    release_data: &[u8],
    family_request: &[u8],
) -> Result<Option<Ref<'accounts, &'accounts mut [u8]>>, ProgramError> {
    let (raw, staging) = match generation {
        CapabilityReleaseGenerationV1::FlatDescriptor => return Ok(None),
        CapabilityReleaseGenerationV1::ProgramSet => (
            suffix.set_descriptor_raw.ok_or(TradingSbfError::Content)?,
            suffix
                .set_descriptor_staging
                .ok_or(TradingSbfError::Content)?,
        ),
    };
    // `authenticate_finalized_record` already required `hash(release_data)` to be
    // exactly `capability_release`, so the selected and authenticated set
    // identities are the same value by construction.
    let set = CapabilityProgramSetV2::decode_selected(
        capability_release,
        capability_release,
        release_data,
    )
    .map_err(|_| TradingSbfError::Content)?;
    let selected = set
        .select_descriptor(family_request)
        .map_err(|_| TradingSbfError::Content)?;
    if selected.schema().to_bytes() != CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V1 {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    let data = raw
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    authenticate_finalized_record(
        suffix.registry.key,
        raw,
        staging,
        rent,
        selected.schema().to_bytes(),
        selected.program().to_bytes(),
        data.as_ref(),
    )?;
    Ok(Some(data))
}

fn require_authentication_accounts_distinct(
    accounts: &[AccountInfo<'_>],
    authentication_accounts: usize,
) -> Result<(), ProgramError> {
    let fixed = accounts
        .get(..authentication_accounts)
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
    framed: &TradingActivationAccountsV2<'_, '_>,
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

fn require_close_selection(
    context: TradingFamilyContextV1,
    request: TradingCloseRequestV2<'_>,
    envelope: CoreEffectEnvelopeV1,
) -> Result<(), ProgramError> {
    let persisted = context.selection();
    let proposed = request.selection();
    if context.market() != envelope.market().to_bytes()
        || context.generation() != envelope.generation()
        || context.release_set().to_bytes() != envelope.release_set().to_bytes()
        || persisted.entry_index() != proposed.entry_index()
        || persisted.manifest() != proposed.manifest()
        || persisted.kind() != proposed.kind()
        || persisted.capability_release() != proposed.capability_release()
        || persisted.config() != proposed.config()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
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

#[derive(Clone, Copy)]
struct AuthenticatedCloseRentCreditV2 {
    key: [u8; 32],
    pre_lamports: u64,
}

fn authenticate_close_rent_credit(
    suffix: &AuthenticatedSuffixV2<'_, '_>,
    market: CoreState,
    rent: &Rent,
) -> Result<AuthenticatedCloseRentCreditV2, ProgramError> {
    if suffix.effect_accounts.len() < CLOSE_RUNTIME_PREFIX_ACCOUNTS_V2 {
        return Err(TradingSbfError::Content.into());
    }
    let rent_program = get(suffix.effect_accounts, CLOSE_RENT_PROGRAM)?;
    let credit_account = get(suffix.effect_accounts, CLOSE_RENT_CREDIT)?;
    if rent_program.key == credit_account.key
        || !rent_program.executable
        || rent_program.is_writable
        || credit_account.executable
        || !credit_account.is_writable
        || credit_account.owner != rent_program.key
        || credit_account.data_len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || credit_account.key.to_bytes() != market.rent_beneficiary.to_bytes()
        || !rent.is_exempt(credit_account.lamports(), LIFECYCLE_RENT_CREDIT_BYTES_V2)
    {
        return Err(TradingSbfError::Content.into());
    }
    // No Rent CPI occurs here. The immutable Market names the exact RentCredit,
    // and that canonical record names its owner/PDA. Requiring ProgramData or a
    // live-code receipt would add a mutable-code trust edge to a pure lamport
    // credit operation without authenticating any additional persisted fact.
    let data = credit_account
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Content)?;
    let credit = LifecycleRentCreditV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
    if credit.to_bytes().as_slice() != data.as_ref()
        || credit.market().to_bytes() != market.identity.market_id.to_bytes()
        || credit.release_set().to_bytes() != market.identity.selected_release_set.to_bytes()
        || credit.generation() != market.identity.generation
    {
        return Err(TradingSbfError::Content.into());
    }
    let seeds = credit.pda_seeds();
    let market_seed = seeds.market().to_bytes();
    let generation_seed = seeds.generation();
    let bump = [seeds.bump()];
    let expected = Pubkey::create_program_address(
        &[
            seeds.domain(),
            market_seed.as_slice(),
            generation_seed.as_slice(),
            &bump,
        ],
        rent_program.key,
    )
    .map_err(|_| TradingSbfError::Content)?;
    if expected != *credit_account.key {
        return Err(TradingSbfError::Content.into());
    }
    Ok(AuthenticatedCloseRentCreditV2 {
        key: credit_account.key.to_bytes(),
        pre_lamports: credit_account.lamports(),
    })
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

/// The canonical raw/staging bumps of one finalized record's coordinate.
///
/// A pure function of the seeds: no account is consulted and none needs to
/// exist. Activation calls this so the root it writes can carry what it found,
/// and every later reader derives with `create_program_address` instead of
/// searching. See `hot_v3::borrow_finalized_record_at`.
fn finalized_record_coordinates(
    registry: &Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> ((Pubkey, u8), (Pubkey, u8)) {
    (
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], registry),
        finalized_staging_coordinate(registry, schema, digest),
    )
}

/// The canonical address and bump of one finalized record's staging cursor.
fn finalized_staging_coordinate(
    registry: &Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], registry)
}

/// Authenticate one finalized record against coordinates the caller derived.
#[allow(clippy::too_many_arguments)]
fn authenticate_finalized_record_against(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    digest: [u8; 32],
    bytes: &[u8],
    expected_raw: Pubkey,
    expected_staging: Pubkey,
) -> Result<(), ProgramError> {
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

/// Authenticate one finalized record, searching for both of its addresses.
///
/// Activation is a write-time route: it is entitled to search, and it is the
/// authority that hands the readers what it found. It returns the two canonical
/// bumps for exactly that reason.
fn authenticate_finalized_record(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    rent: &Rent,
    schema: [u8; 32],
    digest: [u8; 32],
    bytes: &[u8],
) -> Result<(u8, u8), ProgramError> {
    let (raw_coordinate, staging_coordinate) =
        finalized_record_coordinates(registry, schema, digest);
    authenticate_finalized_record_against(
        registry,
        raw,
        staging,
        rent,
        digest,
        bytes,
        raw_coordinate.0,
        staging_coordinate.0,
    )?;
    Ok((raw_coordinate.1, staging_coordinate.1))
}

struct RuntimeFrameV2<'accounts, 'info> {
    accounts: Vec<&'accounts AccountInfo<'info>>,
    funding: Vec<&'accounts AccountInfo<'info>>,
    close_selected_funding_index: Option<usize>,
}

impl<'accounts, 'info> RuntimeFrameV2<'accounts, 'info> {
    fn new(
        framed: &TradingActivationAccountsV2<'accounts, 'info>,
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
            funding: framed.funding().iter().collect(),
            close_selected_funding_index: None,
        })
    }

    /// Project a native close through only root, selected Trading ledger, and
    /// RentCredit. All physical ledgers remain in `funding` for complete
    /// dependency authentication and poststate commitments, but a foreign
    /// dependency can never acquire a descriptor-owned runtime permission.
    fn new_close(
        framed: &TradingActivationAccountsV2<'accounts, 'info>,
        effect_accounts: &'accounts [AccountInfo<'info>],
        selected_entry_index: u16,
    ) -> Result<Self, ProgramError> {
        let selected_bit = 1_u16
            .checked_shl(u32::from(selected_entry_index))
            .ok_or(TradingSbfError::Content)?;
        let mut selected = None;
        for (index, account) in framed.funding().iter().enumerate() {
            let data = account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?;
            let ledger = FundingLedgerV2::decode(&data).map_err(|_| TradingSbfError::Content)?;
            if ledger.selected_mask() & selected_bit != 0
                && selected.replace((index, account)).is_some()
            {
                return Err(TradingSbfError::Content.into());
            }
        }
        let (selected_index, selected_account) = selected.ok_or(TradingSbfError::Content)?;
        let count = 2_usize
            .checked_add(effect_accounts.len())
            .ok_or(TradingSbfError::Content)?;
        if count > MAX_RUNTIME_ACCOUNTS_V2 {
            return Err(TradingSbfError::Content.into());
        }
        let mut accounts = Vec::with_capacity(count);
        accounts.push(framed.child_root());
        accounts.push(selected_account);
        accounts.extend(effect_accounts.iter());
        Ok(Self {
            accounts,
            funding: framed.funding().iter().collect(),
            close_selected_funding_index: Some(selected_index),
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
        request: TradingActivationRequestV2<'_>,
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
        require_activation_local_effects(effect, scalars, identities, self.funding.len())?;

        let manifest =
            CapabilityManifestV1::decode(manifest_bytes).map_err(|_| TradingSbfError::Content)?;
        let manifest_id = content(request.selection().manifest().to_bytes())?;
        let mut ledger_masks = Vec::with_capacity(self.funding.len());
        let mut funding_after = Vec::with_capacity(self.funding.len());
        let selected_entry_index = request.selection().entry_index();
        let selected_bit = 1_u16
            .checked_shl(u32::from(selected_entry_index))
            .ok_or(TradingSbfError::Content)?;
        let mut selected_present = false;
        let mut selected_funding_index = None;
        for (physical_index, account) in self.funding.iter().enumerate() {
            let index = physical_index
                .checked_add(1)
                .ok_or(TradingSbfError::Content)?;
            let pre_bytes = account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?;
            let ledger =
                FundingLedgerV2::decode(&pre_bytes).map_err(|_| TradingSbfError::Content)?;
            let authenticated = ledger
                .authenticate(manifest_id, manifest)
                .map_err(|_| TradingSbfError::Content)?;
            let selected_mask = ledger.selected_mask();
            let writes_data = profile
                .rule(u16::try_from(index).map_err(|_| TradingSbfError::Content)?)
                .map_err(|_| TradingSbfError::Content)?
                .effect_permissions()
                & EFFECT_PERMISSION_WRITE_DATA
                != 0;
            if require_funding_ledger_access(
                program_id,
                account.owner,
                account.is_writable,
                writes_data,
                selected_mask,
                selected_bit,
            )? {
                selected_funding_index =
                    Some(index.checked_sub(1).ok_or(TradingSbfError::Content)?);
            }
            let derivation = CapabilityFundingLedgerDerivationV2::new(
                account.owner.to_bytes(),
                root_header.market(),
                root_header.generation(),
                manifest_id,
                ledger,
            )
            .map_err(|_| TradingSbfError::Content)?;
            let expected =
                Pubkey::find_program_address(&derivation.seed_components(), account.owner).0;
            if expected != *account.key {
                return Err(TradingSbfError::Content.into());
            }
            let ledger_rent = rent.minimum_balance(pre_bytes.len());
            authenticated
                .validate_native_custody(account.lamports(), ledger_rent, false)
                .map_err(|_| TradingSbfError::Content)?;
            let slot_count = ledger.slot_count();
            let mut row_index = 0_u16;
            while row_index < slot_count {
                let entry_index = manifest_entry_for_ledger_row_v2(selected_mask, row_index)
                    .map_err(|_| TradingSbfError::Content)?;
                // Realm collateral requires one explicitly framed, quote-bound
                // vault per row. The activation common frame currently carries
                // only ledgers, so accepting such a row here would authenticate
                // its native half while silently trusting an unobserved token
                // account. Refuse until the descriptor ABI owns that exact map.
                require_native_funding_row(
                    manifest
                        .entry(entry_index)
                        .map_err(|_| TradingSbfError::Content)?
                        .funding_quote()
                        .realm_collateral(),
                )?;
                let slot = authenticated
                    .slot(entry_index)
                    .map_err(|_| TradingSbfError::Content)?;
                if entry_index == selected_entry_index {
                    selected_present = true;
                } else if slot.status() != FundingLedgerStatusV2::Active
                    || slot.activation_slot() == 0
                {
                    return Err(TradingSbfError::Content.into());
                }
                row_index = row_index.checked_add(1).ok_or(TradingSbfError::Content)?;
            }
            ledger_masks.push(selected_mask);
            let mut ledger_post = pre_bytes.to_vec();
            drop(pre_bytes);
            if selected_mask & selected_bit != 0 {
                FundingLedgerV2::activate_in_place(
                    &mut ledger_post,
                    manifest_id,
                    manifest,
                    selected_entry_index,
                    current_slot,
                )
                .map_err(|_| TradingSbfError::Content)?;
            }
            let post = FundingLedgerV2::decode(&ledger_post)
                .and_then(|value| value.authenticate(manifest_id, manifest))
                .map_err(|_| TradingSbfError::Content)?;
            let expected_lamports = ledger_rent
                .checked_add(
                    post.remaining_native_lamports_total()
                        .map_err(|_| TradingSbfError::Content)?,
                )
                .ok_or(TradingSbfError::Content)?;
            if output_lamports.get(index).copied() != Some(expected_lamports) {
                return Err(TradingSbfError::Content.into());
            }
            funding_after.push(ledger_post);
        }
        validate_funding_ledger_masks_v2(
            manifest.entry_count(),
            request.funding().selected_mask(),
            &ledger_masks,
        )
        .map_err(|_| TradingSbfError::Content)?;
        if !selected_present {
            return Err(TradingSbfError::Content.into());
        }
        let selected_funding_index = selected_funding_index.ok_or(TradingSbfError::Content)?;
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
            selected_funding_index,
            post_digest,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_close(
        &self,
        program_id: &Pubkey,
        profile: AccountProfileV1<'_>,
        effect: EffectProgramV2<'_>,
        scalars: &[u64],
        identities: &[[u8; 32]],
        manifest_bytes: &[u8],
        request: TradingCloseRequestV2<'_>,
        rent: &Rent,
        root_header: CapabilityRootHeaderV1,
        root_lamports: u64,
        credit: AuthenticatedCloseRentCreditV2,
    ) -> Result<NativeClosePlanV2, ProgramError> {
        if usize::from(effect.account_count()) != self.accounts.len()
            || effect.scalar_count() != profile.scalar_count()
            || effect.identity_count() != profile.identity_count()
            || effect.request_bytes() != 0
        {
            return Err(TradingSbfError::Content.into());
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
        let aliases = (0..self.accounts.len())
            .map(|index| {
                profile
                    .rule(u16::try_from(index).map_err(|_| TradingSbfError::Content)?)
                    .map(|rule| rule.alias_of())
                    .map_err(|_| TradingSbfError::Content)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut scratch_lamports = vec![0_u64; self.accounts.len()];
        let mut output_lamports = vec![0_u64; self.accounts.len()];
        let mut scratch_request = Vec::new();
        let mut output_request = Vec::new();
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
        require_close_validation_effects(effect, scalars, identities)?;
        if output_lamports
            .iter()
            .zip(account_inputs.iter())
            .any(|(output, input)| *output != input.lamports)
        {
            return Err(TradingSbfError::Content.into());
        }

        require_profile_permissions(
            profile,
            0,
            EFFECT_PERMISSION_DEBIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA,
        )?;
        let credit_runtime_index = self
            .accounts
            .iter()
            .position(|account| account.key.to_bytes() == credit.key)
            .ok_or(TradingSbfError::Content)?;
        require_profile_permissions(
            profile,
            credit_runtime_index,
            EFFECT_PERMISSION_CREDIT_LAMPORTS,
        )?;
        if self
            .accounts
            .get(credit_runtime_index)
            .is_none_or(|account| account.key.to_bytes() != credit.key)
        {
            return Err(TradingSbfError::Content.into());
        }

        let manifest =
            CapabilityManifestV1::decode(manifest_bytes).map_err(|_| TradingSbfError::Content)?;
        let manifest_id = content(request.selection().manifest().to_bytes())?;
        let selected_entry_index = request.selection().entry_index();
        let selected_bit = 1_u16
            .checked_shl(u32::from(selected_entry_index))
            .ok_or(TradingSbfError::Content)?;
        let mut ledger_masks = Vec::with_capacity(self.funding.len());
        let mut funding_after = Vec::with_capacity(self.funding.len());
        let mut selected_funding_index = None;
        let mut selected_close = None;
        for (physical_index, account) in self.funding.iter().enumerate() {
            let pre_bytes = account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::Content)?;
            let ledger =
                FundingLedgerV2::decode(&pre_bytes).map_err(|_| TradingSbfError::Content)?;
            let authenticated = ledger
                .authenticate(manifest_id, manifest)
                .map_err(|_| TradingSbfError::Content)?;
            let selected_mask = ledger.selected_mask();
            let carries_selected = selected_mask & selected_bit != 0;
            let writes_data = if carries_selected {
                profile
                    .rule(1)
                    .map_err(|_| TradingSbfError::Content)?
                    .effect_permissions()
                    & EFFECT_PERMISSION_WRITE_DATA
                    != 0
            } else {
                false
            };
            let selected = require_funding_ledger_access(
                program_id,
                account.owner,
                account.is_writable,
                writes_data,
                selected_mask,
                selected_bit,
            )?;
            let derivation = CapabilityFundingLedgerDerivationV2::new(
                account.owner.to_bytes(),
                root_header.market(),
                root_header.generation(),
                manifest_id,
                ledger,
            )
            .map_err(|_| TradingSbfError::Content)?;
            let expected =
                Pubkey::find_program_address(&derivation.seed_components(), account.owner).0;
            if expected != *account.key {
                return Err(TradingSbfError::Content.into());
            }
            let exact_ledger_rent = rent.minimum_balance(pre_bytes.len());
            authenticated
                .validate_native_custody(account.lamports(), exact_ledger_rent, selected)
                .map_err(|_| TradingSbfError::Content)?;
            let slot_count = ledger.slot_count();
            let mut row_index = 0_u16;
            while row_index < slot_count {
                let entry_index = manifest_entry_for_ledger_row_v2(selected_mask, row_index)
                    .map_err(|_| TradingSbfError::Content)?;
                require_native_funding_row(
                    manifest
                        .entry(entry_index)
                        .map_err(|_| TradingSbfError::Content)?
                        .funding_quote()
                        .realm_collateral(),
                )?;
                let slot = authenticated
                    .slot(entry_index)
                    .map_err(|_| TradingSbfError::Content)?;
                if slot.status() != FundingLedgerStatusV2::Active {
                    return Err(TradingSbfError::Content.into());
                }
                row_index = row_index.checked_add(1).ok_or(TradingSbfError::Content)?;
            }
            ledger_masks.push(selected_mask);
            if selected {
                require_profile_permissions(
                    profile,
                    1,
                    EFFECT_PERMISSION_DEBIT_LAMPORTS | EFFECT_PERMISSION_WRITE_DATA,
                )?;
                let mut post_bytes = pre_bytes.to_vec();
                drop(pre_bytes);
                let close = FundingLedgerV2::close_slot_in_place(
                    &mut post_bytes,
                    manifest_id,
                    manifest,
                    selected_entry_index,
                    FundingLedgerCloseCustodyV2::native_only(
                        account.lamports(),
                        exact_ledger_rent,
                        credit.key,
                    )
                    .map_err(|_| TradingSbfError::Content)?,
                )
                .map_err(|_| TradingSbfError::Content)?;
                if !close.ledger_can_close()
                    || close.expected_post_ledger_lamports() != 0
                    || FundingLedgerV2::decode(&post_bytes)
                        .and_then(|value| value.authenticate(manifest_id, manifest))
                        .is_err()
                {
                    return Err(TradingSbfError::Content.into());
                }
                selected_funding_index = Some(physical_index);
                selected_close = Some(close);
                // The physical account closes, so no ledger bytes survive even
                // though the logical tombstone was validated above.
                funding_after.push(Vec::new());
            } else {
                funding_after.push(pre_bytes.to_vec());
            }
        }
        validate_funding_ledger_masks_v2(
            manifest.entry_count(),
            request.funding().selected_mask(),
            &ledger_masks,
        )
        .map_err(|_| TradingSbfError::Content)?;
        let selected_funding_index = selected_funding_index.ok_or(TradingSbfError::Content)?;
        if self.close_selected_funding_index != Some(selected_funding_index) {
            return Err(TradingSbfError::Content.into());
        }
        let close = selected_close.ok_or(TradingSbfError::Content)?;
        let exact_root_rent = rent.minimum_balance(
            self.accounts
                .first()
                .ok_or(TradingSbfError::Content)?
                .data_len(),
        );
        let root_surplus = root_lamports
            .checked_sub(exact_root_rent)
            .ok_or(TradingSbfError::Content)?;
        let ledger_total = close
            .remaining_native_lamports()
            .checked_add(close.ledger_rent_lamports())
            .and_then(|value| value.checked_add(close.ledger_lamport_donation()))
            .ok_or(TradingSbfError::Content)?;
        let selected_runtime_index = 1_usize;
        if self
            .accounts
            .get(selected_runtime_index)
            .zip(self.funding.get(selected_funding_index))
            .is_none_or(|(runtime, physical)| {
                runtime.key != physical.key || runtime.lamports() != ledger_total
            })
        {
            return Err(TradingSbfError::Content.into());
        }
        let refund_total = exact_root_rent
            .checked_add(root_surplus)
            .and_then(|value| value.checked_add(ledger_total))
            .ok_or(TradingSbfError::Content)?;
        let credit_post_lamports = credit
            .pre_lamports
            .checked_add(refund_total)
            .ok_or(TradingSbfError::Content)?;
        let mut post_lamports = account_inputs
            .iter()
            .map(|input| input.lamports)
            .collect::<Vec<_>>();
        *post_lamports.get_mut(0).ok_or(TradingSbfError::Content)? = 0;
        *post_lamports
            .get_mut(selected_runtime_index)
            .ok_or(TradingSbfError::Content)? = 0;
        *post_lamports
            .get_mut(credit_runtime_index)
            .ok_or(TradingSbfError::Content)? = credit_post_lamports;
        let post_digest = poststate_digest(&[], &funding_after, &post_lamports)?;
        Ok(NativeClosePlanV2 {
            selected_funding_index,
            credit_pre_lamports: credit.pre_lamports,
            credit_post_lamports,
            remaining_native_principal: close.remaining_native_lamports(),
            root_rent_lamports: exact_root_rent,
            root_lamport_surplus: root_surplus,
            ledger_rent_lamports: close.ledger_rent_lamports(),
            ledger_lamport_surplus: close.ledger_lamport_donation(),
            post_digest,
        })
    }
}

fn require_native_funding_row(
    realm_collateral: Option<dclutch_capability_contract::RealmCollateralBindingV1>,
) -> Result<(), ProgramError> {
    if realm_collateral.is_some() {
        return Err(TradingSbfError::UnsupportedContent.into());
    }
    Ok(())
}

/// Admit exactly one Trading-owned selected ledger and foreign readonly
/// dependency ledgers. ManifestV1 has no per-entry controller field, so a
/// Trading-owned ledger may not mix the selected row with dependency rows.
fn require_funding_ledger_access(
    trading_program: &Pubkey,
    owner: &Pubkey,
    writable: bool,
    writes_data: bool,
    selected_mask: u16,
    selected_bit: u16,
) -> Result<bool, ProgramError> {
    let carries_selected = selected_mask & selected_bit != 0;
    if carries_selected {
        if selected_mask != selected_bit || owner != trading_program || !writable || !writes_data {
            return Err(TradingSbfError::Content.into());
        }
        return Ok(true);
    }
    if owner == &system_program::ID || owner == trading_program || writable || writes_data {
        return Err(TradingSbfError::Content.into());
    }
    Ok(false)
}

#[derive(Clone)]
struct ActivationPlanV2 {
    output_lamports: Vec<u64>,
    root_data: Vec<u8>,
    funding_after: Vec<Vec<u8>>,
    selected_funding_index: usize,
    post_digest: Identity,
}

struct NativeClosePlanV2 {
    selected_funding_index: usize,
    credit_pre_lamports: u64,
    credit_post_lamports: u64,
    remaining_native_principal: u64,
    root_rent_lamports: u64,
    root_lamport_surplus: u64,
    ledger_rent_lamports: u64,
    ledger_lamport_surplus: u64,
    post_digest: Identity,
}

fn require_profile_permissions(
    profile: AccountProfileV1<'_>,
    account_index: usize,
    required: u8,
) -> Result<(), ProgramError> {
    let observed = profile
        .rule(u16::try_from(account_index).map_err(|_| TradingSbfError::Content)?)
        .map_err(|_| TradingSbfError::Content)?
        .effect_permissions();
    if observed & required != required {
        return Err(TradingSbfError::Content.into());
    }
    Ok(())
}

fn require_close_validation_effects(
    effect: EffectProgramV2<'_>,
    scalars: &[u64],
    identities: &[[u8; 32]],
) -> Result<(), ProgramError> {
    let mut index = 0_u16;
    while index < effect.instruction_count() {
        match effect
            .resolved_effect(index, scalars, identities)
            .map_err(|_| TradingSbfError::Content)?
        {
            ResolvedEffect::RequireLamportsEq { .. }
            | ResolvedEffect::InvokeRole { enabled: false, .. } => {}
            ResolvedEffect::WriteScalar { .. }
            | ResolvedEffect::WriteIdentity { .. }
            | ResolvedEffect::WriteRequestScalar { .. }
            | ResolvedEffect::WriteRequestIdentity { .. }
            | ResolvedEffect::TransferLamports { .. }
            | ResolvedEffect::InvokeRole { enabled: true, .. } => {
                return Err(TradingSbfError::UnsupportedContent.into());
            }
        }
        index = index.checked_add(1).ok_or(TradingSbfError::Content)?;
    }
    Ok(())
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
    request: TradingActivationRequestV2<'_>,
    descriptor: CapabilityProgramV1<'_>,
    root: &Pubkey,
) -> Result<(), ProgramError> {
    if scalars.len() < ACTIVATION_COMMON_SCALARS_V2
        || identities.len() < ACTIVATION_COMMON_IDENTITIES_V2
        || scalars.len() > MAX_RUNTIME_SCALARS_V2
        || identities.len() > MAX_RUNTIME_IDENTITIES_V2
        || descriptor.transition_program().scalar_count() as usize != scalars.len()
        || descriptor.transition_program().identity_count() as usize != identities.len()
    {
        return Err(TradingSbfError::Content.into());
    }
    // The slots are named, not positional. They are the ABI a family's activation
    // artifacts are authored against, so `activation_registers_v2` publishes them
    // and this is the one writer.
    for (slot, value) in [
        (ACTIVATION_ACTION_SCALAR_V2, envelope.action() as u64),
        (ACTIVATION_GENERATION_SCALAR_V2, envelope.generation()),
        (
            ACTIVATION_ENTRY_INDEX_SCALAR_V2,
            u64::from(request.selection().entry_index()),
        ),
        (
            ACTIVATION_FUNDING_COUNT_SCALAR_V2,
            u64::from(request.funding().physical_count()),
        ),
        (
            ACTIVATION_ROLE_REQUEST_BYTES_SCALAR_V2,
            u64::from(envelope.role_request_bytes()),
        ),
        (
            ACTIVATION_ROOT_STATE_BYTES_SCALAR_V2,
            u64::from(descriptor.root_state_bytes()),
        ),
        (
            ACTIVATION_RESOURCE_A_REVISION_SCALAR_V2,
            envelope.expected_resource_a_revision(),
        ),
        (
            ACTIVATION_RESOURCE_B_REVISION_SCALAR_V2,
            envelope.expected_resource_b_revision(),
        ),
    ] {
        *scalars
            .get_mut(usize::from(slot))
            .ok_or(TradingSbfError::Content)? = value;
    }
    for (slot, value) in [
        (
            ACTIVATION_TRADING_PROGRAM_IDENTITY_V2,
            program_id.to_bytes(),
        ),
        (
            ACTIVATION_CORE_PROGRAM_IDENTITY_V2,
            suffix.core_program.key.to_bytes(),
        ),
        (
            ACTIVATION_REGISTRY_PROGRAM_IDENTITY_V2,
            suffix.registry.key.to_bytes(),
        ),
        (
            ACTIVATION_RELEASE_SET_IDENTITY_V2,
            envelope.release_set().to_bytes(),
        ),
        (ACTIVATION_MARKET_IDENTITY_V2, envelope.market().to_bytes()),
        (
            ACTIVATION_CONTEXT_IDENTITY_V2,
            envelope.context().to_bytes(),
        ),
        (
            ACTIVATION_MANIFEST_IDENTITY_V2,
            request.selection().manifest().to_bytes(),
        ),
        (
            ACTIVATION_CAPABILITY_RELEASE_IDENTITY_V2,
            request.selection().capability_release().to_bytes(),
        ),
        (
            ACTIVATION_CONFIG_IDENTITY_V2,
            request.selection().config().to_bytes(),
        ),
        (
            ACTIVATION_ACCOUNT_PROFILE_IDENTITY_V2,
            descriptor.account_profile().to_bytes(),
        ),
        (
            ACTIVATION_EFFECT_SCHEMA_IDENTITY_V2,
            descriptor.effect_schema().to_bytes(),
        ),
        (ACTIVATION_ROOT_IDENTITY_V2, root.to_bytes()),
    ] {
        *identities
            .get_mut(usize::from(slot))
            .ok_or(TradingSbfError::Content)? = value;
    }
    Ok(())
}

fn commit_activation<'accounts, 'info>(
    program_id: &Pubkey,
    framed: &TradingActivationAccountsV2<'accounts, 'info>,
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
        if account.lamports() != *lamports {
            **account
                .try_borrow_mut_lamports()
                .map_err(|_| TradingSbfError::Commit)? = *lamports;
        }
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
    let selected_account = framed
        .funding()
        .get(plan.selected_funding_index)
        .ok_or(TradingSbfError::Commit)?;
    let selected_bytes = plan
        .funding_after
        .get(plan.selected_funding_index)
        .ok_or(TradingSbfError::Commit)?;
    let mut data = selected_account
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?;
    if data.len() != selected_bytes.len() {
        return Err(TradingSbfError::Commit.into());
    }
    data.copy_from_slice(selected_bytes);
    Ok(())
}

#[inline(never)]
fn commit_close<'accounts, 'info>(
    program_id: &Pubkey,
    framed: &TradingActivationAccountsV2<'accounts, 'info>,
    suffix: &AuthenticatedSuffixV2<'accounts, 'info>,
    plan: &NativeClosePlanV2,
) -> Result<(), ProgramError> {
    let selected = framed
        .funding()
        .get(plan.selected_funding_index)
        .ok_or(TradingSbfError::Commit)?;
    let credit = get(suffix.effect_accounts, CLOSE_RENT_CREDIT)?;
    if framed.child_root().owner != program_id
        || selected.owner != program_id
        || framed.child_root().lamports()
            != plan
                .root_rent_lamports
                .checked_add(plan.root_lamport_surplus)
                .ok_or(TradingSbfError::Commit)?
        || selected.lamports()
            != plan
                .remaining_native_principal
                .checked_add(plan.ledger_rent_lamports)
                .and_then(|value| value.checked_add(plan.ledger_lamport_surplus))
                .ok_or(TradingSbfError::Commit)?
        || credit.lamports() != plan.credit_pre_lamports
    {
        return Err(TradingSbfError::Commit.into());
    }
    framed
        .child_root()
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?
        .fill(0);
    selected
        .try_borrow_mut_data()
        .map_err(|_| TradingSbfError::Commit)?
        .fill(0);
    {
        let mut root_lamports = framed
            .child_root()
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        let mut selected_lamports = selected
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        let mut credit_lamports = credit
            .try_borrow_mut_lamports()
            .map_err(|_| TradingSbfError::Commit)?;
        **root_lamports = 0;
        **selected_lamports = 0;
        **credit_lamports = plan.credit_post_lamports;
    }
    framed
        .child_root()
        .resize(0)
        .map_err(|_| TradingSbfError::Commit)?;
    selected.resize(0).map_err(|_| TradingSbfError::Commit)?;
    framed.child_root().assign(&system_program::ID);
    selected.assign(&system_program::ID);
    if framed.child_root().owner != &system_program::ID
        || framed.child_root().data_len() != 0
        || framed.child_root().lamports() != 0
        || selected.owner != &system_program::ID
        || selected.data_len() != 0
        || selected.lamports() != 0
        || credit.lamports() != plan.credit_post_lamports
    {
        return Err(TradingSbfError::Commit.into());
    }
    solana_program::msg!(
        "Trading close native principal={} root_rent={} root_surplus={} ledger_rent={} ledger_surplus={}",
        plan.remaining_native_principal,
        plan.root_rent_lamports,
        plan.root_lamport_surplus,
        plan.ledger_rent_lamports,
        plan.ledger_lamport_surplus,
    );
    Ok(())
}

fn emit_ack(
    program_id: &Pubkey,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
    plan: ActivationPlanV2,
) -> Result<(), ProgramError> {
    emit_ack_for_post(
        program_id,
        envelope,
        envelope_bytes,
        role_request,
        plan.post_digest,
    )
}

fn emit_ack_for_post(
    program_id: &Pubkey,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
    post_digest: Identity,
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
        post_digest,
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

/// Commit the exact activation poststate: root account, full subset-ledger
/// bytes, and output lamports.
///
/// Domain ‖ 0x00 ‖ u32_le(root len) ‖ u32_le(ledger count) ‖ u32_le(lamport
/// count) ‖ root ‖ (u32_le(ledger len) ‖ ledger)… ‖ lamports…. Ledger
/// lengths are explicit because subset masks make physical widths independent.
///
/// The three counts are ahead of the data on purpose. `hashv` concatenates its
/// parts and frames nothing, so digesting a variable-length root, a variable
/// number of ledgers, and a variable-length lamport encoding back to back would
/// commit only to their concatenation. Per-ledger lengths prevent bytes from
/// being reinterpreted across subset-ledger boundaries.
///
/// **This digest is currently verified by nobody.** It is carried as
/// `CoreEffectAckV1::post_resource_digest`, and `CoreEffectAckV1::validate_for`
/// compares every other field and not this one; no consumer in the tree
/// recomputes it. The framing is fixed here so the commitment is sound whenever
/// a consumer does start checking it, but the unchecked field is real debt and
/// is named as such rather than treated as closed.
fn poststate_digest(
    root: &[u8],
    funding: &[Vec<u8>],
    lamports: &[u64],
) -> Result<Identity, ProgramError> {
    let root_len = u32::try_from(root.len())
        .map_err(|_| TradingSbfError::Content)?
        .to_le_bytes();
    let funding_count = u32::try_from(funding.len())
        .map_err(|_| TradingSbfError::Content)?
        .to_le_bytes();
    let lamport_count = u32::try_from(lamports.len())
        .map_err(|_| TradingSbfError::Content)?
        .to_le_bytes();
    let encoded_lamports = lamports
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let ledger_lengths = funding
        .iter()
        .map(|bytes| {
            u32::try_from(bytes.len())
                .map(u32::to_le_bytes)
                .map_err(|_| TradingSbfError::Content.into())
        })
        .collect::<Result<Vec<_>, ProgramError>>()?;
    let mut parts: Vec<&[u8]> = Vec::with_capacity(6 + funding.len().saturating_mul(2));
    parts.push(ACTIVATION_POSTSTATE_DIGEST_DOMAIN_V2);
    parts.push(&[0_u8]);
    parts.push(&root_len);
    parts.push(&funding_count);
    parts.push(&lamport_count);
    parts.push(root);
    for (length, bytes) in ledger_lengths.iter().zip(funding.iter()) {
        parts.push(length);
        parts.push(bytes);
    }
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

#[cfg(test)]
mod funding_v2_tests {
    use super::*;
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
        RealmCollateralBindingV1, funding_ledger_bytes_v2,
    };

    #[test]
    fn realm_backed_row_refuses_until_the_ordered_vault_adapter_exists() {
        let realm = ContentId::new([1; 32]).expect("realm");
        let release = ContentId::new([2; 32]).expect("release");
        let binding = RealmCollateralBindingV1::new(realm, release, [3; 32], [4; 32], [5; 32])
            .expect("binding");
        assert_eq!(require_native_funding_row(None), Ok(()));
        assert_eq!(
            require_native_funding_row(Some(binding)),
            Err(TradingSbfError::UnsupportedContent.into())
        );
    }

    #[test]
    fn direct_dependency_union_is_readonly_and_only_trading_selection_is_writable() {
        let trading = Pubkey::new_from_array([7; 32]);
        let resolution = Pubkey::new_from_array([8; 32]);
        assert_eq!(
            require_funding_ledger_access(&trading, &resolution, false, false, 0b0111, 0b1000),
            Ok(false)
        );
        assert_eq!(
            require_funding_ledger_access(&trading, &trading, true, true, 0b1000, 0b1000),
            Ok(true)
        );
        for refused in [
            require_funding_ledger_access(&trading, &resolution, true, false, 0b0111, 0b1000),
            require_funding_ledger_access(&trading, &trading, false, false, 0b0111, 0b1000),
            require_funding_ledger_access(&trading, &trading, true, true, 0b1100, 0b1000),
        ] {
            assert_eq!(refused, Err(TradingSbfError::Content.into()));
        }
    }

    fn native_entry(seed: u8, dependencies: &[u8], rent_lamports: u64) -> CapabilityEntryV1 {
        let mut dependency_slots = [0_u8; MAX_DEPENDENCIES_PER_CAPABILITY];
        dependency_slots
            .get_mut(..dependencies.len())
            .expect("dependency width")
            .copy_from_slice(dependencies);
        let amounts = FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(rent_lamports).expect("native rent"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("funding amounts");
        CapabilityEntryV1::new(
            ContentId::new([seed; 32]).expect("kind"),
            ContentId::new([seed.wrapping_add(16); 32]).expect("release"),
            ContentId::new([seed.wrapping_add(32); 32]).expect("config"),
            ContentId::new([seed.wrapping_add(48); 32]).expect("capacity"),
            ContentId::new([seed.wrapping_add(64); 32]).expect("schema"),
            ContentId::new([seed.wrapping_add(80); 32]).expect("derivation"),
            ActivationPolicy::RequiredAtFounding,
            0,
            u8::try_from(dependencies.len()).expect("dependency count"),
            dependency_slots,
            FundingQuoteV1::new(amounts, None).expect("native quote"),
        )
        .expect("entry")
    }

    #[test]
    fn direct_v2_ledgers_activate_and_close_only_the_trading_selection() {
        let entries = [
            native_entry(1, &[], 11),
            native_entry(2, &[], 12),
            native_entry(3, &[], 13),
            native_entry(4, &[0, 1, 2], 100),
        ];
        let mut manifest_bytes =
            vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&entries, &mut manifest_bytes).expect("manifest");
        let manifest = CapabilityManifestV1::decode(&manifest_bytes).expect("manifest");
        let manifest_id = ContentId::new(hash(&manifest_bytes).to_bytes()).expect("manifest ID");
        let market = [9_u8; 32];
        let resolution = Pubkey::new_from_array([10; 32]);
        let trading = Pubkey::new_from_array([11; 32]);
        let generation = 7_u64;
        let resolution_rent = 5_000_u64;
        let trading_rent = 6_000_u64;

        let mut resolution_bytes =
            vec![0_u8; funding_ledger_bytes_v2(3).expect("Resolution ledger width")];
        FundingLedgerV2::initialize(&mut resolution_bytes, manifest_id, manifest, 0b0111)
            .expect("Resolution ledger");
        for (entry_index, slot) in [(0_u16, 91_u64), (1, 92), (2, 93)] {
            FundingLedgerV2::activate_in_place(
                &mut resolution_bytes,
                manifest_id,
                manifest,
                entry_index,
                slot,
            )
            .expect("dependency activation");
        }
        let resolution_ledger = FundingLedgerV2::decode(&resolution_bytes)
            .expect("Resolution ledger")
            .authenticate(manifest_id, manifest)
            .expect("Resolution authentication");
        let resolution_derivation = CapabilityFundingLedgerDerivationV2::new(
            resolution.to_bytes(),
            market,
            generation,
            manifest_id,
            resolution_ledger.ledger(),
        )
        .expect("Resolution derivation");
        let resolution_key =
            Pubkey::find_program_address(&resolution_derivation.seed_components(), &resolution).0;
        assert_ne!(resolution_key, Pubkey::default());
        resolution_ledger
            .validate_native_custody(resolution_rent, resolution_rent, false)
            .expect("Resolution custody");
        for entry_index in 0_u16..3 {
            let slot = resolution_ledger
                .slot(entry_index)
                .expect("dependency slot");
            assert_eq!(slot.status(), FundingLedgerStatusV2::Active);
            assert!(slot.activation_slot() > 0);
        }

        let mut trading_bytes =
            vec![0_u8; funding_ledger_bytes_v2(1).expect("Trading ledger width")];
        FundingLedgerV2::initialize(&mut trading_bytes, manifest_id, manifest, 0b1000)
            .expect("Trading ledger");
        let trading_ledger = FundingLedgerV2::decode(&trading_bytes)
            .expect("Trading ledger")
            .authenticate(manifest_id, manifest)
            .expect("Trading authentication");
        let trading_derivation = CapabilityFundingLedgerDerivationV2::new(
            trading.to_bytes(),
            market,
            generation,
            manifest_id,
            trading_ledger.ledger(),
        )
        .expect("Trading derivation");
        let trading_key =
            Pubkey::find_program_address(&trading_derivation.seed_components(), &trading).0;
        assert_ne!(trading_key, Pubkey::default());
        trading_ledger
            .validate_native_custody(trading_rent + 100, trading_rent, false)
            .expect("Trading custody");
        assert_eq!(
            trading_ledger.slot(3).expect("selected slot").status(),
            FundingLedgerStatusV2::Pending
        );

        let header = dclutch_market_core_codec::CapabilityFundingHeaderV2::new(2, 4, 0b1111)
            .expect("funding header");
        validate_funding_ledger_masks_v2(
            manifest.entry_count(),
            header.selected_mask(),
            &[0b0111, 0b1000],
        )
        .expect("exact disjoint union");
        assert_eq!(
            require_funding_ledger_access(&trading, &resolution, false, false, 0b0111, 0b1000,),
            Ok(false)
        );
        assert_eq!(
            require_funding_ledger_access(&trading, &trading, true, true, 0b1000, 0b1000),
            Ok(true)
        );

        let resolution_before = resolution_bytes.clone();
        let trading_before = trading_bytes.clone();
        let debit =
            FundingLedgerV2::activate_in_place(&mut trading_bytes, manifest_id, manifest, 3, 100)
                .expect("selected activation");
        assert_eq!(debit.rent_lamports(), 100);
        assert_eq!(resolution_bytes, resolution_before);
        assert_ne!(trading_bytes, trading_before);
        let trading_after = FundingLedgerV2::decode(&trading_bytes)
            .expect("Trading poststate")
            .authenticate(manifest_id, manifest)
            .expect("Trading poststate authentication");
        trading_after
            .validate_native_custody(trading_rent, trading_rent, false)
            .expect("Trading poststate custody");
        assert_eq!(
            trading_after.slot(3).expect("selected slot").status(),
            FundingLedgerStatusV2::Active
        );
        assert_eq!(
            trading_after
                .slot(3)
                .expect("selected slot")
                .activation_slot(),
            100
        );

        let close = FundingLedgerV2::close_slot_in_place(
            &mut trading_bytes,
            manifest_id,
            manifest,
            3,
            FundingLedgerCloseCustodyV2::native_only(trading_rent + 7, trading_rent, [12; 32])
                .expect("native close custody"),
        )
        .expect("selected close");
        assert!(close.ledger_can_close());
        assert_eq!(close.remaining_native_lamports(), 0);
        assert_eq!(close.ledger_rent_lamports(), trading_rent);
        assert_eq!(close.ledger_lamport_donation(), 7);
        assert_eq!(close.expected_post_ledger_lamports(), 0);
        assert_eq!(resolution_bytes, resolution_before);
        assert_eq!(
            FundingLedgerV2::decode(&trading_bytes)
                .expect("closed Trading ledger")
                .authenticate(manifest_id, manifest)
                .expect("closed Trading ledger authentication")
                .slot(3)
                .expect("closed slot")
                .status(),
            FundingLedgerStatusV2::Closed
        );
    }
}
