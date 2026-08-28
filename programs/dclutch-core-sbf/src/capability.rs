//! Generic data-defined Trading capability activation and closure.
//!
//! Core authenticates the immutable manifest, the fixed Registry-selected
//! Trading interpreter, and the ordered controller-owned FundingLedgerV2 set.
//! Trading may write only the selected entry's ledger; Resolution dependency
//! ledgers remain read-only and byte-identical across the child CPI. Core
//! commits only its outstanding-capability count after the exact child
//! acknowledgement and all physical postconditions succeed.

use alloc::vec::Vec;

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, FundingLedgerCloseCustodyV2,
    FundingLedgerStatusV2, FundingLedgerV2, validate_funding_ledger_masks_v2,
};
use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    Action, CapabilityChildObservation, CapabilityFundingHeaderV2, CoreEffectAckV1,
    CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState, MarketCoreStateSeedsV2, Request, Role,
    STATE_BYTES, activate_capability_child, close_capability_child,
};
use dclutch_realm_contract::{REALM_BYTES, REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_release_set_contract::{
    CAPABILITY_EXECUTION_SELECTION_BYTES_V1, CapabilityExecutionSelectionV1,
};
use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::{Sysvar, SysvarSerialize},
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    CoreSbfError,
    frame::require_distinct,
    records::authenticate_finalized_record,
    release::{RoleDeploymentAccounts, authenticate_roles, identity},
};

const MARKET: usize = 0;
const REALM_RAW: usize = 1;
const REALM_STAGING: usize = 2;
const MANIFEST_RAW: usize = 3;
const MANIFEST_STAGING: usize = 4;
const FUNDING_START: usize = 5;
const ROUTE_FIXED_ACCOUNTS: usize = 11;

struct Route<'accounts, 'info> {
    market: &'accounts AccountInfo<'info>,
    funding_ledgers: &'accounts [AccountInfo<'info>],
    manifest: &'accounts AccountInfo<'info>,
    root: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    child_program: &'accounts AccountInfo<'info>,
    child_programdata: &'accounts AccountInfo<'info>,
    resolution_program: &'accounts AccountInfo<'info>,
    resolution_programdata: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    rent: &'accounts AccountInfo<'info>,
    caller_authority: &'accounts AccountInfo<'info>,
    child_tail: &'accounts [AccountInfo<'info>],
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(crate) fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    request: Request,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
    selection: CapabilityExecutionSelectionV1,
    funding_header: CapabilityFundingHeaderV2,
) -> Result<(), ProgramError> {
    require_distinct(accounts)?;
    let route = Route::parse(accounts, funding_header.physical_count())?;
    route.validate(program_id)?;
    let market = account(accounts, MARKET)?;
    let state_bytes = read_state_bytes(program_id, market)?;
    let state = CoreState::decode(&state_bytes).map_err(|_| CoreSbfError::Market)?;
    authenticate_market(program_id, market, state, request)?;
    validate_envelope(program_id, request, envelope, role_request, &state_bytes)?;
    validate_selection(envelope, selection, state)?;

    let rent = Rent::from_account_info(route.rent).map_err(|_| CoreSbfError::Funding)?;
    let realm_raw = account(accounts, REALM_RAW)?;
    let realm_staging = account(accounts, REALM_STAGING)?;
    let manifest_raw = account(accounts, MANIFEST_RAW)?;
    let manifest_staging = account(accounts, MANIFEST_STAGING)?;
    let realm_data = realm_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    authenticate_finalized_record(
        route.registry.key,
        realm_raw,
        realm_staging,
        &rent,
        REALM_SCHEMA_RELEASE_ID_V1,
        state.identity.realm_id.to_bytes(),
        &realm_data,
    )?;
    if realm_data.len() != REALM_BYTES {
        return Err(CoreSbfError::Reference.into());
    }
    let realm = RealmV1::decode(&realm_data).map_err(|_| CoreSbfError::Reference)?;
    let manifest_data = manifest_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    authenticate_finalized_record(
        route.registry.key,
        manifest_raw,
        manifest_staging,
        &rent,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        state.identity.capability_manifest.to_bytes(),
        &manifest_data,
    )?;
    let manifest =
        CapabilityManifestV1::decode(&manifest_data).map_err(|_| CoreSbfError::Funding)?;
    let manifest_id = ContentId::new(state.identity.capability_manifest.to_bytes())
        .map_err(|_| CoreSbfError::Funding)?;
    validate_selected_entry(selection, manifest_id, manifest)?;
    let selected_mask = validate_funding_header(funding_header, selection, manifest)?;
    let admissions = authenticate_roles(
        route.cache,
        route.registry,
        state.identity.registry_program,
        state.identity.selected_release_set.to_bytes(),
        &[
            RoleDeploymentAccounts::new(Role::Core, route.core_program, route.core_programdata),
            RoleDeploymentAccounts::new(
                Role::Trading,
                route.child_program,
                route.child_programdata,
            ),
            RoleDeploymentAccounts::new(
                Role::Resolution,
                route.resolution_program,
                route.resolution_programdata,
            ),
        ],
    )?;
    let core_admission = admissions.admission(Role::Core)?;
    let child_admission = admissions.admission(Role::Trading)?;
    let resolution_admission = admissions.admission(Role::Resolution)?;
    if core_admission.receipt.observed.program.to_bytes() != program_id.to_bytes()
        || child_admission.receipt.observed.program.to_bytes() != route.child_program.key.to_bytes()
        || resolution_admission.receipt.observed.program.to_bytes()
            != route.resolution_program.key.to_bytes()
    {
        return Err(CoreSbfError::Release.into());
    }
    let now = Clock::get().map_err(|_| CoreSbfError::Funding)?.slot;
    let ledger_plans = validate_ledgers_pre(
        request.action,
        route.funding_ledgers,
        route.child_program.key,
        route.resolution_program.key,
        state,
        manifest_id,
        manifest,
        realm,
        &rent,
        selected_mask,
        selection.entry_index(),
        now,
    )?;

    authenticate_caller_authority(program_id, &route, envelope)?;
    let mut next_state = state;
    let observation = CapabilityChildObservation {
        target_role: Role::Trading,
        admission: child_admission,
        manifest_entry_authenticated: true,
        funding_state_authenticated: true,
        effect: complete_child_effect(),
    };
    match request.action {
        Action::ActivateCapability => {
            activate_capability_child(request, &mut next_state, observation)
                .map_err(|_| CoreSbfError::Transition)?;
        }
        Action::CloseCapability => {
            close_capability_child(request, &mut next_state, observation)
                .map_err(|_| CoreSbfError::Transition)?;
        }
        _ => return Err(CoreSbfError::Instruction.into()),
    }
    drop(manifest_data);
    drop(realm_data);

    invoke_child(program_id, &route, envelope, envelope_bytes, role_request)?;
    authenticate_ack(&route, envelope, envelope_bytes, role_request)?;
    match request.action {
        Action::ActivateCapability => {
            let manifest_data = manifest_raw
                .try_borrow_data()
                .map_err(|_| CoreSbfError::FinalizedRecord)?;
            let manifest =
                CapabilityManifestV1::decode(&manifest_data).map_err(|_| CoreSbfError::Funding)?;
            validate_ledgers_post(
                route.funding_ledgers,
                manifest_id,
                manifest,
                &rent,
                &ledger_plans,
            )?;
            if route.root.owner != route.child_program.key || route.root.data_len() == 0 {
                return Err(CoreSbfError::ChildAck.into());
            }
        }
        Action::CloseCapability => {
            let manifest_data = manifest_raw
                .try_borrow_data()
                .map_err(|_| CoreSbfError::FinalizedRecord)?;
            let manifest =
                CapabilityManifestV1::decode(&manifest_data).map_err(|_| CoreSbfError::Funding)?;
            validate_ledgers_post(
                route.funding_ledgers,
                manifest_id,
                manifest,
                &rent,
                &ledger_plans,
            )?;
            if route.root.owner != &system_program::ID
                || route.root.data_len() != 0
                || route.root.lamports() != 0
            {
                return Err(CoreSbfError::ChildAck.into());
            }
        }
        _ => return Err(CoreSbfError::Instruction.into()),
    }
    persist_state(market, next_state)
}

impl<'accounts, 'info> Route<'accounts, 'info> {
    fn parse(
        accounts: &'accounts [AccountInfo<'info>],
        physical_count: u8,
    ) -> Result<Self, CoreSbfError> {
        if physical_count == 0 {
            return Err(CoreSbfError::AccountFrame);
        }
        let funding_end = FUNDING_START
            .checked_add(usize::from(physical_count))
            .ok_or(CoreSbfError::Arithmetic)?;
        let fixed_end = funding_end
            .checked_add(ROUTE_FIXED_ACCOUNTS)
            .ok_or(CoreSbfError::Arithmetic)?;
        if accounts.len() < fixed_end {
            return Err(CoreSbfError::AccountFrame);
        }
        Ok(Self {
            market: account(accounts, MARKET)?,
            funding_ledgers: accounts
                .get(FUNDING_START..funding_end)
                .ok_or(CoreSbfError::AccountFrame)?,
            manifest: account(accounts, MANIFEST_RAW)?,
            root: account(accounts, funding_end)?,
            cache: account(accounts, funding_end + 1)?,
            core_program: account(accounts, funding_end + 2)?,
            core_programdata: account(accounts, funding_end + 3)?,
            child_program: account(accounts, funding_end + 4)?,
            child_programdata: account(accounts, funding_end + 5)?,
            resolution_program: account(accounts, funding_end + 6)?,
            resolution_programdata: account(accounts, funding_end + 7)?,
            registry: account(accounts, funding_end + 8)?,
            rent: account(accounts, funding_end + 9)?,
            caller_authority: account(accounts, funding_end + 10)?,
            child_tail: accounts
                .get(fixed_end..)
                .ok_or(CoreSbfError::AccountFrame)?,
        })
    }

    fn validate(&self, program_id: &Pubkey) -> Result<(), CoreSbfError> {
        if self
            .funding_ledgers
            .iter()
            .any(|ledger| ledger.is_signer || ledger.executable)
            || self.manifest.is_signer
            || self.manifest.is_writable
            || self.manifest.executable
            || self.root.is_signer
            || !self.root.is_writable
            || self.root.executable
            || self.cache.is_signer
            || self.cache.is_writable
            || self.cache.executable
            || self.core_program.key != program_id
            || self.core_program.is_signer
            || self.core_program.is_writable
            || !self.core_program.executable
            || self.core_programdata.is_signer
            || self.core_programdata.is_writable
            || self.core_programdata.executable
            || self.child_program.is_signer
            || self.child_program.is_writable
            || !self.child_program.executable
            || self.child_programdata.is_signer
            || self.child_programdata.is_writable
            || self.child_programdata.executable
            || self.resolution_program.is_signer
            || self.resolution_program.is_writable
            || !self.resolution_program.executable
            || self.resolution_programdata.is_signer
            || self.resolution_programdata.is_writable
            || self.resolution_programdata.executable
            || self.registry.is_signer
            || self.registry.is_writable
            || !self.registry.executable
            || self.rent.key != &sysvar::rent::ID
            || self.rent.is_signer
            || self.rent.is_writable
            || self.rent.executable
            || self.caller_authority.is_signer
            || self.caller_authority.is_writable
            || self.caller_authority.executable
            || self.child_tail.iter().any(|value| value.is_signer)
        {
            return Err(CoreSbfError::AccountFrame);
        }
        Ok(())
    }
}

fn validate_selection(
    envelope: CoreEffectEnvelopeV1,
    selection: CapabilityExecutionSelectionV1,
    state: CoreState,
) -> Result<(), CoreSbfError> {
    let expected_action = matches!(
        envelope.action(),
        CoreEffectActionV1::ActivateCapability | CoreEffectActionV1::CloseCapability
    );
    if !expected_action
        || envelope.target_role() != Role::Trading
        || selection.manifest().to_bytes() != state.identity.capability_manifest.to_bytes()
    {
        return Err(CoreSbfError::Instruction);
    }
    Ok(())
}

fn validate_selected_entry(
    selection: CapabilityExecutionSelectionV1,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
) -> Result<(), CoreSbfError> {
    let entry = manifest
        .entry(selection.entry_index())
        .map_err(|_| CoreSbfError::Funding)?;
    if selection.manifest() != manifest_id
        || selection.kind() != entry.kind_id()
        || selection.capability_release() != entry.release_id()
        || selection.config() != entry.config_id()
    {
        return Err(CoreSbfError::Funding);
    }
    Ok(())
}

fn validate_funding_header(
    funding_header: CapabilityFundingHeaderV2,
    selection: CapabilityExecutionSelectionV1,
    manifest: CapabilityManifestV1<'_>,
) -> Result<u16, CoreSbfError> {
    let expected = dependency_closure_mask(manifest, selection.entry_index())?;
    let logical_count =
        u8::try_from(expected.count_ones()).map_err(|_| CoreSbfError::Arithmetic)?;
    if funding_header.logical_count() != logical_count || funding_header.selected_mask() != expected
    {
        return Err(CoreSbfError::Funding);
    }
    Ok(expected)
}

fn dependency_closure_mask(
    manifest: CapabilityManifestV1<'_>,
    selected_entry_index: u16,
) -> Result<u16, CoreSbfError> {
    if selected_entry_index >= manifest.entry_count() {
        return Err(CoreSbfError::Funding);
    }
    let selected_bit = 1_u16
        .checked_shl(u32::from(selected_entry_index))
        .ok_or(CoreSbfError::Arithmetic)?;
    let mut closure = selected_bit;
    loop {
        let before = closure;
        let mut entry_index = 0_u16;
        while entry_index < manifest.entry_count() {
            let entry_bit = 1_u16
                .checked_shl(u32::from(entry_index))
                .ok_or(CoreSbfError::Arithmetic)?;
            if closure & entry_bit != 0 {
                let entry = manifest
                    .entry(entry_index)
                    .map_err(|_| CoreSbfError::Funding)?;
                let mut position = 0_usize;
                while position < usize::from(entry.dependency_count()) {
                    let dependency = entry
                        .dependency(position)
                        .map_err(|_| CoreSbfError::Funding)?;
                    closure |= 1_u16
                        .checked_shl(u32::from(dependency))
                        .ok_or(CoreSbfError::Arithmetic)?;
                    position = position.checked_add(1).ok_or(CoreSbfError::Arithmetic)?;
                }
            }
            entry_index = entry_index.checked_add(1).ok_or(CoreSbfError::Arithmetic)?;
        }
        if closure == before {
            return Ok(closure);
        }
    }
}

struct LedgerTransitionPlan {
    expected_post_bytes: Vec<u8>,
    expected_post_lamports: u64,
    expected_owner: Pubkey,
    closes_ledger: bool,
}

#[allow(clippy::too_many_arguments)]
fn validate_ledgers_pre(
    action: Action,
    ledger_accounts: &[AccountInfo<'_>],
    trading_program: &Pubkey,
    resolution_program: &Pubkey,
    state: CoreState,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    realm: RealmV1,
    rent: &Rent,
    required_union: u16,
    selected_entry_index: u16,
    current_slot: u64,
) -> Result<Vec<LedgerTransitionPlan>, CoreSbfError> {
    let selected_bit = 1_u16
        .checked_shl(u32::from(selected_entry_index))
        .ok_or(CoreSbfError::Arithmetic)?;
    if required_union & selected_bit == 0 {
        return Err(CoreSbfError::Funding);
    }
    let mut ledger_masks = Vec::with_capacity(ledger_accounts.len());
    let mut plans = Vec::with_capacity(ledger_accounts.len());
    for ledger_account in ledger_accounts {
        let ledger_data = ledger_account
            .try_borrow_data()
            .map_err(|_| CoreSbfError::Funding)?;
        let ledger = FundingLedgerV2::decode(&ledger_data).map_err(|_| CoreSbfError::Funding)?;
        let authenticated = ledger
            .authenticate(manifest_id, manifest)
            .map_err(|_| CoreSbfError::Funding)?;
        let ledger_mask = ledger.selected_mask();
        let controller = authenticate_ledger_controller(
            ledger_mask,
            selected_bit,
            ledger_account.is_writable,
            ledger_account.owner,
            trading_program,
            resolution_program,
        )?;
        let selected_ledger = ledger_mask == selected_bit;
        validate_ledger_pda(ledger_account, &controller, state, manifest_id, ledger)?;
        authenticated
            .validate_native_custody(
                ledger_account.lamports(),
                rent.minimum_balance(ledger_data.len()),
                action == Action::CloseCapability && selected_ledger,
            )
            .map_err(|_| CoreSbfError::Funding)?;

        let mut entry_index = 0_u16;
        while entry_index < manifest.entry_count() {
            let entry_bit = 1_u16
                .checked_shl(u32::from(entry_index))
                .ok_or(CoreSbfError::Arithmetic)?;
            if ledger_mask & entry_bit != 0 {
                let entry = manifest
                    .entry(entry_index)
                    .map_err(|_| CoreSbfError::Funding)?;
                validate_realm_binding(entry.funding_quote().realm_collateral(), realm, state)?;
                let slot = authenticated
                    .slot(entry_index)
                    .map_err(|_| CoreSbfError::Funding)?;
                if entry_index == selected_entry_index {
                    match action {
                        Action::ActivateCapability => {
                            if slot.status() != FundingLedgerStatusV2::Pending
                                || entry.activation_policy() != ActivationPolicy::PrepaidLazy
                                || current_slot > entry.activation_deadline_slot()
                                || slot.activation_slot() != 0
                            {
                                return Err(CoreSbfError::Funding);
                            }
                        }
                        Action::CloseCapability => {
                            if slot.status() != FundingLedgerStatusV2::Active
                                || entry.activation_policy() != ActivationPolicy::PrepaidLazy
                            {
                                return Err(CoreSbfError::Funding);
                            }
                        }
                        _ => return Err(CoreSbfError::Instruction),
                    }
                } else if slot.status() != FundingLedgerStatusV2::Active {
                    return Err(CoreSbfError::Funding);
                }
            }
            entry_index = entry_index.checked_add(1).ok_or(CoreSbfError::Arithmetic)?;
        }
        let pre_bytes = ledger_data.to_vec();
        drop(ledger_data);
        let mut expected_post_bytes = pre_bytes.clone();
        let exact_ledger_rent = rent.minimum_balance(pre_bytes.len());
        let mut expected_post_lamports = ledger_account.lamports();
        let mut entry_index = 0_u16;
        while entry_index < manifest.entry_count() {
            let entry_bit = 1_u16
                .checked_shl(u32::from(entry_index))
                .ok_or(CoreSbfError::Arithmetic)?;
            if ledger_mask & entry_bit != 0 && entry_index == selected_entry_index {
                match action {
                    Action::ActivateCapability => {
                        let debit = FundingLedgerV2::activate_in_place(
                            &mut expected_post_bytes,
                            manifest_id,
                            manifest,
                            entry_index,
                            current_slot,
                        )
                        .map_err(|_| CoreSbfError::Funding)?;
                        expected_post_lamports = expected_post_lamports
                            .checked_sub(debit.rent_lamports())
                            .and_then(|value| value.checked_sub(debit.creation_lamports()))
                            .ok_or(CoreSbfError::Funding)?;
                    }
                    Action::CloseCapability => {
                        let close = FundingLedgerV2::close_slot_in_place(
                            &mut expected_post_bytes,
                            manifest_id,
                            manifest,
                            entry_index,
                            FundingLedgerCloseCustodyV2::native_only(
                                expected_post_lamports,
                                exact_ledger_rent,
                                state.rent_beneficiary.to_bytes(),
                            )
                            .map_err(|_| CoreSbfError::Funding)?,
                        )
                        .map_err(|_| CoreSbfError::Funding)?;
                        expected_post_lamports = close.expected_post_ledger_lamports();
                    }
                    _ => return Err(CoreSbfError::Instruction),
                }
            }
            entry_index = entry_index.checked_add(1).ok_or(CoreSbfError::Arithmetic)?;
        }
        require_unselected_slots_unchanged(
            &pre_bytes,
            &expected_post_bytes,
            manifest_id,
            manifest,
            ledger_mask,
            selected_bit,
        )?;
        let closes_ledger = action == Action::CloseCapability
            && FundingLedgerV2::decode(&expected_post_bytes)
                .and_then(|value| value.authenticate(manifest_id, manifest))
                .map_err(|_| CoreSbfError::Funding)?
                .all_closed();
        ledger_masks.push(ledger_mask);
        plans.push(LedgerTransitionPlan {
            expected_post_bytes,
            expected_post_lamports,
            expected_owner: controller,
            closes_ledger,
        });
    }
    validate_funding_ledger_masks_v2(manifest.entry_count(), required_union, &ledger_masks)
        .map_err(|_| CoreSbfError::Funding)?;
    Ok(plans)
}

fn authenticate_ledger_controller(
    ledger_mask: u16,
    selected_bit: u16,
    is_writable: bool,
    owner: &Pubkey,
    trading_program: &Pubkey,
    resolution_program: &Pubkey,
) -> Result<Pubkey, CoreSbfError> {
    let expected = if ledger_mask & selected_bit != 0 {
        if ledger_mask != selected_bit || !is_writable {
            return Err(CoreSbfError::Funding);
        }
        trading_program
    } else {
        if is_writable {
            return Err(CoreSbfError::Funding);
        }
        resolution_program
    };
    if owner != expected {
        return Err(CoreSbfError::Funding);
    }
    Ok(*expected)
}

fn validate_ledger_pda(
    ledger_account: &AccountInfo<'_>,
    controller_program: &Pubkey,
    state: CoreState,
    manifest_id: ContentId,
    ledger: FundingLedgerV2<'_>,
) -> Result<(), CoreSbfError> {
    let derivation = CapabilityFundingLedgerDerivationV2::new(
        controller_program.to_bytes(),
        state.identity.market_id.to_bytes(),
        state.identity.generation,
        manifest_id,
        ledger,
    )
    .map_err(|_| CoreSbfError::Funding)?;
    let expected =
        Pubkey::find_program_address(&derivation.seed_components(), controller_program).0;
    if ledger_account.key != &expected {
        return Err(CoreSbfError::Funding);
    }
    Ok(())
}

fn require_unselected_slots_unchanged(
    pre_bytes: &[u8],
    post_bytes: &[u8],
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    ledger_mask: u16,
    action_mask: u16,
) -> Result<(), CoreSbfError> {
    let pre = FundingLedgerV2::decode(pre_bytes)
        .and_then(|value| value.authenticate(manifest_id, manifest))
        .map_err(|_| CoreSbfError::Funding)?;
    let post = FundingLedgerV2::decode(post_bytes)
        .and_then(|value| value.authenticate(manifest_id, manifest))
        .map_err(|_| CoreSbfError::Funding)?;
    let mut entry_index = 0_u16;
    while entry_index < manifest.entry_count() {
        let entry_bit = 1_u16
            .checked_shl(u32::from(entry_index))
            .ok_or(CoreSbfError::Arithmetic)?;
        if ledger_mask & entry_bit != 0
            && action_mask & entry_bit == 0
            && pre
                .slot_bytes(entry_index)
                .map_err(|_| CoreSbfError::Funding)?
                != post
                    .slot_bytes(entry_index)
                    .map_err(|_| CoreSbfError::Funding)?
        {
            return Err(CoreSbfError::Funding);
        }
        entry_index = entry_index.checked_add(1).ok_or(CoreSbfError::Arithmetic)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_ledgers_post(
    ledger_accounts: &[AccountInfo<'_>],
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    rent: &Rent,
    plans: &[LedgerTransitionPlan],
) -> Result<(), CoreSbfError> {
    if ledger_accounts.len() != plans.len() {
        return Err(CoreSbfError::ChildAck);
    }
    for (ledger_account, plan) in ledger_accounts.iter().zip(plans) {
        if plan.closes_ledger {
            if ledger_account.owner != &system_program::ID
                || ledger_account.data_len() != 0
                || ledger_account.lamports() != 0
            {
                return Err(CoreSbfError::ChildAck);
            }
            continue;
        }
        if ledger_account.owner != &plan.expected_owner
            || ledger_account.data_len() != plan.expected_post_bytes.len()
            || ledger_account.lamports() != plan.expected_post_lamports
        {
            return Err(CoreSbfError::ChildAck);
        }
        let ledger_data = ledger_account
            .try_borrow_data()
            .map_err(|_| CoreSbfError::ChildAck)?;
        if ledger_data.as_ref() != plan.expected_post_bytes.as_slice() {
            return Err(CoreSbfError::ChildAck);
        }
        let ledger = FundingLedgerV2::decode(&ledger_data).map_err(|_| CoreSbfError::ChildAck)?;
        ledger
            .authenticate(manifest_id, manifest)
            .and_then(|authenticated| {
                authenticated.validate_native_custody(
                    ledger_account.lamports(),
                    rent.minimum_balance(ledger_data.len()),
                    false,
                )
            })
            .map_err(|_| CoreSbfError::ChildAck)?;
    }
    Ok(())
}

fn validate_realm_binding(
    binding: Option<dclutch_capability_contract::RealmCollateralBindingV1>,
    realm: RealmV1,
    state: CoreState,
) -> Result<(), CoreSbfError> {
    let Some(binding) = binding else {
        return Ok(());
    };
    if binding.realm_id().to_bytes() != state.identity.realm_id.to_bytes()
        || binding.collateral_release_id().to_bytes() != *realm.collateral_adapter_release_id()
        || binding.token_program() != *realm.token_program()
        || binding.mint() != *realm.collateral_mint()
    {
        return Err(CoreSbfError::Funding);
    }
    Ok(())
}

fn read_state_bytes(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
) -> Result<[u8; STATE_BYTES], CoreSbfError> {
    if market.owner != program_id
        || market.data_len() != STATE_BYTES
        || market.is_signer
        || !market.is_writable
        || market.executable
    {
        return Err(CoreSbfError::Market);
    }
    let data = market.try_borrow_data().map_err(|_| CoreSbfError::Market)?;
    data.as_ref().try_into().map_err(|_| CoreSbfError::Market)
}

fn authenticate_market(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    state: CoreState,
    request: Request,
) -> Result<(), CoreSbfError> {
    if request.market != state.identity.market_id
        || request.generation != state.identity.generation
        || market.key.to_bytes() != state.identity.market_id.to_bytes()
    {
        return Err(CoreSbfError::Market);
    }
    let seeds = MarketCoreStateSeedsV2::new(state.identity);
    let expected = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
    if market.key != &expected {
        return Err(CoreSbfError::Market);
    }
    Ok(())
}

fn validate_envelope(
    program_id: &Pubkey,
    request: Request,
    envelope: CoreEffectEnvelopeV1,
    role_request: &[u8],
    state_bytes: &[u8; STATE_BYTES],
) -> Result<(), CoreSbfError> {
    let expected_action = match request.action {
        Action::ActivateCapability => CoreEffectActionV1::ActivateCapability,
        Action::CloseCapability => CoreEffectActionV1::CloseCapability,
        _ => return Err(CoreSbfError::Instruction),
    };
    envelope
        .validate_role_request(role_request.len(), identity(hash(role_request).to_bytes())?)
        .map_err(|_| CoreSbfError::Instruction)?;
    let state = CoreState::decode(state_bytes).map_err(|_| CoreSbfError::Market)?;
    if envelope.action() != expected_action
        || envelope.target_role() != Role::Trading
        || envelope.caller_program().to_bytes() != program_id.to_bytes()
        || envelope.market() != request.market
        || envelope.release_set().to_bytes() != state.identity.selected_release_set.to_bytes()
        || envelope.generation() != request.generation
        || envelope.parent_state_digest().to_bytes() != hash(state_bytes).to_bytes()
    {
        return Err(CoreSbfError::Instruction);
    }
    Ok(())
}

fn authenticate_caller_authority(
    program_id: &Pubkey,
    route: &Route<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
) -> Result<(), CoreSbfError> {
    let seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| CoreSbfError::CallerAuthority)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
    if route.caller_authority.key != &expected
        || route.caller_authority.key.to_bytes() != envelope.caller_authority().to_bytes()
    {
        return Err(CoreSbfError::CallerAuthority);
    }
    Ok(())
}

fn invoke_child(
    program_id: &Pubkey,
    route: &Route<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
) -> Result<(), ProgramError> {
    let mut data = Vec::with_capacity(envelope_bytes.len().saturating_add(role_request.len()));
    data.extend_from_slice(envelope_bytes);
    data.extend_from_slice(role_request);
    let account_capacity = route
        .funding_ledgers
        .len()
        .saturating_add(route.child_tail.len())
        .saturating_add(4);
    let mut metas = Vec::with_capacity(account_capacity);
    metas.push(AccountMeta::new_readonly(*route.caller_authority.key, true));
    metas.push(AccountMeta::new(*route.root.key, false));
    for ledger in route.funding_ledgers {
        metas.push(if ledger.is_writable {
            AccountMeta::new(*ledger.key, false)
        } else {
            AccountMeta::new_readonly(*ledger.key, false)
        });
    }
    metas.push(AccountMeta::new_readonly(*route.manifest.key, false));
    metas.push(AccountMeta::new_readonly(*route.market.key, false));
    for value in route.child_tail {
        metas.push(if value.is_writable {
            AccountMeta::new(*value.key, false)
        } else {
            AccountMeta::new_readonly(*value.key, false)
        });
    }
    let instruction = Instruction {
        program_id: *route.child_program.key,
        accounts: metas,
        data,
    };
    let authority_seeds = envelope
        .caller_authority_seeds()
        .map_err(|_| CoreSbfError::CallerAuthority)?;
    let (expected, bump) = Pubkey::find_program_address(&authority_seeds.as_slices(), program_id);
    if route.caller_authority.key != &expected {
        return Err(CoreSbfError::CallerAuthority.into());
    }
    let [domain, release, market, role, context, request_digest] = authority_seeds.as_slices();
    let bump_seed = [bump];
    let signer: [&[u8]; 7] = [
        domain,
        release,
        market,
        role,
        context,
        request_digest,
        &bump_seed,
    ];
    let mut infos = Vec::with_capacity(account_capacity.saturating_add(1));
    infos.push(route.caller_authority.clone());
    infos.push(route.root.clone());
    infos.extend(route.funding_ledgers.iter().cloned());
    infos.push(route.manifest.clone());
    infos.push(route.market.clone());
    infos.extend(route.child_tail.iter().cloned());
    infos.push(route.child_program.clone());
    invoke_signed(&instruction, &infos, &[&signer]).map_err(|_| CoreSbfError::ChildCpi.into())
}

fn authenticate_ack(
    route: &Route<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    envelope_bytes: &[u8],
    role_request: &[u8],
) -> Result<(), CoreSbfError> {
    let (producer, bytes) = get_return_data().ok_or(CoreSbfError::ChildAck)?;
    if producer != *route.child_program.key {
        return Err(CoreSbfError::ChildAck);
    }
    let acknowledgement = CoreEffectAckV1::decode(&bytes).map_err(|_| CoreSbfError::ChildAck)?;
    let envelope_len = u32::try_from(envelope_bytes.len()).map_err(|_| CoreSbfError::Arithmetic)?;
    let role_len = u32::try_from(role_request.len()).map_err(|_| CoreSbfError::Arithmetic)?;
    let full_effect_digest = hashv(&[
        &dclutch_market_core_codec::CORE_EFFECT_DIGEST_DOMAIN_V1,
        &envelope_len.to_le_bytes(),
        envelope_bytes,
        &role_len.to_le_bytes(),
        role_request,
    ]);
    acknowledgement
        .validate_for(
            envelope,
            identity(route.child_program.key.to_bytes())?,
            identity(full_effect_digest.to_bytes())?,
        )
        .map_err(|_| CoreSbfError::ChildAck)
}

fn persist_state(account: &AccountInfo<'_>, state: CoreState) -> Result<(), ProgramError> {
    let bytes = state.encode().map_err(|_| CoreSbfError::Commit)?;
    let mut data = account
        .try_borrow_mut_data()
        .map_err(|_| CoreSbfError::Commit)?;
    if data.len() != STATE_BYTES {
        return Err(CoreSbfError::Commit.into());
    }
    data.copy_from_slice(&bytes);
    if CoreState::decode(&data) != Ok(state) {
        return Err(CoreSbfError::Commit.into());
    }
    Ok(())
}

const fn complete_child_effect() -> dclutch_market_core_codec::ChildEffectObservation {
    dclutch_market_core_codec::ChildEffectObservation {
        exact_request_authenticated: true,
        exact_receipt_authenticated: true,
        post_resource_authenticated: true,
    }
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    index: usize,
) -> Result<&'accounts AccountInfo<'info>, CoreSbfError> {
    accounts.get(index).ok_or(CoreSbfError::AccountFrame)
}

const _: usize = CAPABILITY_EXECUTION_SELECTION_BYTES_V1;

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_contract::{
        CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1, FundingAmountsV1,
        FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };

    fn content(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("nonzero content identity")
    }

    fn lazy_quote() -> FundingQuoteV1 {
        let absent = CompartmentFundingV1::not_applicable();
        let amounts = FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(1).expect("rent"),
            CompartmentFundingV1::native_lamports(1).expect("creation"),
            absent,
            absent,
            absent,
            absent,
            absent,
        )
        .expect("amounts");
        FundingQuoteV1::new(amounts, None).expect("quote")
    }

    fn entry(kind: u8, dependency: Option<u8>) -> CapabilityEntryV1 {
        let mut dependencies = [0_u8; MAX_DEPENDENCIES_PER_CAPABILITY];
        let dependency_count = if let Some(dependency) = dependency {
            if let Some(first) = dependencies.first_mut() {
                *first = dependency;
            }
            1
        } else {
            0
        };
        CapabilityEntryV1::new(
            content(kind),
            content(20),
            content(21),
            content(22),
            content(23),
            content(24),
            ActivationPolicy::PrepaidLazy,
            500,
            dependency_count,
            dependencies,
            lazy_quote(),
        )
        .expect("entry")
    }

    #[test]
    fn funding_header_requires_the_exact_transitive_dependency_closure() {
        let entries = [entry(1, Some(1)), entry(2, Some(2)), entry(3, None)];
        let mut storage = [0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        let manifest =
            CapabilityManifestV1::encode_into(&entries, &mut storage).expect("canonical manifest");
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            content(30),
            content(1),
            content(20),
            content(21),
        )
        .expect("selection");

        assert_eq!(dependency_closure_mask(manifest, 0), Ok(0b111));
        assert_eq!(dependency_closure_mask(manifest, 1), Ok(0b110));
        assert_eq!(dependency_closure_mask(manifest, 2), Ok(0b100));
        assert_eq!(
            dependency_closure_mask(manifest, 3),
            Err(CoreSbfError::Funding)
        );

        let exact = CapabilityFundingHeaderV2::new(2, 3, 0b111).expect("exact header");
        assert_eq!(
            validate_funding_header(exact, selection, manifest),
            Ok(0b111)
        );

        let missing_transitive =
            CapabilityFundingHeaderV2::new(1, 2, 0b011).expect("structural header");
        assert_eq!(
            validate_funding_header(missing_transitive, selection, manifest),
            Err(CoreSbfError::Funding)
        );
    }

    #[test]
    fn mixed_controller_ledgers_have_one_exact_privilege_shape() {
        let trading = Pubkey::new_from_array([41; 32]);
        let resolution = Pubkey::new_from_array([42; 32]);
        let unknown = Pubkey::new_from_array([43; 32]);
        let selected = 0b1000;

        assert_eq!(
            authenticate_ledger_controller(
                selected,
                selected,
                true,
                &trading,
                &trading,
                &resolution,
            ),
            Ok(trading)
        );
        assert_eq!(
            authenticate_ledger_controller(
                0b0111,
                selected,
                false,
                &resolution,
                &trading,
                &resolution,
            ),
            Ok(resolution)
        );

        for hostile in [
            authenticate_ledger_controller(
                selected,
                selected,
                false,
                &trading,
                &trading,
                &resolution,
            ),
            authenticate_ledger_controller(
                0b0111,
                selected,
                true,
                &resolution,
                &trading,
                &resolution,
            ),
            authenticate_ledger_controller(0b1111, selected, true, &trading, &trading, &resolution),
            authenticate_ledger_controller(
                0b0111,
                selected,
                false,
                &unknown,
                &trading,
                &resolution,
            ),
            authenticate_ledger_controller(
                selected,
                selected,
                true,
                &resolution,
                &trading,
                &resolution,
            ),
        ] {
            assert_eq!(hostile, Err(CoreSbfError::Funding));
        }
    }
}
