#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![allow(unexpected_cfgs)]

//! Authenticated SBF adapter for the one canonical Claims economic owner.

use dclutch_claims_representation_codec::{ACTION_WIRE_BYTES_V1, ActionV1};
use dclutch_claims_svm::{
    CallerRole, ClaimsAction, ClaimsAggregateSeedsV1, ClaimsPlanV1, ClaimsPositionSeedsV1,
    ClaimsReceiptV1, NO_POSITION_REVISION,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::CUSTODY_REQUEST_BYTES_V1;
use dclutch_economic_slice_kernel::{
    BasketAction, BasketFrame, MARKET_HEADER_BYTES, POSITION_HEADER_BYTES, Phase as EconomicPhase,
    SCALAR_BYTES, execute_basket, initialize_market, initialize_position, market_identity,
    market_outcome_count, market_phase, market_registry_program, market_release_set_id,
    market_revision, position_market_id, position_owner, position_revision,
};
use dclutch_market_core_codec::{
    CORE_EFFECT_DIGEST_DOMAIN_V1, CORE_EFFECT_ENVELOPE_BYTES_V1, CORE_EFFECT_MAGIC_V1,
    CoreEffectAckV1, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState, Identity,
    MarketCoreStateSeedsV1, Phase as CorePhase, Role, STATE_BYTES,
};
use dclutch_registry_svm::{AuthenticatedRoleReceiptV1, RegistryInstructionV1};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    hash::{hash, hashv},
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke, invoke_signed, set_return_data},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::{allocate, assign};

mod representation;

entrypoint!(process_instruction);

/// Generic Claims child account index: caller authority PDA signer.
pub const AUTHORITY_ACCOUNT: usize = 0;
/// Generic Claims child account index: aggregate Claims Market.
pub const MARKET_ACCOUNT: usize = 1;
/// Generic Claims child account index: source Position or Claims-program sentinel.
pub const SOURCE_POSITION_ACCOUNT: usize = 2;
/// Generic Claims child account index: destination Position or Claims-program sentinel.
pub const DESTINATION_POSITION_ACCOUNT: usize = 3;
/// Generic Claims child account index: Registry activation cache.
pub const ACTIVATION_CACHE_ACCOUNT: usize = 4;
/// Generic Claims child account index: selected caller program.
pub const CALLER_PROGRAM_ACCOUNT: usize = 5;
/// Generic Claims child account index: current caller ProgramData.
pub const CALLER_PROGRAMDATA_ACCOUNT: usize = 6;
/// Generic Claims child account index: current Claims program.
pub const CLAIMS_PROGRAM_ACCOUNT: usize = 7;
/// Generic Claims child account index: current Claims ProgramData.
pub const CLAIMS_PROGRAMDATA_ACCOUNT: usize = 8;
/// Generic Claims child account index: immutable Market-selected Registry program.
pub const REGISTRY_PROGRAM_ACCOUNT: usize = 9;
/// Generic Claims child account index: canonical Core Market state.
pub const CORE_MARKET_ACCOUNT: usize = 10;
/// Generic Claims child account index: current Core program.
pub const CORE_PROGRAM_ACCOUNT: usize = 11;
/// Generic Claims child account index: current Core ProgramData.
pub const CORE_PROGRAMDATA_ACCOUNT: usize = 12;
/// Exact generic Claims child account count.
pub const GENERIC_ACCOUNT_COUNT: usize = 13;
/// Foundational Split account index: Rent sysvar.
pub const FOUNDATIONAL_RENT_ACCOUNT: usize = 13;
/// Foundational Split account index: System program.
pub const FOUNDATIONAL_SYSTEM_ACCOUNT: usize = 14;
/// Exact foundational Split Claims child account count.
pub const FOUNDATIONAL_ACCOUNT_COUNT: usize = 15;

/// Unified representation state PDA seed prefix.
pub const REPRESENTATION_STATE_SEED_V1: &[u8] = b"dclutch:representation:v1";
/// Exact permissioned-burn representation Mint width.
pub const REPRESENTATION_MINT_BYTES_V1: usize = 238;
/// Unified representation account index: claimant signer.
pub const REPRESENTATION_CLAIMANT_ACCOUNT: usize = 0;
/// Unified representation account index: immutable descriptor.
pub const REPRESENTATION_DESCRIPTOR_ACCOUNT: usize = 1;
/// Unified representation account index: wrapper state/controller.
pub const REPRESENTATION_STATE_ACCOUNT: usize = 2;
/// Unified representation account index: aggregate Claims Market.
pub const REPRESENTATION_MARKET_ACCOUNT: usize = 3;
/// Unified representation account index: claimant Position.
pub const REPRESENTATION_CLAIMANT_POSITION_ACCOUNT: usize = 4;
/// Unified representation account index: wrapper Position.
pub const REPRESENTATION_WRAPPER_POSITION_ACCOUNT: usize = 5;
/// Unified representation account index: Registry activation cache.
pub const REPRESENTATION_ACTIVATION_CACHE_ACCOUNT: usize = 6;
/// Unified representation account index: current Claims program.
pub const REPRESENTATION_CLAIMS_PROGRAM_ACCOUNT: usize = 7;
/// Unified representation account index: Claims ProgramData.
pub const REPRESENTATION_CLAIMS_PROGRAMDATA_ACCOUNT: usize = 8;
/// Unified representation account index: Market-selected Registry program.
pub const REPRESENTATION_REGISTRY_PROGRAM_ACCOUNT: usize = 9;
/// Unified representation account index: permissioned-burn Token-2022 Mint.
pub const REPRESENTATION_MINT_ACCOUNT: usize = 10;
/// Unified representation account index: claimant holder Token-2022 account.
pub const REPRESENTATION_HOLDER_TOKEN_ACCOUNT: usize = 11;
/// Unified representation account index: Token-2022 program.
pub const REPRESENTATION_TOKEN_PROGRAM_ACCOUNT: usize = 12;
/// Unified representation account index: canonical Core Market state.
pub const REPRESENTATION_CORE_MARKET_ACCOUNT: usize = 13;
/// Unified representation account index: current Core program.
pub const REPRESENTATION_CORE_PROGRAM_ACCOUNT: usize = 14;
/// Unified representation account index: current Core ProgramData.
pub const REPRESENTATION_CORE_PROGRAMDATA_ACCOUNT: usize = 15;
/// Exact unified representation account count before terminal Custody composition.
pub const REPRESENTATION_ACCOUNT_COUNT: usize = 16;
/// Terminal representation account index: Claims release-pinned Custody caller authority.
pub const REPRESENTATION_CUSTODY_CALLER_AUTHORITY_ACCOUNT: usize = 16;
/// Terminal representation account index: current Custody program.
pub const REPRESENTATION_CUSTODY_PROGRAM_ACCOUNT: usize = 17;
/// Terminal representation account index: current Custody ProgramData.
pub const REPRESENTATION_CUSTODY_PROGRAMDATA_ACCOUNT: usize = 18;
/// Terminal representation account index: finalized immutable Realm record.
pub const REPRESENTATION_REALM_ACCOUNT: usize = 19;
/// Terminal representation account index: canonical per-descriptor Custody replay.
pub const REPRESENTATION_CUSTODY_REPLAY_ACCOUNT: usize = 20;
/// Terminal representation account index: Realm-selected collateral Mint.
pub const REPRESENTATION_COLLATERAL_MINT_ACCOUNT: usize = 21;
/// Terminal representation account index: canonical Market Hoard vault.
pub const REPRESENTATION_HOARD_VAULT_ACCOUNT: usize = 22;
/// Terminal representation account index: claimant's external collateral account.
pub const REPRESENTATION_COLLATERAL_RECIPIENT_ACCOUNT: usize = 23;
/// Terminal representation account index: canonical Custody transfer authority.
pub const REPRESENTATION_CUSTODY_TRANSFER_AUTHORITY_ACCOUNT: usize = 24;
/// Terminal representation account index: Realm-selected collateral token program.
pub const REPRESENTATION_COLLATERAL_TOKEN_PROGRAM_ACCOUNT: usize = 25;
/// Exact unified representation account count with terminal Custody composition.
pub const REPRESENTATION_TERMINAL_ACCOUNT_COUNT: usize = 26;

struct GenericAccounts<'accounts, 'info> {
    authority: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    source: &'accounts AccountInfo<'info>,
    destination: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    caller_program: &'accounts AccountInfo<'info>,
    caller_programdata: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
}

struct AppliedClaims {
    payout: u64,
    post_market_revision: u64,
    post_source_revision: u64,
    post_destination_revision: u64,
    post_resource_digest: [u8; 32],
}

impl<'accounts, 'info> GenericAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        let accounts = accounts
            .get(..GENERIC_ACCOUNT_COUNT)
            .ok_or(ClaimsSbfError::Accounts)?;
        let [
            authority,
            market,
            source,
            destination,
            cache,
            caller_program,
            caller_programdata,
            claims_program,
            claims_programdata,
            registry,
            core_market,
            core_program,
            core_programdata,
        ] = accounts
        else {
            return Err(ClaimsSbfError::Accounts.into());
        };
        Ok(Self {
            authority,
            market,
            source,
            destination,
            cache,
            caller_program,
            caller_programdata,
            claims_program,
            claims_programdata,
            registry,
            core_market,
            core_program,
            core_programdata,
        })
    }
}

struct RepresentationAccounts<'accounts, 'info> {
    claimant: &'accounts AccountInfo<'info>,
    descriptor: &'accounts AccountInfo<'info>,
    state: &'accounts AccountInfo<'info>,
    market: &'accounts AccountInfo<'info>,
    claimant_position: &'accounts AccountInfo<'info>,
    wrapper_position: &'accounts AccountInfo<'info>,
    cache: &'accounts AccountInfo<'info>,
    claims_program: &'accounts AccountInfo<'info>,
    claims_programdata: &'accounts AccountInfo<'info>,
    registry: &'accounts AccountInfo<'info>,
    mint: &'accounts AccountInfo<'info>,
    holder_token: &'accounts AccountInfo<'info>,
    token_program: &'accounts AccountInfo<'info>,
    core_market: &'accounts AccountInfo<'info>,
    core_program: &'accounts AccountInfo<'info>,
    core_programdata: &'accounts AccountInfo<'info>,
}

impl<'accounts, 'info> RepresentationAccounts<'accounts, 'info> {
    fn parse(accounts: &'accounts [AccountInfo<'info>]) -> Result<Self, ProgramError> {
        let accounts = accounts
            .get(..REPRESENTATION_ACCOUNT_COUNT)
            .ok_or(ClaimsSbfError::Accounts)?;
        let [
            claimant,
            descriptor,
            state,
            market,
            claimant_position,
            wrapper_position,
            cache,
            claims_program,
            claims_programdata,
            registry,
            mint,
            holder_token,
            token_program,
            core_market,
            core_program,
            core_programdata,
        ] = accounts
        else {
            return Err(ClaimsSbfError::Accounts.into());
        };
        Ok(Self {
            claimant,
            descriptor,
            state,
            market,
            claimant_position,
            wrapper_position,
            cache,
            claims_program,
            claims_programdata,
            registry,
            mint,
            holder_token,
            token_program,
            core_market,
            core_program,
            core_programdata,
        })
    }
}

/// Stable adapter refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimsSbfError {
    /// Instruction bytes were hostile or selected no supported family.
    Instruction = 0,
    /// Account count, privileges, owners, or executable flags were wrong.
    Accounts = 1,
    /// Market or Position semantic identities did not join the packet.
    Identity = 2,
    /// Registry receipt or current deployment authentication failed.
    Release = 3,
    /// Caller PDA authority did not authenticate the packet.
    Authority = 4,
    /// Claims economic transition refused.
    Economic = 5,
    /// This action requires the canonical Custody child composition.
    CustodyRequired = 6,
    /// Receipt construction or post-state commitment failed.
    Receipt = 7,
    /// Representation descriptor/state or unified wrapper transition refused.
    Representation = 8,
    /// Token-2022 mint/account profile or CPI refused.
    Token = 9,
}

impl From<ClaimsSbfError> for ProgramError {
    fn from(value: ClaimsSbfError) -> Self {
        Self::Custom(value as u32)
    }
}

/// Process one exact Claims instruction.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.get(..CORE_EFFECT_MAGIC_V1.len()) == Some(CORE_EFFECT_MAGIC_V1.as_slice()) {
        return process_core_effect(program_id, accounts, instruction_data);
    }
    if let Ok(plan) = ClaimsPlanV1::decode(instruction_data) {
        return process_generic_plan(program_id, accounts, instruction_data, plan);
    }
    if instruction_data.len() == ACTION_WIRE_BYTES_V1 {
        let action = ActionV1::decode(instruction_data).map_err(|_| ClaimsSbfError::Instruction)?;
        return representation::process(program_id, accounts, action, instruction_data, None);
    }
    let terminal_wire_bytes = ACTION_WIRE_BYTES_V1
        .checked_add(CUSTODY_REQUEST_BYTES_V1)
        .ok_or(ClaimsSbfError::Instruction)?;
    if instruction_data.len() != terminal_wire_bytes {
        return Err(ClaimsSbfError::Instruction.into());
    }
    let (action_bytes, custody_bytes) = instruction_data.split_at(ACTION_WIRE_BYTES_V1);
    let action = ActionV1::decode(action_bytes).map_err(|_| ClaimsSbfError::Instruction)?;
    representation::process(
        program_id,
        accounts,
        action,
        action_bytes,
        Some(custody_bytes),
    )
}

fn process_generic_plan(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
    plan: ClaimsPlanV1<'_>,
) -> ProgramResult {
    if accounts.len() != GENERIC_ACCOUNT_COUNT {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let accounts = GenericAccounts::parse(accounts)?;
    authenticate_generic_privileges(program_id, &accounts, plan)?;
    let packet_digest = hash(instruction_data).to_bytes();
    authenticate_authority(&accounts, plan, packet_digest)?;
    authenticate_releases(&accounts, plan)?;
    authenticate_economic_accounts(program_id, &accounts, plan, false)?;
    let basket_action = match plan.action() {
        ClaimsAction::TransferNative => BasketAction::TransferNative,
        ClaimsAction::Materialize => BasketAction::Materialize,
        ClaimsAction::Dematerialize => BasketAction::Dematerialize,
        ClaimsAction::RedeemNativeTerminal => BasketAction::RedeemNativeTerminal,
        ClaimsAction::RedeemMaterializedTerminal => BasketAction::RedeemMaterializedTerminal,
        ClaimsAction::MintCompleteSet => BasketAction::MintCompleteSet,
        ClaimsAction::MergeCompleteSet => BasketAction::MergeCompleteSet,
        ClaimsAction::InitializeCompleteSet => return Err(ClaimsSbfError::Instruction.into()),
    };
    let applied = execute_plan_economics(&accounts, plan, basket_action)?;
    let receipt = ClaimsReceiptV1::new(
        plan,
        packet_digest,
        program_id.to_bytes(),
        applied.post_market_revision,
        applied.post_source_revision,
        applied.post_destination_revision,
        applied.payout,
        applied.post_resource_digest,
    )
    .map_err(|_| ClaimsSbfError::Receipt)?;
    set_return_data(&receipt.to_bytes());
    Ok(())
}

fn process_core_effect(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() <= CORE_EFFECT_ENVELOPE_BYTES_V1 {
        return Err(ClaimsSbfError::Instruction.into());
    }
    let (envelope_bytes, request_bytes) = instruction_data.split_at(CORE_EFFECT_ENVELOPE_BYTES_V1);
    let envelope =
        CoreEffectEnvelopeV1::decode(envelope_bytes).map_err(|_| ClaimsSbfError::Instruction)?;
    let plan = ClaimsPlanV1::decode(request_bytes).map_err(|_| ClaimsSbfError::Instruction)?;
    let request_digest = identity(hash(request_bytes).to_bytes())?;
    envelope
        .validate_role_request(request_bytes.len(), request_digest)
        .map_err(|_| ClaimsSbfError::Instruction)?;
    if envelope.target_role() != Role::Claims
        || envelope.caller_program().to_bytes()
            != account_infos
                .get(CALLER_PROGRAM_ACCOUNT)
                .ok_or(ClaimsSbfError::Accounts)?
                .key
                .to_bytes()
        || envelope.caller_authority().to_bytes()
            != account_infos
                .get(AUTHORITY_ACCOUNT)
                .ok_or(ClaimsSbfError::Accounts)?
                .key
                .to_bytes()
        || envelope.release_set().to_bytes() != plan.release_set_id()
        || envelope.market().to_bytes() != plan.market()
        || envelope.context().to_bytes() != plan.request_id()
        || plan.caller_role() != CallerRole::Core
        || envelope.expected_resource_a_revision() != plan.expected_market_revision()
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let basket_action = match (envelope.action(), plan.action()) {
        (CoreEffectActionV1::InitializeClaims, ClaimsAction::InitializeCompleteSet) => {
            if envelope.expected_resource_b_revision() != plan.expected_destination_revision() {
                return Err(ClaimsSbfError::Identity.into());
            }
            BasketAction::MintCompleteSet
        }
        (CoreEffectActionV1::SplitClaims, ClaimsAction::MintCompleteSet) => {
            if envelope.expected_resource_b_revision() != plan.expected_destination_revision() {
                return Err(ClaimsSbfError::Identity.into());
            }
            BasketAction::MintCompleteSet
        }
        (CoreEffectActionV1::RedeemClaims, ClaimsAction::RedeemNativeTerminal) => {
            if envelope.expected_resource_b_revision() != plan.expected_source_revision() {
                return Err(ClaimsSbfError::Identity.into());
            }
            BasketAction::RedeemNativeTerminal
        }
        _ => return Err(ClaimsSbfError::Instruction.into()),
    };
    let foundational = matches!(
        (envelope.action(), plan.action()),
        (
            CoreEffectActionV1::InitializeClaims,
            ClaimsAction::InitializeCompleteSet
        )
    );
    let expected_account_count = if foundational {
        FOUNDATIONAL_ACCOUNT_COUNT
    } else {
        GENERIC_ACCOUNT_COUNT
    };
    if account_infos.len() != expected_account_count {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let accounts = GenericAccounts::parse(account_infos)?;
    authenticate_generic_privileges(program_id, &accounts, plan)?;
    authenticate_core_authority(&accounts, envelope)?;
    authenticate_releases(&accounts, plan)?;
    if foundational {
        let creation =
            prepare_foundational_split(program_id, account_infos, &accounts, envelope, plan)?;
        apply_foundational_split(program_id, account_infos, &accounts, plan, creation)?;
    }
    authenticate_economic_accounts(program_id, &accounts, plan, foundational)?;
    let applied = execute_plan_economics(&accounts, plan, basket_action)?;
    let envelope_length = u32::try_from(envelope_bytes.len())
        .map_err(|_| ClaimsSbfError::Instruction)?
        .to_le_bytes();
    let request_length = u32::try_from(request_bytes.len())
        .map_err(|_| ClaimsSbfError::Instruction)?
        .to_le_bytes();
    let full_effect_digest = identity(
        hashv(&[
            &CORE_EFFECT_DIGEST_DOMAIN_V1,
            &envelope_length,
            envelope_bytes,
            &request_length,
            request_bytes,
        ])
        .to_bytes(),
    )?;
    let post_holder_revision = match envelope.action() {
        CoreEffectActionV1::InitializeClaims | CoreEffectActionV1::SplitClaims => {
            applied.post_destination_revision
        }
        CoreEffectActionV1::RedeemClaims => applied.post_source_revision,
        _ => return Err(ClaimsSbfError::Instruction.into()),
    };
    let acknowledgement = CoreEffectAckV1::new(
        envelope.action(),
        Role::Claims,
        identity(program_id.to_bytes())?,
        envelope.release_set(),
        envelope.market(),
        envelope.context(),
        full_effect_digest,
        identity(applied.post_resource_digest)?,
        envelope.expected_resource_a_revision(),
        applied.post_market_revision,
        envelope.expected_resource_b_revision(),
        post_holder_revision,
    )
    .map_err(|_| ClaimsSbfError::Receipt)?;
    set_return_data(
        &acknowledgement
            .encode()
            .map_err(|_| ClaimsSbfError::Receipt)?,
    );
    Ok(())
}

fn identity(bytes: [u8; 32]) -> Result<Identity, ProgramError> {
    Identity::new(bytes).map_err(|_| ClaimsSbfError::Identity.into())
}

fn authenticate_core_authority(
    accounts: &GenericAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
) -> ProgramResult {
    let seeds = CallerAuthoritySeedsV1::new(
        content_id(envelope.release_set().to_bytes())?,
        envelope.market().to_bytes(),
        ExecutionRoleV1::Core,
        envelope.context().to_bytes(),
        envelope.role_request_digest().to_bytes(),
    )
    .map_err(|_| ClaimsSbfError::Authority)?;
    let (expected, _) =
        Pubkey::find_program_address(&seeds.as_slices(), accounts.caller_program.key);
    if accounts.authority.key != &expected
        || envelope.caller_authority().to_bytes() != expected.to_bytes()
    {
        return Err(ClaimsSbfError::Authority.into());
    }
    Ok(())
}

fn content_id(bytes: [u8; 32]) -> Result<ContentId, ProgramError> {
    ContentId::new(bytes).map_err(|_| ClaimsSbfError::Identity.into())
}

#[derive(Clone, Copy)]
struct FoundationalCreation {
    aggregate_width: usize,
    position_width: usize,
}

fn prepare_foundational_split(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'_>],
    accounts: &GenericAccounts<'_, '_>,
    envelope: CoreEffectEnvelopeV1,
    plan: ClaimsPlanV1<'_>,
) -> Result<FoundationalCreation, ProgramError> {
    if envelope.action() != CoreEffectActionV1::InitializeClaims
        || plan.action() != ClaimsAction::InitializeCompleteSet
        || plan.caller_role() != CallerRole::Core
        || plan.expected_market_revision() != 0
        || plan.expected_source_revision() != NO_POSITION_REVISION
        || plan.expected_destination_revision() != 0
    {
        return Err(ClaimsSbfError::Instruction.into());
    }
    let core = authenticate_core_market(
        program_id,
        accounts.core_market,
        accounts.core_program,
        accounts.market,
        plan.market(),
        plan.release_set_id(),
    )?;
    if core.phase != CorePhase::Founding
        || core.identity.generation != envelope.generation()
        || envelope.context().to_bytes() != plan.request_id()
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let rent = account_infos
        .get(FOUNDATIONAL_RENT_ACCOUNT)
        .ok_or(ClaimsSbfError::Accounts)?;
    let system = account_infos
        .get(FOUNDATIONAL_SYSTEM_ACCOUNT)
        .ok_or(ClaimsSbfError::Accounts)?;
    if rent.key != &sysvar::rent::ID
        || rent.is_signer
        || rent.is_writable
        || rent.executable
        || system.key != &system_program::ID
        || system.is_signer
        || system.is_writable
        || !system.executable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    let aggregate_width = runtime_account_width(MARKET_HEADER_BYTES, plan.outcome_count(), 3)?;
    let position_width = runtime_account_width(POSITION_HEADER_BYTES, plan.outcome_count(), 2)?;
    let rent_value = Rent::from_account_info(rent).map_err(|_| ClaimsSbfError::Accounts)?;
    authenticate_vacant_prepaid(accounts.market, rent_value.minimum_balance(aggregate_width))?;
    authenticate_vacant_prepaid(
        accounts.destination,
        rent_value.minimum_balance(position_width),
    )?;
    let destination_seeds = ClaimsPositionSeedsV1::new(plan.market(), plan.destination_owner())
        .map_err(|_| ClaimsSbfError::Identity)?;
    if accounts.destination.key
        != &Pubkey::find_program_address(&destination_seeds.as_slices(), program_id).0
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(FoundationalCreation {
        aggregate_width,
        position_width,
    })
}

fn apply_foundational_split<'info>(
    program_id: &Pubkey,
    account_infos: &[AccountInfo<'info>],
    accounts: &GenericAccounts<'_, 'info>,
    plan: ClaimsPlanV1<'_>,
    creation: FoundationalCreation,
) -> ProgramResult {
    let system = account_infos
        .get(FOUNDATIONAL_SYSTEM_ACCOUNT)
        .ok_or(ClaimsSbfError::Accounts)?;
    allocate_aggregate(
        program_id,
        accounts.market,
        system,
        plan.market(),
        creation.aggregate_width,
    )?;
    allocate_position(
        program_id,
        accounts.destination,
        system,
        plan.market(),
        plan.destination_owner(),
        creation.position_width,
    )?;
    {
        let mut aggregate = accounts
            .market
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        if aggregate.len() != creation.aggregate_width || aggregate.iter().any(|byte| *byte != 0) {
            return Err(ClaimsSbfError::Accounts.into());
        }
        initialize_market(
            &mut aggregate,
            plan.market(),
            plan.release_set_id(),
            accounts.registry.key.to_bytes(),
            plan.outcome_count(),
            EconomicPhase::Open,
            0,
        )
        .map_err(|_| ClaimsSbfError::Economic)?;
    }
    {
        let mut destination = accounts
            .destination
            .try_borrow_mut_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        if destination.len() != creation.position_width || destination.iter().any(|byte| *byte != 0)
        {
            return Err(ClaimsSbfError::Accounts.into());
        }
        initialize_position(
            &mut destination,
            plan.market(),
            plan.destination_owner(),
            plan.outcome_count(),
        )
        .map_err(|_| ClaimsSbfError::Economic)?;
    }
    Ok(())
}

fn authenticate_vacant_prepaid(account: &AccountInfo<'_>, minimum_lamports: u64) -> ProgramResult {
    if account.owner != &system_program::ID
        || account.data_len() != 0
        || account.lamports() < minimum_lamports
        || account.is_signer
        || !account.is_writable
        || account.executable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

fn runtime_account_width(
    header: usize,
    outcome_count: u32,
    vector_count: usize,
) -> Result<usize, ProgramError> {
    usize::try_from(outcome_count)
        .ok()
        .and_then(|count| count.checked_mul(vector_count))
        .and_then(|count| count.checked_mul(SCALAR_BYTES))
        .and_then(|tail| header.checked_add(tail))
        .ok_or_else(|| ClaimsSbfError::Accounts.into())
}

fn allocate_aggregate<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    market: [u8; 32],
    width: usize,
) -> ProgramResult {
    let seeds = ClaimsAggregateSeedsV1::new(market).map_err(|_| ClaimsSbfError::Identity)?;
    let [domain, market_seed] = seeds.as_slices();
    let bump = Pubkey::find_program_address(&seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    allocate_and_assign(
        program_id,
        account,
        system,
        width,
        &[domain, market_seed, &bump_seed],
    )
}

fn allocate_position<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    market: [u8; 32],
    owner: [u8; 32],
    width: usize,
) -> ProgramResult {
    let seeds = ClaimsPositionSeedsV1::new(market, owner).map_err(|_| ClaimsSbfError::Identity)?;
    let [domain, market_seed, owner_seed] = seeds.as_slices();
    let bump = Pubkey::find_program_address(&seeds.as_slices(), program_id).1;
    let bump_seed = [bump];
    allocate_and_assign(
        program_id,
        account,
        system,
        width,
        &[domain, market_seed, owner_seed, &bump_seed],
    )
}

fn allocate_and_assign<'info>(
    program_id: &Pubkey,
    account: &AccountInfo<'info>,
    system: &AccountInfo<'info>,
    width: usize,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    let space = u64::try_from(width).map_err(|_| ClaimsSbfError::Accounts)?;
    invoke_signed(
        &allocate(account.key, space),
        &[account.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| ClaimsSbfError::Accounts)?;
    invoke_signed(
        &assign(account.key, program_id),
        &[account.clone(), system.clone()],
        &[signer_seeds],
    )
    .map_err(|_| ClaimsSbfError::Accounts)?;
    if account.owner != program_id || account.data_len() != width {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

fn execute_plan_economics(
    accounts: &GenericAccounts<'_, '_>,
    plan: ClaimsPlanV1<'_>,
    action: BasketAction,
) -> Result<AppliedClaims, ProgramError> {
    let source_present = plan.expected_source_revision() != NO_POSITION_REVISION;
    let destination_present = plan.expected_destination_revision() != NO_POSITION_REVISION;
    let mut market_data = accounts
        .market
        .try_borrow_mut_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let frame = BasketFrame {
        expected_market_revision: plan.expected_market_revision(),
        expected_source_revision: source_present.then_some(plan.expected_source_revision()),
        expected_destination_revision: destination_present
            .then_some(plan.expected_destination_revision()),
        action,
        quantities: plan.quantities_bytes(),
        quantity_multiplier: 1,
    };
    let payout = match (source_present, destination_present) {
        (true, true) => {
            let mut source_data = accounts
                .source
                .try_borrow_mut_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            let mut destination_data = accounts
                .destination
                .try_borrow_mut_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            execute_basket(
                &mut market_data,
                Some(&mut source_data),
                Some(&mut destination_data),
                frame,
            )
        }
        (true, false) => {
            let mut source_data = accounts
                .source
                .try_borrow_mut_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            execute_basket(&mut market_data, Some(&mut source_data), None, frame)
        }
        (false, true) => {
            let mut destination_data = accounts
                .destination
                .try_borrow_mut_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            execute_basket(&mut market_data, None, Some(&mut destination_data), frame)
        }
        (false, false) => return Err(ClaimsSbfError::Accounts.into()),
    }
    .map_err(|_| ClaimsSbfError::Economic)?;
    let post_market_revision =
        market_revision(&market_data).map_err(|_| ClaimsSbfError::Receipt)?;
    drop(market_data);
    let outcome_count = plan.outcome_count();
    let post_source_revision = if source_present {
        let source_data = accounts
            .source
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        position_revision(&source_data, outcome_count).map_err(|_| ClaimsSbfError::Receipt)?
    } else {
        NO_POSITION_REVISION
    };
    let post_destination_revision = if destination_present {
        let destination_data = accounts
            .destination
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        position_revision(&destination_data, outcome_count).map_err(|_| ClaimsSbfError::Receipt)?
    } else {
        NO_POSITION_REVISION
    };
    Ok(AppliedClaims {
        payout: payout.amount,
        post_market_revision,
        post_source_revision,
        post_destination_revision,
        post_resource_digest: resource_digest(accounts, source_present, destination_present)?,
    })
}

fn authenticate_generic_privileges(
    program_id: &Pubkey,
    accounts: &GenericAccounts<'_, '_>,
    plan: ClaimsPlanV1<'_>,
) -> ProgramResult {
    let authority = accounts.authority;
    let market = accounts.market;
    let source = accounts.source;
    let destination = accounts.destination;
    let cache = accounts.cache;
    let caller_program = accounts.caller_program;
    let caller_programdata = accounts.caller_programdata;
    let claims_program = accounts.claims_program;
    let claims_programdata = accounts.claims_programdata;
    let registry = accounts.registry;
    let core_market = accounts.core_market;
    let core_program = accounts.core_program;
    let core_programdata = accounts.core_programdata;
    if !authority.is_signer
        || authority.is_writable
        || !market.is_writable
        || market.is_signer
        || cache.is_writable
        || cache.is_signer
        || !caller_program.executable
        || caller_program.is_writable
        || caller_program.is_signer
        || caller_programdata.is_writable
        || caller_programdata.is_signer
        || !claims_program.executable
        || claims_program.is_writable
        || claims_program.is_signer
        || claims_program.key != program_id
        || claims_programdata.is_writable
        || claims_programdata.is_signer
        || !registry.executable
        || registry.is_writable
        || registry.is_signer
        || core_market.is_writable
        || core_market.is_signer
        || core_market.executable
        || !core_program.executable
        || core_program.is_writable
        || core_program.is_signer
        || core_programdata.is_writable
        || core_programdata.is_signer
        || core_programdata.executable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    authenticate_position_privilege(
        source,
        claims_program,
        plan.expected_source_revision() != NO_POSITION_REVISION,
    )?;
    authenticate_position_privilege(
        destination,
        claims_program,
        plan.expected_destination_revision() != NO_POSITION_REVISION,
    )
}

fn authenticate_position_privilege(
    position: &AccountInfo<'_>,
    claims_program: &AccountInfo<'_>,
    present: bool,
) -> ProgramResult {
    if present {
        if !position.is_writable || position.is_signer || position.executable {
            return Err(ClaimsSbfError::Accounts.into());
        }
    } else if position.key != claims_program.key
        || position.is_writable
        || position.is_signer
        || !position.executable
    {
        return Err(ClaimsSbfError::Accounts.into());
    }
    Ok(())
}

fn authenticate_authority(
    accounts: &GenericAccounts<'_, '_>,
    plan: ClaimsPlanV1<'_>,
    role_request_digest: [u8; 32],
) -> ProgramResult {
    let caller_program = accounts.caller_program.key;
    let caller_role = match plan.caller_role() {
        CallerRole::Core => ExecutionRoleV1::Core,
        CallerRole::Trading => ExecutionRoleV1::Trading,
    };
    let seeds = CallerAuthoritySeedsV1::new(
        content_id(plan.release_set_id())?,
        plan.market(),
        caller_role,
        plan.request_id(),
        role_request_digest,
    )
    .map_err(|_| ClaimsSbfError::Authority)?;
    let (expected, _) = Pubkey::find_program_address(&seeds.as_slices(), caller_program);
    if accounts.authority.key != &expected {
        return Err(ClaimsSbfError::Authority.into());
    }
    Ok(())
}

fn authenticate_releases(
    accounts: &GenericAccounts<'_, '_>,
    plan: ClaimsPlanV1<'_>,
) -> ProgramResult {
    let caller_role = match plan.caller_role() {
        CallerRole::Core => ExecutionRoleV1::Core,
        CallerRole::Trading => ExecutionRoleV1::Trading,
    };
    let caller = reauthenticate(
        accounts.registry,
        accounts.cache,
        caller_role,
        accounts.caller_program,
        accounts.caller_programdata,
    )?;
    let claims = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Claims,
        accounts.claims_program,
        accounts.claims_programdata,
    )?;
    let core = reauthenticate(
        accounts.registry,
        accounts.cache,
        ExecutionRoleV1::Core,
        accounts.core_program,
        accounts.core_programdata,
    )?;
    for receipt in [caller, claims, core] {
        if receipt.execution_release_set_id().as_bytes() != &plan.release_set_id() {
            return Err(ClaimsSbfError::Release.into());
        }
    }
    Ok(())
}

pub(crate) fn reauthenticate<'info>(
    registry: &AccountInfo<'info>,
    cache: &AccountInfo<'info>,
    role: ExecutionRoleV1,
    program: &AccountInfo<'info>,
    programdata: &AccountInfo<'info>,
) -> Result<AuthenticatedRoleReceiptV1, ProgramError> {
    let instruction = Instruction {
        program_id: *registry.key,
        accounts: Vec::from([
            AccountMeta::new_readonly(*cache.key, false),
            AccountMeta::new_readonly(*program.key, false),
            AccountMeta::new_readonly(*programdata.key, false),
        ]),
        data: RegistryInstructionV1::Reauthenticate(role)
            .to_bytes()
            .to_vec(),
    };
    invoke(
        &instruction,
        &[
            cache.clone(),
            program.clone(),
            programdata.clone(),
            registry.clone(),
        ],
    )
    .map_err(|_| ClaimsSbfError::Release)?;
    let (producer, bytes) = get_return_data().ok_or(ClaimsSbfError::Release)?;
    if producer != *registry.key {
        return Err(ClaimsSbfError::Release.into());
    }
    let receipt =
        AuthenticatedRoleReceiptV1::decode(&bytes).map_err(|_| ClaimsSbfError::Release)?;
    if receipt.role() != role || receipt.program().as_bytes() != &program.key.to_bytes() {
        return Err(ClaimsSbfError::Release.into());
    }
    Ok(receipt)
}

fn authenticate_economic_accounts(
    program_id: &Pubkey,
    accounts: &GenericAccounts<'_, '_>,
    plan: ClaimsPlanV1<'_>,
    foundational: bool,
) -> ProgramResult {
    let market = accounts.market;
    let core = authenticate_core_market(
        program_id,
        accounts.core_market,
        accounts.core_program,
        market,
        plan.market(),
        plan.release_set_id(),
    )?;
    if market.owner != program_id {
        return Err(ClaimsSbfError::Identity.into());
    }
    let market_data = market
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let economic_phase = market_phase(&market_data).map_err(|_| ClaimsSbfError::Identity)?;
    let lifecycle_joined = if foundational {
        core.phase == CorePhase::Founding && economic_phase == EconomicPhase::Open
    } else {
        phases_join(core.phase, core.terminal_winner, economic_phase)
    };
    if market_identity(&market_data).map_err(|_| ClaimsSbfError::Identity)? != plan.market()
        || market_release_set_id(&market_data).map_err(|_| ClaimsSbfError::Identity)?
            != plan.release_set_id()
        || market_registry_program(&market_data).map_err(|_| ClaimsSbfError::Identity)?
            != accounts.registry.key.to_bytes()
        || market_outcome_count(&market_data).map_err(|_| ClaimsSbfError::Identity)?
            != plan.outcome_count()
        || market_revision(&market_data).map_err(|_| ClaimsSbfError::Identity)?
            != plan.expected_market_revision()
        || !lifecycle_joined
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    drop(market_data);
    authenticate_position(
        program_id,
        accounts.source,
        plan.market(),
        plan.source_owner(),
        plan.outcome_count(),
        plan.expected_source_revision(),
    )?;
    authenticate_position(
        program_id,
        accounts.destination,
        plan.market(),
        plan.destination_owner(),
        plan.outcome_count(),
        plan.expected_destination_revision(),
    )
}

pub(crate) fn authenticate_core_market(
    claims_program: &Pubkey,
    core_market: &AccountInfo<'_>,
    core_program: &AccountInfo<'_>,
    claims_aggregate: &AccountInfo<'_>,
    logical_market: [u8; 32],
    release_set: [u8; 32],
) -> Result<CoreState, ProgramError> {
    if core_market.owner != core_program.key
        || core_market.key.to_bytes() != logical_market
        || !core_program.executable
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let data = core_market
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if data.len() != STATE_BYTES {
        return Err(ClaimsSbfError::Identity.into());
    }
    let state = CoreState::decode(&data).map_err(|_| ClaimsSbfError::Identity)?;
    let seeds = MarketCoreStateSeedsV1::new(state.identity);
    let expected = Pubkey::find_program_address(&seeds.as_slices(), core_program.key).0;
    let aggregate_seeds =
        ClaimsAggregateSeedsV1::new(logical_market).map_err(|_| ClaimsSbfError::Identity)?;
    let expected_aggregate =
        Pubkey::find_program_address(&aggregate_seeds.as_slices(), claims_program).0;
    if core_market.key != &expected
        || state.identity.market_id.to_bytes() != logical_market
        || state.identity.selected_release_set.to_bytes() != release_set
        || claims_aggregate.key != &expected_aggregate
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(state)
}

pub(crate) const fn phases_join(core: CorePhase, winner: u32, claims: EconomicPhase) -> bool {
    match (core, claims) {
        (CorePhase::Open, EconomicPhase::Open) | (CorePhase::Retired, EconomicPhase::Retired) => {
            true
        }
        (CorePhase::Terminal, EconomicPhase::Terminal(claims_winner))
        | (CorePhase::Retiring, EconomicPhase::Retiring(claims_winner)) => winner == claims_winner,
        _ => false,
    }
}

fn authenticate_position(
    program_id: &Pubkey,
    account: &AccountInfo<'_>,
    market: [u8; 32],
    expected_owner: [u8; 32],
    outcome_count: u32,
    expected_revision: u64,
) -> ProgramResult {
    if expected_revision == NO_POSITION_REVISION {
        return Ok(());
    }
    if account.owner != program_id {
        return Err(ClaimsSbfError::Identity.into());
    }
    let seeds =
        ClaimsPositionSeedsV1::new(market, expected_owner).map_err(|_| ClaimsSbfError::Identity)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
    if account.key != &expected {
        return Err(ClaimsSbfError::Identity.into());
    }
    let data = account
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if position_market_id(&data, outcome_count).map_err(|_| ClaimsSbfError::Identity)? != market
        || position_owner(&data, outcome_count).map_err(|_| ClaimsSbfError::Identity)?
            != expected_owner
        || position_revision(&data, outcome_count).map_err(|_| ClaimsSbfError::Identity)?
            != expected_revision
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

fn resource_digest(
    accounts: &GenericAccounts<'_, '_>,
    source_present: bool,
    destination_present: bool,
) -> Result<[u8; 32], ProgramError> {
    let market = accounts
        .market
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    match (source_present, destination_present) {
        (true, true) => {
            let source = accounts
                .source
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            let destination = accounts
                .destination
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            Ok(hashv(&[&market, &source, &destination]).to_bytes())
        }
        (true, false) => {
            let source = accounts
                .source
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            Ok(hashv(&[&market, &source]).to_bytes())
        }
        (false, true) => {
            let destination = accounts
                .destination
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            Ok(hashv(&[&market, &destination]).to_bytes())
        }
        (false, false) => Err(ClaimsSbfError::Accounts.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::{boxed::Box, vec::Vec};

    use dclutch_economic_slice_kernel::{
        MARKET_HEADER_BYTES, POSITION_HEADER_BYTES, Phase, SCALAR_BYTES, initialize_market,
        initialize_position,
    };
    use dclutch_market_core_codec::{MarketIdentity, Readiness};

    use super::*;

    fn account(
        key: Pubkey,
        data: Vec<u8>,
        owner: Pubkey,
        signer: bool,
        writable: bool,
        executable: bool,
    ) -> AccountInfo<'static> {
        account_with_lamports(key, data, owner, signer, writable, executable, 1)
    }

    fn account_with_lamports(
        key: Pubkey,
        data: Vec<u8>,
        owner: Pubkey,
        signer: bool,
        writable: bool,
        executable: bool,
        lamports: u64,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(lamports)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn quantities(values: &[u64]) -> Vec<u8> {
        let mut output = Vec::new();
        for value in values {
            output.extend_from_slice(&value.to_le_bytes());
        }
        output
    }

    fn fixture() -> Result<([AccountInfo<'static>; GENERIC_ACCOUNT_COUNT], Vec<u8>), ProgramError> {
        let program_id = Pubkey::new_from_array([9; 32]);
        let market_key = Pubkey::new_from_array([2; 32]);
        let registry_key = Pubkey::new_from_array([8; 32]);
        let release_set = [1; 32];
        let count = 3_u32;
        let mut market = vec![0_u8; MARKET_HEADER_BYTES + 3 * 3 * SCALAR_BYTES];
        let mut source = vec![0_u8; POSITION_HEADER_BYTES + 3 * 2 * SCALAR_BYTES];
        let mut destination = vec![0_u8; POSITION_HEADER_BYTES + 3 * 2 * SCALAR_BYTES];
        initialize_market(
            &mut market,
            market_key.to_bytes(),
            release_set,
            registry_key.to_bytes(),
            count,
            Phase::Open,
            0,
        )
        .map_err(|_| ClaimsSbfError::Economic)?;
        initialize_position(&mut source, market_key.to_bytes(), [4; 32], count)
            .map_err(|_| ClaimsSbfError::Economic)?;
        initialize_position(&mut destination, market_key.to_bytes(), [5; 32], count)
            .map_err(|_| ClaimsSbfError::Economic)?;
        let complete = quantities(&[10, 10, 10]);
        execute_basket(
            &mut market,
            None,
            Some(&mut source),
            BasketFrame {
                expected_market_revision: 0,
                expected_source_revision: None,
                expected_destination_revision: Some(0),
                action: BasketAction::MintCompleteSet,
                quantities: &complete,
                quantity_multiplier: 1,
            },
        )
        .map_err(|_| ClaimsSbfError::Economic)?;
        let claims_program = account(
            program_id,
            Vec::new(),
            Pubkey::new_from_array([99; 32]),
            false,
            false,
            true,
        );
        let accounts = [
            account(
                Pubkey::new_unique(),
                Vec::new(),
                Pubkey::new_unique(),
                true,
                false,
                false,
            ),
            account(market_key, market, program_id, false, true, false),
            account(Pubkey::new_unique(), source, program_id, false, true, false),
            account(
                Pubkey::new_unique(),
                destination,
                program_id,
                false,
                true,
                false,
            ),
            account(
                Pubkey::new_unique(),
                Vec::new(),
                Pubkey::new_unique(),
                false,
                false,
                false,
            ),
            account(
                Pubkey::new_unique(),
                Vec::new(),
                Pubkey::new_unique(),
                false,
                false,
                true,
            ),
            account(
                Pubkey::new_unique(),
                Vec::new(),
                Pubkey::new_unique(),
                false,
                false,
                false,
            ),
            claims_program,
            account(
                Pubkey::new_unique(),
                Vec::new(),
                Pubkey::new_unique(),
                false,
                false,
                false,
            ),
            account(
                registry_key,
                Vec::new(),
                Pubkey::new_unique(),
                false,
                false,
                true,
            ),
            account(
                Pubkey::new_unique(),
                Vec::new(),
                Pubkey::new_unique(),
                false,
                false,
                false,
            ),
            account(
                Pubkey::new_unique(),
                Vec::new(),
                Pubkey::new_unique(),
                false,
                false,
                true,
            ),
            account(
                Pubkey::new_unique(),
                Vec::new(),
                Pubkey::new_unique(),
                false,
                false,
                false,
            ),
        ];
        Ok((accounts, release_set.to_vec()))
    }

    fn transfer_plan<'a>(
        release_set: [u8; 32],
        quantities: &'a [u8],
    ) -> Result<ClaimsPlanV1<'a>, ProgramError> {
        ClaimsPlanV1::new(
            ClaimsAction::TransferNative,
            CallerRole::Trading,
            release_set,
            [2; 32],
            [3; 32],
            [4; 32],
            [5; 32],
            1,
            1,
            0,
            3,
            quantities,
        )
        .map_err(|_| ClaimsSbfError::Instruction.into())
    }

    fn semantic_id(byte: u8) -> Result<Identity, ProgramError> {
        Identity::new([byte; 32]).map_err(|_| ClaimsSbfError::Identity.into())
    }

    fn core_state(market: Identity) -> Result<CoreState, ProgramError> {
        Ok(CoreState {
            phase: CorePhase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity: MarketIdentity {
                market_id: market,
                realm_id: semantic_id(11)?,
                product_id: semantic_id(12)?,
                result_domain: semantic_id(13)?,
                resolution_policy: semantic_id(14)?,
                capability_manifest: semantic_id(15)?,
                selected_release_set: semantic_id(1)?,
                registry_program: semantic_id(8)?,
                generation: 1,
            },
            outstanding_capabilities: 1,
            rent_beneficiary: semantic_id(16)?,
            terminal_receipt: None,
        })
    }

    #[test]
    fn authenticated_runtime_basket_advances_once() -> Result<(), ProgramError> {
        let (accounts, release) = fixture()?;
        let view = GenericAccounts::parse(&accounts)?;
        let quantities = quantities(&[3, 0, 2]);
        let release_set: [u8; 32] = release
            .as_slice()
            .try_into()
            .map_err(|_| ClaimsSbfError::Instruction)?;
        let applied = execute_plan_economics(
            &view,
            transfer_plan(release_set, &quantities)?,
            BasketAction::TransferNative,
        )?;
        assert_eq!(applied.post_market_revision, 2);
        assert_eq!(applied.post_source_revision, 2);
        assert_eq!(applied.post_destination_revision, 1);
        assert_eq!(applied.payout, 0);
        Ok(())
    }

    #[test]
    fn late_runtime_coordinate_refusal_preserves_every_account_byte() -> Result<(), ProgramError> {
        let (accounts, release) = fixture()?;
        let before_market = accounts
            .get(MARKET_ACCOUNT)
            .ok_or(ClaimsSbfError::Accounts)?
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?
            .to_vec();
        let before_source = accounts
            .get(SOURCE_POSITION_ACCOUNT)
            .ok_or(ClaimsSbfError::Accounts)?
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?
            .to_vec();
        let before_destination = accounts
            .get(DESTINATION_POSITION_ACCOUNT)
            .ok_or(ClaimsSbfError::Accounts)?
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?
            .to_vec();
        let view = GenericAccounts::parse(&accounts)?;
        let quantities = quantities(&[3, 0, 11]);
        let release_set: [u8; 32] = release
            .as_slice()
            .try_into()
            .map_err(|_| ClaimsSbfError::Instruction)?;
        assert!(
            execute_plan_economics(
                &view,
                transfer_plan(release_set, &quantities)?,
                BasketAction::TransferNative,
            )
            .is_err()
        );
        assert_eq!(
            accounts
                .get(MARKET_ACCOUNT)
                .ok_or(ClaimsSbfError::Accounts)?
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?
                .as_ref(),
            before_market
        );
        assert_eq!(
            accounts
                .get(SOURCE_POSITION_ACCOUNT)
                .ok_or(ClaimsSbfError::Accounts)?
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?
                .as_ref(),
            before_source
        );
        assert_eq!(
            accounts
                .get(DESTINATION_POSITION_ACCOUNT)
                .ok_or(ClaimsSbfError::Accounts)?
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?
                .as_ref(),
            before_destination
        );
        Ok(())
    }

    #[test]
    fn core_join_refuses_substituted_claims_aggregate() -> Result<(), ProgramError> {
        let claims_program = Pubkey::new_from_array([9; 32]);
        let core_program = Pubkey::new_from_array([10; 32]);
        let provisional_market = semantic_id(2)?;
        let provisional_state = core_state(provisional_market)?;
        let state_seeds = MarketCoreStateSeedsV1::new(provisional_state.identity);
        let market = Pubkey::find_program_address(&state_seeds.as_slices(), &core_program).0;
        let state = core_state(identity(market.to_bytes())?)?;
        let final_seeds = MarketCoreStateSeedsV1::new(state.identity);
        let final_market = Pubkey::find_program_address(&final_seeds.as_slices(), &core_program).0;
        if final_market != market {
            return Err(ClaimsSbfError::Identity.into());
        }
        let aggregate_seeds =
            ClaimsAggregateSeedsV1::new(market.to_bytes()).map_err(|_| ClaimsSbfError::Identity)?;
        let aggregate =
            Pubkey::find_program_address(&aggregate_seeds.as_slices(), &claims_program).0;
        let core_program_account = account(
            core_program,
            Vec::new(),
            Pubkey::new_unique(),
            false,
            false,
            true,
        );
        let core_market = account(
            market,
            state
                .encode()
                .map_err(|_| ClaimsSbfError::Identity)?
                .to_vec(),
            core_program,
            false,
            false,
            false,
        );
        let aggregate_account = account(aggregate, Vec::new(), claims_program, false, true, false);
        assert_eq!(
            authenticate_core_market(
                &claims_program,
                &core_market,
                &core_program_account,
                &aggregate_account,
                market.to_bytes(),
                [1; 32],
            ),
            Ok(state)
        );
        let substituted = account(
            Pubkey::new_unique(),
            Vec::new(),
            claims_program,
            false,
            true,
            false,
        );
        assert_eq!(
            authenticate_core_market(
                &claims_program,
                &core_market,
                &core_program_account,
                &substituted,
                market.to_bytes(),
                [1; 32],
            ),
            Err(ClaimsSbfError::Identity.into())
        );
        Ok(())
    }

    #[test]
    fn foundational_creation_accepts_dust_but_refuses_implicit_authority() {
        let minimum = 100;
        let vacant_with_dust = account_with_lamports(
            Pubkey::new_unique(),
            Vec::new(),
            system_program::ID,
            false,
            true,
            false,
            minimum + 7,
        );
        assert_eq!(
            authenticate_vacant_prepaid(&vacant_with_dust, minimum),
            Ok(())
        );

        let underfunded = account_with_lamports(
            Pubkey::new_unique(),
            Vec::new(),
            system_program::ID,
            false,
            true,
            false,
            minimum - 1,
        );
        assert_eq!(
            authenticate_vacant_prepaid(&underfunded, minimum),
            Err(ClaimsSbfError::Accounts.into())
        );
        let caller_signer = account_with_lamports(
            Pubkey::new_unique(),
            Vec::new(),
            system_program::ID,
            true,
            true,
            false,
            minimum,
        );
        assert_eq!(
            authenticate_vacant_prepaid(&caller_signer, minimum),
            Err(ClaimsSbfError::Accounts.into())
        );
        let preinitialized = account_with_lamports(
            Pubkey::new_unique(),
            vec![0],
            system_program::ID,
            false,
            true,
            false,
            minimum,
        );
        assert_eq!(
            authenticate_vacant_prepaid(&preinitialized, minimum),
            Err(ClaimsSbfError::Accounts.into())
        );
    }

    #[test]
    fn runtime_width_is_derived_from_the_product_outcome_count() -> Result<(), ProgramError> {
        assert_eq!(
            runtime_account_width(MARKET_HEADER_BYTES, 3, 3)?,
            MARKET_HEADER_BYTES + 3 * 3 * SCALAR_BYTES
        );
        assert_eq!(
            runtime_account_width(POSITION_HEADER_BYTES, 3, 2)?,
            POSITION_HEADER_BYTES + 3 * 2 * SCALAR_BYTES
        );
        Ok(())
    }
}
