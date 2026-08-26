//! Generic data-defined Trading capability activation and closure.
//!
//! Core authenticates the immutable manifest, the fixed Registry-selected
//! Trading interpreter, and the ordered child-owned FundingState slice. The
//! child remains the sole writer and custody authority for every FundingState;
//! Core commits only its outstanding-capability count after the exact child
//! acknowledgement and all physical postconditions succeed.

use alloc::vec::Vec;

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingDerivationV1,
    CapabilityManifestV1, FUNDING_STATE_BYTES, FundingAmountsV1, FundingCompartment,
    FundingStateV1, FundingStatus,
};
use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    Action, CapabilityChildObservation, CapabilityFundingHeaderV1, CoreEffectAckV1,
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
    release::{authenticate_role, identity},
};

const MARKET: usize = 0;
const REALM_RAW: usize = 1;
const REALM_STAGING: usize = 2;
const MANIFEST_RAW: usize = 3;
const MANIFEST_STAGING: usize = 4;
const FUNDING_START: usize = 5;
const ROUTE_FIXED_ACCOUNTS: usize = 9;

struct Route<'accounts, 'info> {
    market: &'accounts AccountInfo<'info>,
    funding: &'accounts [AccountInfo<'info>],
    manifest: &'accounts AccountInfo<'info>,
    root: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
    child_program: &'accounts AccountInfo<'info>,
    child_programdata: &'accounts AccountInfo<'info>,
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
    funding_header: CapabilityFundingHeaderV1,
) -> Result<(), ProgramError> {
    require_distinct(accounts)?;
    let route = Route::parse(accounts, funding_header.funding_count())?;
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
    let now = Clock::get().map_err(|_| CoreSbfError::Funding)?.slot;
    validate_funding_pre(
        request.action,
        route.funding,
        route.child_program.key,
        state,
        manifest_id,
        manifest,
        realm,
        &rent,
        selection.entry_index(),
        now,
    )?;

    let core_admission = authenticate_role(
        route.cache,
        route.registry,
        route.core_program,
        route.core_programdata,
        state.identity.registry_program,
        state.identity.selected_release_set.to_bytes(),
        Role::Core,
    )?;
    if core_admission.receipt.observed.program.to_bytes() != program_id.to_bytes() {
        return Err(CoreSbfError::Release.into());
    }
    let child_admission = authenticate_role(
        route.cache,
        route.registry,
        route.child_program,
        route.child_programdata,
        state.identity.registry_program,
        state.identity.selected_release_set.to_bytes(),
        Role::Trading,
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
            validate_funding_activated(
                route.funding,
                route.child_program.key,
                state,
                manifest_id,
                manifest,
                &rent,
                selection.entry_index(),
            )?;
            if route.root.owner != route.child_program.key || route.root.data_len() == 0 {
                return Err(CoreSbfError::ChildAck.into());
            }
        }
        Action::CloseCapability => {
            for funding in route.funding {
                if funding.owner != &system_program::ID
                    || funding.data_len() != 0
                    || funding.lamports() != 0
                {
                    return Err(CoreSbfError::ChildAck.into());
                }
            }
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
        funding_count: u8,
    ) -> Result<Self, CoreSbfError> {
        let funding_end = FUNDING_START
            .checked_add(usize::from(funding_count))
            .ok_or(CoreSbfError::Arithmetic)?;
        let fixed_end = funding_end
            .checked_add(ROUTE_FIXED_ACCOUNTS)
            .ok_or(CoreSbfError::Arithmetic)?;
        if accounts.len() < fixed_end {
            return Err(CoreSbfError::AccountFrame);
        }
        Ok(Self {
            market: account(accounts, MARKET)?,
            funding: accounts
                .get(FUNDING_START..funding_end)
                .ok_or(CoreSbfError::AccountFrame)?,
            manifest: account(accounts, MANIFEST_RAW)?,
            root: account(accounts, funding_end)?,
            cache: account(accounts, funding_end + 1)?,
            core_program: account(accounts, funding_end + 2)?,
            core_programdata: account(accounts, funding_end + 3)?,
            child_program: account(accounts, funding_end + 4)?,
            child_programdata: account(accounts, funding_end + 5)?,
            registry: account(accounts, funding_end + 6)?,
            rent: account(accounts, funding_end + 7)?,
            caller_authority: account(accounts, funding_end + 8)?,
            child_tail: accounts
                .get(fixed_end..)
                .ok_or(CoreSbfError::AccountFrame)?,
        })
    }

    fn validate(&self, program_id: &Pubkey) -> Result<(), CoreSbfError> {
        if self
            .funding
            .iter()
            .any(|value| value.is_signer || !value.is_writable || value.executable)
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

#[allow(clippy::too_many_arguments)]
fn validate_funding_pre(
    action: Action,
    funding_accounts: &[AccountInfo<'_>],
    child_program: &Pubkey,
    state: CoreState,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    realm: RealmV1,
    rent: &Rent,
    selected_entry_index: u16,
    current_slot: u64,
) -> Result<(), CoreSbfError> {
    let mut previous: Option<u16> = None;
    let mut selected_present = false;
    for account in funding_accounts {
        let funding = decode_funding(account, child_program)?;
        let entry_index = funding.entry_index();
        if previous.is_some_and(|value| value >= entry_index) {
            return Err(CoreSbfError::Funding);
        }
        if entry_index == selected_entry_index {
            selected_present = true;
        }
        validate_funding_pda(
            account,
            child_program,
            state,
            manifest_id,
            manifest,
            funding,
        )?;
        let entry = manifest
            .entry(entry_index)
            .map_err(|_| CoreSbfError::Funding)?;
        validate_realm_binding(entry.funding_quote().realm_collateral(), realm, state)?;
        match action {
            Action::ActivateCapability => {
                if funding.status() != FundingStatus::Pending
                    || entry.activation_policy() != ActivationPolicy::PrepaidLazy
                    || current_slot > entry.activation_deadline_slot()
                    || funding.activation_slot() != 0
                    || funding.remaining() != entry.funding_quote().amounts()
                    || funding.released() != FundingAmountsV1::default()
                {
                    return Err(CoreSbfError::Funding);
                }
                require_native_balance(account, funding, rent, false)?;
            }
            Action::CloseCapability => {
                if funding.status() != FundingStatus::Active
                    || entry.activation_policy() != ActivationPolicy::PrepaidLazy
                    || !funding_conserves(funding, entry.funding_quote().amounts())?
                {
                    return Err(CoreSbfError::Funding);
                }
                require_native_balance(account, funding, rent, true)?;
            }
            _ => return Err(CoreSbfError::Instruction),
        }
        previous = Some(entry_index);
    }
    if !selected_present {
        return Err(CoreSbfError::Funding);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_funding_activated(
    funding_accounts: &[AccountInfo<'_>],
    child_program: &Pubkey,
    state: CoreState,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    rent: &Rent,
    selected_entry_index: u16,
) -> Result<(), CoreSbfError> {
    let mut previous: Option<u16> = None;
    let mut selected_present = false;
    for account in funding_accounts {
        let funding = decode_funding(account, child_program)?;
        let entry_index = funding.entry_index();
        if previous.is_some_and(|value| value >= entry_index) {
            return Err(CoreSbfError::ChildAck);
        }
        if entry_index == selected_entry_index {
            selected_present = true;
        }
        validate_funding_pda(
            account,
            child_program,
            state,
            manifest_id,
            manifest,
            funding,
        )?;
        let quote = manifest
            .entry(entry_index)
            .map_err(|_| CoreSbfError::ChildAck)?
            .funding_quote()
            .amounts();
        if funding.status() != FundingStatus::Active
            || !funding_conserves(funding, quote)?
            || funding.remaining().rent().amount() != 0
            || funding.remaining().creation().amount() != 0
            || funding.released().rent() != quote.rent()
            || funding.released().creation() != quote.creation()
        {
            return Err(CoreSbfError::ChildAck);
        }
        require_native_balance(account, funding, rent, false)
            .map_err(|_| CoreSbfError::ChildAck)?;
        previous = Some(entry_index);
    }
    if !selected_present {
        return Err(CoreSbfError::ChildAck);
    }
    Ok(())
}

fn decode_funding(
    account: &AccountInfo<'_>,
    child_program: &Pubkey,
) -> Result<FundingStateV1, CoreSbfError> {
    if account.owner != child_program || account.data_len() != FUNDING_STATE_BYTES {
        return Err(CoreSbfError::Funding);
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| CoreSbfError::Funding)?;
    let funding = FundingStateV1::decode(&data).map_err(|_| CoreSbfError::Funding)?;
    if funding.to_bytes().as_slice() != data.as_ref() {
        return Err(CoreSbfError::Funding);
    }
    Ok(funding)
}

fn validate_funding_pda(
    account: &AccountInfo<'_>,
    child_program: &Pubkey,
    state: CoreState,
    manifest_id: ContentId,
    manifest: CapabilityManifestV1<'_>,
    funding: FundingStateV1,
) -> Result<(), CoreSbfError> {
    if funding.manifest_content_id() != manifest_id {
        return Err(CoreSbfError::Funding);
    }
    let derivation = CapabilityFundingDerivationV1::new(
        state.identity.market_id.to_bytes(),
        state.identity.generation,
        manifest_id,
        manifest,
        funding,
    )
    .map_err(|_| CoreSbfError::Funding)?;
    let expected = Pubkey::find_program_address(&derivation.seed_components(), child_program).0;
    if account.key != &expected {
        return Err(CoreSbfError::Funding);
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

fn funding_conserves(
    funding: FundingStateV1,
    quote: FundingAmountsV1,
) -> Result<bool, CoreSbfError> {
    for compartment in [
        FundingCompartment::Rent,
        FundingCompartment::Creation,
        FundingCompartment::Work,
        FundingCompartment::Provider,
        FundingCompartment::Bounty,
        FundingCompartment::Liquidity,
        FundingCompartment::Service,
    ] {
        let expected = quote.compartment(compartment);
        let remaining = funding.remaining().compartment(compartment);
        let released = funding.released().compartment(compartment);
        if remaining
            .amount()
            .checked_add(released.amount())
            .ok_or(CoreSbfError::Arithmetic)?
            != expected.amount()
            || (remaining.amount() != 0 && remaining.asset_class() != expected.asset_class())
            || (released.amount() != 0 && released.asset_class() != expected.asset_class())
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn require_native_balance(
    account: &AccountInfo<'_>,
    funding: FundingStateV1,
    rent: &Rent,
    admit_donation: bool,
) -> Result<(), CoreSbfError> {
    let expected = rent
        .minimum_balance(FUNDING_STATE_BYTES)
        .checked_add(funding.remaining().native_lamports_total())
        .ok_or(CoreSbfError::Arithmetic)?;
    if (admit_donation && account.lamports() < expected)
        || (!admit_donation && account.lamports() != expected)
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
        .funding
        .len()
        .saturating_add(route.child_tail.len())
        .saturating_add(4);
    let mut metas = Vec::with_capacity(account_capacity);
    metas.push(AccountMeta::new_readonly(*route.caller_authority.key, true));
    metas.push(AccountMeta::new(*route.root.key, false));
    for funding in route.funding {
        metas.push(AccountMeta::new(*funding.key, false));
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
    infos.extend(route.funding.iter().cloned());
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
