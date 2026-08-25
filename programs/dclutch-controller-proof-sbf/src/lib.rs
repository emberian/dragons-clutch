#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Controller membrane compiling signed Direct intents into child plans.

extern crate std;

mod generated_direct_program;

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1,
};
use dclutch_core_contract::{MARKET_ROOT_BYTES, MarketRoot, Phase};
use dclutch_direct_codec::{
    COMPACT_INTENT_BYTES, COMPILED_DIRECT_CAPACITY_ID_V1, COMPILED_DIRECT_CHILD_SCHEMA_ID_V1,
    COMPILED_DIRECT_DERIVATION_ID_V1, COMPILED_DIRECT_RELEASE_ID_V1, CONTROLLER_INSTRUCTION_BYTES,
    CompactIntentV1, ControllerInstructionV1, REGISTERED_CLAIM_FILL_BYTES,
    REGISTERED_CONTROLLER_INSTRUCTION_BYTES, RegisteredFillInstructionV1, RegisteredIntentStateV1,
    registered_claim_fill_instruction,
};
use dclutch_direct_contract::{
    DIRECT_CAPABILITY_KIND_ID_V2, PRICE_SCALE, VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
    VenueFeePolicyV3,
};
use dclutch_market_contract::market::{MARKET_ROOT_OFFSET, decode_market_outcome_count};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_PDA_DOMAIN, RealmV1,
};
use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;
use dclutch_token_svm::{
    ACCOUNT_BYTES, COption, ExactTransferProfileV1, LEGACY_TOKEN_PROGRAM_ID,
    PRODUCTION_ADAPTER_RELEASES,
};
use dclutch_transition_vm::Registers;
use generated_direct_program::{
    CLAIM_BUYER_FILL_OFFSET, CLAIM_BUYER_NONCE_OFFSET, CLAIM_BUYER_OUTCOME_OFFSET,
    CLAIM_PLAN_BYTES, CLAIM_PLAN_TEMPLATE, CLAIM_SELLER_FILL_OFFSET, CLAIM_SELLER_NONCE_OFFSET,
    CLAIM_SELLER_OUTCOME_OFFSET, CUSTODY_FEE_OFFSET, CUSTODY_GROSS_OFFSET, CUSTODY_PLAN_BYTES,
    CUSTODY_PLAN_TEMPLATE, DIRECT_PROGRAM, IDENTITY_COUNT, SCALAR_BUYER_NONCE_OUTPUT,
    SCALAR_FEE_OUTPUT, SCALAR_FILL, SCALAR_GROSS_OUTPUT, SCALAR_SELLER_NONCE_OUTPUT,
    direct_input_identities, direct_input_scalars,
};
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use solana_sdk_ids::{ed25519_program, sysvar};

/// PDA seed defining the controller authority namespace.
pub const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
/// Canonical compiled-Direct maker replay-root domain.
pub const REPLAY_SEED: &[u8] = b"dclutch/direct-replay/v3";
/// Canonical registered-intent PDA domain.
pub const REGISTERED_SEED: &[u8] = b"dclutch/direct-registered/v1";
/// Canonical maker/outcome Position domain for the successor experiment.
pub const POSITION_SEED: &[u8] = b"dclutch/position/v1";
/// Canonical Market PDA domain shared with the protocol adapter.
pub const MARKET_SEED: &[u8] = b"dclutch/market-root/v1";
/// Exact-account claim proof-program identity used by this experiment.
pub const CLAIM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
/// Real custody proof-program identity used by this experiment.
pub const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([75_u8; 32]);
/// Bytes in the canonical controller journal.
pub const JOURNAL_BYTES: usize = 16;
const JOURNAL_MAGIC: &[u8; 4] = b"DCCJ";
const REPLAY_STATE_BYTES: usize = 48;
const POSITION_STATE_BYTES: usize = 56;
const ED25519_DESCRIPTOR_BYTES: usize = 14;
const ED25519_PAYLOAD_START: usize = 2 + 2 * ED25519_DESCRIPTOR_BYTES;
const ED25519_INSTRUCTION_BYTES: usize = ED25519_PAYLOAD_START + 2 * 96;
const SELLER_INTENT_OFFSET: usize = 32;
const BUYER_INTENT_OFFSET: usize = SELLER_INTENT_OFFSET + COMPACT_INTENT_BYTES;

/// Stable controller experiment refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControllerError {
    /// Account count or order was not canonical.
    AccountFrame = 0,
    /// Account privilege, owner, executable state, or aliasing was invalid.
    AccountAuthority = 1,
    /// The named claim or custody program was not the pinned child.
    ChildProgram = 2,
    /// The controller PDA did not match the runtime program and supplied bump.
    ControllerPda = 3,
    /// A replay PDA did not match exact Market/generation/maker coordinates.
    ReplayPda = 4,
    /// Controller journal bytes were not canonical or could not be borrowed.
    Journal = 5,
    /// Journal counter overflowed.
    JournalOverflow = 6,
    /// Controller instruction bytes were not canonical.
    Instruction = 7,
    /// Compact signed-intent bytes were not canonical.
    Intent = 8,
    /// Native Ed25519 instruction evidence was not exact.
    Signature = 9,
    /// Market, Realm, capability manifest, or fee policy authority was invalid.
    MarketAuthority = 10,
    /// Canonical replay or Position state bytes/binding were not exact.
    ClaimState = 11,
    /// Token account state did not match the signed/profile bindings.
    TokenState = 12,
    /// The Lean-owned transition program refused the authenticated frame.
    Transition = 13,
    /// Checked controller arithmetic overflowed.
    Arithmetic = 14,
}

impl From<ControllerError> for ProgramError {
    fn from(error: ControllerError) -> Self {
        Self::Custom(error as u32)
    }
}

#[derive(Clone, Copy)]
struct ReplayState {
    nonce: u64,
}

#[derive(Clone, Copy)]
struct PositionState {
    outcome: u64,
    claims: u64,
}

#[derive(Clone, Copy)]
struct ClaimState {
    seller_nonce: u64,
    buyer_nonce: u64,
    seller_claims: u64,
    buyer_claims: u64,
}

#[derive(Clone, Copy)]
struct MarketAuthority {
    phase: u8,
    outcome_count: u8,
    generation: u64,
    fee_basis_points: u16,
    collateral_mint: [u8; 32],
    fee_recipient: [u8; 32],
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Dispatch one exact inline or registered Direct controller request.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.len() {
        CONTROLLER_INSTRUCTION_BYTES => process_inline(program_id, accounts, instruction_data),
        REGISTERED_CONTROLLER_INSTRUCTION_BYTES => {
            process_registered_fill(program_id, accounts, instruction_data)
        }
        _ => Err(ControllerError::AccountFrame.into()),
    }
}

/// Authenticate compact signed intents, run Lean bytecode, and invoke children.
///
/// The instruction contains no claim or custody plan. Both are derived only
/// after exact Ed25519, Market/Realm/capability/policy, replay, Position, PDA,
/// and token checks.
#[inline(never)]
fn process_inline(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != 18 || instruction_data.len() != CONTROLLER_INSTRUCTION_BYTES {
        return Err(ControllerError::AccountFrame.into());
    }
    let instruction = ControllerInstructionV1::decode(instruction_data)
        .map_err(|_| ControllerError::Instruction)?;
    let seller_intent = instruction.seller;
    let buyer_intent = instruction.buyer;

    let mut iterator = accounts.iter();
    let controller = next(&mut iterator)?;
    let seller_replay = next(&mut iterator)?;
    let buyer_replay = next(&mut iterator)?;
    let journal = next(&mut iterator)?;
    let seller_position = next(&mut iterator)?;
    let buyer_position = next(&mut iterator)?;
    let claim_program = next(&mut iterator)?;
    let custody_program = next(&mut iterator)?;
    let market = next(&mut iterator)?;
    let realm = next(&mut iterator)?;
    let fee_policy = next(&mut iterator)?;
    let capability_manifest = next(&mut iterator)?;
    let mint = next(&mut iterator)?;
    let source = next(&mut iterator)?;
    let seller = next(&mut iterator)?;
    let venue = next(&mut iterator)?;
    let token_program = next(&mut iterator)?;
    let instructions = next(&mut iterator)?;

    validate_account_frame(
        program_id,
        controller,
        seller_replay,
        buyer_replay,
        journal,
        seller_position,
        buyer_position,
        claim_program,
        custody_program,
        market,
        realm,
        fee_policy,
        capability_manifest,
        mint,
        source,
        seller,
        venue,
        token_program,
        instructions,
    )?;
    let authority = authenticate_market_authority(
        market,
        realm,
        fee_policy,
        capability_manifest,
        mint,
        venue,
        token_program,
    )?;
    if seller_intent.market != market.key.to_bytes()
        || buyer_intent.market != market.key.to_bytes()
        || seller_intent.generation != authority.generation
        || buyer_intent.generation != authority.generation
        || seller_intent.collateral_account != seller.key.to_bytes()
        || buyer_intent.collateral_account != source.key.to_bytes()
        || seller_intent.fee_basis_points != authority.fee_basis_points
        || buyer_intent.fee_basis_points != authority.fee_basis_points
    {
        return Err(ControllerError::MarketAuthority.into());
    }

    let makers = authenticate_ed25519_batch(program_id, accounts, instruction_data, instructions)?;
    let controller_bump_seed = [instruction.controller_bump];
    let controller_seeds: [&[u8]; 2] = [CONTROLLER_SEED, &controller_bump_seed];
    let expected_controller = Pubkey::create_program_address(&controller_seeds, program_id)
        .map_err(|_| ControllerError::ControllerPda)?;
    if controller.key != &expected_controller {
        return Err(ControllerError::ControllerPda.into());
    }
    let generation = authority.generation.to_le_bytes();
    let seller_bump_seed = [instruction.seller_replay_bump];
    let seller_seeds: [&[u8]; 5] = [
        REPLAY_SEED,
        market.key.as_ref(),
        &generation,
        makers[0].as_ref(),
        &seller_bump_seed,
    ];
    let buyer_bump_seed = [instruction.buyer_replay_bump];
    let buyer_seeds: [&[u8]; 5] = [
        REPLAY_SEED,
        market.key.as_ref(),
        &generation,
        makers[1].as_ref(),
        &buyer_bump_seed,
    ];
    if seller_replay.key
        != &Pubkey::create_program_address(&seller_seeds, program_id)
            .map_err(|_| ControllerError::ReplayPda)?
        || buyer_replay.key
            != &Pubkey::create_program_address(&buyer_seeds, program_id)
                .map_err(|_| ControllerError::ReplayPda)?
    {
        return Err(ControllerError::ReplayPda.into());
    }

    let seller_outcome_seed = [seller_intent.outcome];
    let buyer_outcome_seed = [buyer_intent.outcome];
    let seller_position_bump_seed = [instruction.seller_position_bump];
    let buyer_position_bump_seed = [instruction.buyer_position_bump];
    let seller_position_seeds: [&[u8]; 5] = [
        POSITION_SEED,
        market.key.as_ref(),
        makers[0].as_ref(),
        &seller_outcome_seed,
        &seller_position_bump_seed,
    ];
    let buyer_position_seeds: [&[u8]; 5] = [
        POSITION_SEED,
        market.key.as_ref(),
        makers[1].as_ref(),
        &buyer_outcome_seed,
        &buyer_position_bump_seed,
    ];
    if seller_position.key
        != &Pubkey::create_program_address(&seller_position_seeds, program_id)
            .map_err(|_| ControllerError::ClaimState)?
        || buyer_position.key
            != &Pubkey::create_program_address(&buyer_position_seeds, program_id)
                .map_err(|_| ControllerError::ClaimState)?
    {
        return Err(ControllerError::ClaimState.into());
    }

    let seller_replay_state = decode_replay_state(seller_replay, controller)?;
    let buyer_replay_state = decode_replay_state(buyer_replay, controller)?;
    let seller_position_state = decode_position_state(seller_position, controller)?;
    let buyer_position_state = decode_position_state(buyer_position, controller)?;
    if seller_position_state.outcome != u64::from(seller_intent.outcome)
        || buyer_position_state.outcome != u64::from(buyer_intent.outcome)
    {
        return Err(ControllerError::ClaimState.into());
    }
    let claim_state = ClaimState {
        seller_nonce: seller_replay_state.nonce,
        buyer_nonce: buyer_replay_state.nonce,
        seller_claims: seller_position_state.claims,
        buyer_claims: buyer_position_state.claims,
    };
    let token_state = authenticate_token_state(
        authority,
        makers[1],
        buyer_replay,
        mint,
        source,
        seller,
        venue,
        token_program,
    )?;
    let mut registers = build_registers(
        authority,
        seller_intent,
        buyer_intent,
        makers,
        claim_state,
        token_state,
        instruction.fill,
        instruction.execution_price,
        Clock::get().map_err(|_| ControllerError::Instruction)?.slot,
    )?;
    dclutch_transition_vm::execute(&DIRECT_PROGRAM, &mut registers)
        .map_err(|_| ControllerError::Transition)?;
    let claim_plan = claim_plan(&registers, seller_intent.outcome)?;
    let custody_plan = custody_plan(&registers)?;

    increment_journal(journal)?;
    invoke_claim(
        controller,
        seller_replay,
        buyer_replay,
        seller_position,
        buyer_position,
        claim_program,
        &controller_seeds,
        claim_plan,
    )?;
    invoke_custody(
        controller,
        buyer_replay,
        mint,
        source,
        seller,
        venue,
        token_program,
        custody_program,
        &controller_seeds,
        &buyer_seeds,
        custody_plan,
    )
}

#[inline(never)]
fn process_registered_fill(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != 17 {
        return Err(ControllerError::AccountFrame.into());
    }
    let instruction = RegisteredFillInstructionV1::decode(instruction_data)
        .map_err(|_| ControllerError::Instruction)?;

    let mut iterator = accounts.iter();
    let controller = next(&mut iterator)?;
    let seller_registration = next(&mut iterator)?;
    let buyer_registration = next(&mut iterator)?;
    let journal = next(&mut iterator)?;
    let seller_position = next(&mut iterator)?;
    let buyer_position = next(&mut iterator)?;
    let claim_program = next(&mut iterator)?;
    let custody_program = next(&mut iterator)?;
    let market = next(&mut iterator)?;
    let realm = next(&mut iterator)?;
    let fee_policy = next(&mut iterator)?;
    let capability_manifest = next(&mut iterator)?;
    let mint = next(&mut iterator)?;
    let source = next(&mut iterator)?;
    let seller = next(&mut iterator)?;
    let venue = next(&mut iterator)?;
    let token_program = next(&mut iterator)?;

    validate_registered_account_frame(
        program_id,
        controller,
        seller_registration,
        buyer_registration,
        journal,
        seller_position,
        buyer_position,
        claim_program,
        custody_program,
        market,
        realm,
        fee_policy,
        capability_manifest,
        mint,
        source,
        seller,
        venue,
        token_program,
    )?;
    let authority = authenticate_market_authority(
        market,
        realm,
        fee_policy,
        capability_manifest,
        mint,
        venue,
        token_program,
    )?;
    let controller_bump_seed = [instruction.controller_bump];
    let controller_seeds: [&[u8]; 2] = [CONTROLLER_SEED, &controller_bump_seed];
    let expected_controller = Pubkey::create_program_address(&controller_seeds, program_id)
        .map_err(|_| ControllerError::ControllerPda)?;
    if controller.key != &expected_controller {
        return Err(ControllerError::ControllerPda.into());
    }

    let seller_state = decode_registered_state(seller_registration, controller)?;
    let buyer_state = decode_registered_state(buyer_registration, controller)?;
    let seller_intent = seller_state.intent;
    let buyer_intent = buyer_state.intent;
    if seller_state.phase != 0
        || buyer_state.phase != 0
        || seller_state.remaining == 0
        || buyer_state.remaining == 0
        || seller_state.remaining > seller_intent.maximum_fill
        || buyer_state.remaining > buyer_intent.maximum_fill
        || seller_intent.market != market.key.to_bytes()
        || buyer_intent.market != market.key.to_bytes()
        || seller_intent.generation != authority.generation
        || buyer_intent.generation != authority.generation
        || seller_intent.collateral_account != seller.key.to_bytes()
        || buyer_intent.collateral_account != source.key.to_bytes()
        || seller_intent.fee_basis_points != authority.fee_basis_points
        || buyer_intent.fee_basis_points != authority.fee_basis_points
    {
        return Err(ControllerError::MarketAuthority.into());
    }
    let makers = [
        Pubkey::new_from_array(seller_state.maker),
        Pubkey::new_from_array(buyer_state.maker),
    ];
    if makers[0] == Pubkey::default() || makers[1] == Pubkey::default() || makers[0] == makers[1] {
        return Err(ControllerError::ClaimState.into());
    }

    let generation = authority.generation.to_le_bytes();
    let seller_nonce = seller_intent.nonce.to_le_bytes();
    let buyer_nonce = buyer_intent.nonce.to_le_bytes();
    let seller_registration_bump_seed = [instruction.seller_registration_bump];
    let buyer_registration_bump_seed = [instruction.buyer_registration_bump];
    let seller_registration_seeds: [&[u8]; 6] = [
        REGISTERED_SEED,
        market.key.as_ref(),
        &generation,
        makers[0].as_ref(),
        &seller_nonce,
        &seller_registration_bump_seed,
    ];
    let buyer_registration_seeds: [&[u8]; 6] = [
        REGISTERED_SEED,
        market.key.as_ref(),
        &generation,
        makers[1].as_ref(),
        &buyer_nonce,
        &buyer_registration_bump_seed,
    ];
    if seller_registration.key
        != &Pubkey::create_program_address(&seller_registration_seeds, program_id)
            .map_err(|_| ControllerError::ReplayPda)?
        || buyer_registration.key
            != &Pubkey::create_program_address(&buyer_registration_seeds, program_id)
                .map_err(|_| ControllerError::ReplayPda)?
    {
        return Err(ControllerError::ReplayPda.into());
    }

    authenticate_position_addresses(
        program_id,
        market,
        makers,
        [seller_intent.outcome, buyer_intent.outcome],
        [
            instruction.seller_position_bump,
            instruction.buyer_position_bump,
        ],
        seller_position,
        buyer_position,
    )?;
    let seller_position_state = decode_position_state(seller_position, controller)?;
    let buyer_position_state = decode_position_state(buyer_position, controller)?;
    if seller_position_state.outcome != u64::from(seller_intent.outcome)
        || buyer_position_state.outcome != u64::from(buyer_intent.outcome)
    {
        return Err(ControllerError::ClaimState.into());
    }
    let claim_state = ClaimState {
        seller_nonce: seller_state.sequence,
        buyer_nonce: buyer_state.sequence,
        seller_claims: seller_position_state.claims,
        buyer_claims: buyer_position_state.claims,
    };
    let token_state = authenticate_token_state(
        authority,
        makers[1],
        buyer_registration,
        mint,
        source,
        seller,
        venue,
        token_program,
    )?;
    let mut seller_execution = seller_intent;
    seller_execution.nonce = seller_state.sequence;
    seller_execution.maximum_fill = seller_state.remaining;
    let mut buyer_execution = buyer_intent;
    buyer_execution.nonce = buyer_state.sequence;
    buyer_execution.maximum_fill = buyer_state.remaining;
    let mut registers = build_registers(
        authority,
        seller_execution,
        buyer_execution,
        makers,
        claim_state,
        token_state,
        instruction.fill,
        instruction.execution_price,
        Clock::get().map_err(|_| ControllerError::Instruction)?.slot,
    )?;
    dclutch_transition_vm::execute(&DIRECT_PROGRAM, &mut registers)
        .map_err(|_| ControllerError::Transition)?;
    let claim_instruction = registered_claim_fill_instruction(instruction.fill)
        .map_err(|_| ControllerError::Instruction)?;
    let custody_plan = custody_plan(&registers)?;

    increment_journal(journal)?;
    invoke_registered_claim(
        controller,
        seller_registration,
        buyer_registration,
        seller_position,
        buyer_position,
        claim_program,
        &controller_seeds,
        claim_instruction,
    )?;
    invoke_custody(
        controller,
        buyer_registration,
        mint,
        source,
        seller,
        venue,
        token_program,
        custody_program,
        &controller_seeds,
        &buyer_registration_seeds,
        custody_plan,
    )
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn validate_account_frame(
    program_id: &Pubkey,
    controller: &AccountInfo<'_>,
    seller_replay: &AccountInfo<'_>,
    buyer_replay: &AccountInfo<'_>,
    journal: &AccountInfo<'_>,
    seller_position: &AccountInfo<'_>,
    buyer_position: &AccountInfo<'_>,
    claim_program: &AccountInfo<'_>,
    custody_program: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    realm: &AccountInfo<'_>,
    fee_policy: &AccountInfo<'_>,
    capability_manifest: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    source: &AccountInfo<'_>,
    seller: &AccountInfo<'_>,
    venue: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
    instructions: &AccountInfo<'_>,
) -> Result<(), ControllerError> {
    if controller.is_signer
        || controller.is_writable
        || controller.executable
        || seller_replay.is_signer
        || !seller_replay.is_writable
        || seller_replay.executable
        || buyer_replay.is_signer
        || !buyer_replay.is_writable
        || buyer_replay.executable
        || journal.is_signer
        || !journal.is_writable
        || journal.executable
        || seller_position.is_signer
        || !seller_position.is_writable
        || seller_position.executable
        || buyer_position.is_signer
        || !buyer_position.is_writable
        || buyer_position.executable
        || !readonly_executable(claim_program)
        || !readonly_executable(custody_program)
        || !readonly_data(market)
        || !readonly_data(realm)
        || !readonly_data(fee_policy)
        || !readonly_data(capability_manifest)
        || mint.is_signer
        || mint.is_writable
        || mint.executable
        || source.is_signer
        || !source.is_writable
        || source.executable
        || seller.is_signer
        || !seller.is_writable
        || seller.executable
        || venue.is_signer
        || !venue.is_writable
        || venue.executable
        || !readonly_executable(token_program)
        || instructions.is_signer
        || instructions.is_writable
        || instructions.executable
    {
        return Err(ControllerError::AccountAuthority);
    }
    if journal.owner != program_id
        || seller_replay.owner != &CLAIM_PROGRAM_ID
        || buyer_replay.owner != &CLAIM_PROGRAM_ID
        || seller_position.owner != &CLAIM_PROGRAM_ID
        || buyer_position.owner != &CLAIM_PROGRAM_ID
        || claim_program.key != &CLAIM_PROGRAM_ID
        || custody_program.key != &CUSTODY_PROGRAM_ID
        || instructions.key != &sysvar::instructions::ID
        || instructions.owner != &sysvar::ID
    {
        return Err(ControllerError::AccountAuthority);
    }
    let keys = [
        controller.key,
        seller_replay.key,
        buyer_replay.key,
        journal.key,
        seller_position.key,
        buyer_position.key,
        claim_program.key,
        custody_program.key,
        market.key,
        realm.key,
        fee_policy.key,
        capability_manifest.key,
        mint.key,
        source.key,
        seller.key,
        venue.key,
        token_program.key,
        instructions.key,
    ];
    for (index, left) in keys.iter().enumerate() {
        if keys.iter().skip(index + 1).any(|right| left == right) {
            return Err(ControllerError::AccountAuthority);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn validate_registered_account_frame(
    program_id: &Pubkey,
    controller: &AccountInfo<'_>,
    seller_registration: &AccountInfo<'_>,
    buyer_registration: &AccountInfo<'_>,
    journal: &AccountInfo<'_>,
    seller_position: &AccountInfo<'_>,
    buyer_position: &AccountInfo<'_>,
    claim_program: &AccountInfo<'_>,
    custody_program: &AccountInfo<'_>,
    market: &AccountInfo<'_>,
    realm: &AccountInfo<'_>,
    fee_policy: &AccountInfo<'_>,
    capability_manifest: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    source: &AccountInfo<'_>,
    seller: &AccountInfo<'_>,
    venue: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<(), ControllerError> {
    if controller.is_signer
        || controller.is_writable
        || controller.executable
        || seller_registration.is_signer
        || !seller_registration.is_writable
        || seller_registration.executable
        || buyer_registration.is_signer
        || !buyer_registration.is_writable
        || buyer_registration.executable
        || journal.is_signer
        || !journal.is_writable
        || journal.executable
        || seller_position.is_signer
        || !seller_position.is_writable
        || seller_position.executable
        || buyer_position.is_signer
        || !buyer_position.is_writable
        || buyer_position.executable
        || !readonly_executable(claim_program)
        || !readonly_executable(custody_program)
        || !readonly_data(market)
        || !readonly_data(realm)
        || !readonly_data(fee_policy)
        || !readonly_data(capability_manifest)
        || mint.is_signer
        || mint.is_writable
        || mint.executable
        || source.is_signer
        || !source.is_writable
        || source.executable
        || seller.is_signer
        || !seller.is_writable
        || seller.executable
        || venue.is_signer
        || !venue.is_writable
        || venue.executable
        || !readonly_executable(token_program)
    {
        return Err(ControllerError::AccountAuthority);
    }
    if journal.owner != program_id
        || seller_registration.owner != &CLAIM_PROGRAM_ID
        || buyer_registration.owner != &CLAIM_PROGRAM_ID
        || seller_position.owner != &CLAIM_PROGRAM_ID
        || buyer_position.owner != &CLAIM_PROGRAM_ID
        || claim_program.key != &CLAIM_PROGRAM_ID
        || custody_program.key != &CUSTODY_PROGRAM_ID
    {
        return Err(ControllerError::AccountAuthority);
    }
    let keys = [
        controller.key,
        seller_registration.key,
        buyer_registration.key,
        journal.key,
        seller_position.key,
        buyer_position.key,
        claim_program.key,
        custody_program.key,
        market.key,
        realm.key,
        fee_policy.key,
        capability_manifest.key,
        mint.key,
        source.key,
        seller.key,
        venue.key,
        token_program.key,
    ];
    for (index, left) in keys.iter().enumerate() {
        if keys.iter().skip(index + 1).any(|right| left == right) {
            return Err(ControllerError::AccountAuthority);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn authenticate_position_addresses(
    program_id: &Pubkey,
    market: &AccountInfo<'_>,
    makers: [Pubkey; 2],
    outcomes: [u8; 2],
    bumps: [u8; 2],
    seller_position: &AccountInfo<'_>,
    buyer_position: &AccountInfo<'_>,
) -> Result<(), ControllerError> {
    let seller_outcome_seed = [outcomes[0]];
    let buyer_outcome_seed = [outcomes[1]];
    let seller_bump_seed = [bumps[0]];
    let buyer_bump_seed = [bumps[1]];
    let seller_seeds: [&[u8]; 5] = [
        POSITION_SEED,
        market.key.as_ref(),
        makers[0].as_ref(),
        &seller_outcome_seed,
        &seller_bump_seed,
    ];
    let buyer_seeds: [&[u8]; 5] = [
        POSITION_SEED,
        market.key.as_ref(),
        makers[1].as_ref(),
        &buyer_outcome_seed,
        &buyer_bump_seed,
    ];
    if seller_position.key
        != &Pubkey::create_program_address(&seller_seeds, program_id)
            .map_err(|_| ControllerError::ClaimState)?
        || buyer_position.key
            != &Pubkey::create_program_address(&buyer_seeds, program_id)
                .map_err(|_| ControllerError::ClaimState)?
    {
        return Err(ControllerError::ClaimState);
    }
    Ok(())
}

fn readonly_data(account: &AccountInfo<'_>) -> bool {
    !account.is_signer && !account.is_writable && !account.executable
}

fn authenticate_ed25519_batch(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    data: &[u8],
    instructions: &AccountInfo<'_>,
) -> Result<[Pubkey; 2], ControllerError> {
    let current =
        load_current_index_checked(instructions).map_err(|_| ControllerError::Signature)?;
    if current == 0 {
        return Err(ControllerError::Signature);
    }
    let current_instruction = load_instruction_at_checked(usize::from(current), instructions)
        .map_err(|_| ControllerError::Signature)?;
    if current_instruction.program_id != *program_id
        || current_instruction.data.as_slice() != data
        || current_instruction.accounts.len() != accounts.len()
    {
        return Err(ControllerError::Signature);
    }
    for (meta, actual) in current_instruction.accounts.iter().zip(accounts) {
        if meta.pubkey != *actual.key
            || meta.is_signer != actual.is_signer
            || meta.is_writable != actual.is_writable
        {
            return Err(ControllerError::Signature);
        }
    }
    let preceding = load_instruction_at_checked(usize::from(current - 1), instructions)
        .map_err(|_| ControllerError::Signature)?;
    if preceding.program_id != ed25519_program::ID
        || !preceding.accounts.is_empty()
        || preceding.data.len() != ED25519_INSTRUCTION_BYTES
        || read_u16(&preceding.data, 0)? != 2
    {
        return Err(ControllerError::Signature);
    }
    let mut makers = [Pubkey::default(); 2];
    for (index, message_offset) in [SELLER_INTENT_OFFSET, BUYER_INTENT_OFFSET]
        .into_iter()
        .enumerate()
    {
        let descriptor = 2 + index * ED25519_DESCRIPTOR_BYTES;
        let public_key_offset = ED25519_PAYLOAD_START + index * 96;
        let signature_offset = public_key_offset + 32;
        if usize::from(read_u16(&preceding.data, descriptor)?) != signature_offset
            || read_u16(&preceding.data, descriptor + 2)? != u16::MAX
            || usize::from(read_u16(&preceding.data, descriptor + 4)?) != public_key_offset
            || read_u16(&preceding.data, descriptor + 6)? != u16::MAX
            || usize::from(read_u16(&preceding.data, descriptor + 8)?) != message_offset
            || usize::from(read_u16(&preceding.data, descriptor + 10)?) != COMPACT_INTENT_BYTES
            || read_u16(&preceding.data, descriptor + 12)? != current
            || read_slice(&preceding.data, signature_offset, 64)?
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(ControllerError::Signature);
        }
        let maker = Pubkey::new_from_array(read_array(&preceding.data, public_key_offset)?);
        if maker == Pubkey::default() {
            return Err(ControllerError::Signature);
        }
        *makers.get_mut(index).ok_or(ControllerError::Signature)? = maker;
    }
    if makers[0] == makers[1] {
        return Err(ControllerError::Signature);
    }
    Ok(makers)
}

fn decode_replay_state(
    replay: &AccountInfo<'_>,
    controller: &AccountInfo<'_>,
) -> Result<ReplayState, ControllerError> {
    let data = replay
        .try_borrow_data()
        .map_err(|_| ControllerError::ClaimState)?;
    if data.len() != REPLAY_STATE_BYTES
        || data.get(..8) != Some([b'D', b'C', b'R', b'P', 1, 0, 0, 0].as_slice())
        || data.get(8..40) != Some(controller.key.as_ref())
    {
        return Err(ControllerError::ClaimState);
    }
    Ok(ReplayState {
        nonce: read_u64(&data, 40)?,
    })
}

fn decode_registered_state(
    registration: &AccountInfo<'_>,
    controller: &AccountInfo<'_>,
) -> Result<RegisteredIntentStateV1, ControllerError> {
    let data = registration
        .try_borrow_data()
        .map_err(|_| ControllerError::ClaimState)?;
    let state = RegisteredIntentStateV1::decode(&data).map_err(|_| ControllerError::ClaimState)?;
    if state.controller != controller.key.to_bytes() {
        return Err(ControllerError::ClaimState);
    }
    Ok(state)
}

fn decode_position_state(
    position: &AccountInfo<'_>,
    controller: &AccountInfo<'_>,
) -> Result<PositionState, ControllerError> {
    let data = position
        .try_borrow_data()
        .map_err(|_| ControllerError::ClaimState)?;
    if data.len() != POSITION_STATE_BYTES
        || data.get(..8) != Some([b'D', b'C', b'P', b'N', 1, 0, 0, 0].as_slice())
        || data.get(8..40) != Some(controller.key.as_ref())
    {
        return Err(ControllerError::ClaimState);
    }
    Ok(PositionState {
        outcome: read_u64(&data, 40)?,
        claims: read_u64(&data, 48)?,
    })
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_market_authority(
    market_account: &AccountInfo<'_>,
    realm_account: &AccountInfo<'_>,
    policy_account: &AccountInfo<'_>,
    manifest_account: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    venue: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<MarketAuthority, ControllerError> {
    let protocol_program = market_account.owner;
    if realm_account.owner != protocol_program
        || policy_account.owner != protocol_program
        || manifest_account.owner != protocol_program
    {
        return Err(ControllerError::MarketAuthority);
    }

    let market_data = market_account
        .try_borrow_data()
        .map_err(|_| ControllerError::MarketAuthority)?;
    let outcome_count =
        decode_market_outcome_count(&market_data).map_err(|_| ControllerError::MarketAuthority)?;
    let root_end = MARKET_ROOT_OFFSET
        .checked_add(MARKET_ROOT_BYTES)
        .ok_or(ControllerError::Arithmetic)?;
    let root = MarketRoot::decode(
        market_data
            .get(MARKET_ROOT_OFFSET..root_end)
            .ok_or(ControllerError::MarketAuthority)?,
    )
    .map_err(|_| ControllerError::MarketAuthority)?;
    if root.phase() != Phase::Open {
        return Err(ControllerError::MarketAuthority);
    }
    let identity_digest = hash(&root.identity().to_bytes()).to_bytes();
    let (expected_market, _) =
        Pubkey::find_program_address(&[MARKET_SEED, &identity_digest], protocol_program);
    if market_account.key != &expected_market {
        return Err(ControllerError::MarketAuthority);
    }

    let realm_data = realm_account
        .try_borrow_data()
        .map_err(|_| ControllerError::MarketAuthority)?;
    let realm = RealmV1::decode(&realm_data).map_err(|_| ControllerError::MarketAuthority)?;
    let realm_digest = hash(&realm_data).to_bytes();
    let (expected_realm, _) =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &realm_digest], protocol_program);
    if realm_account.key != &expected_realm
        || root.identity().realm_id().to_bytes() != realm_digest
        || realm.token_program() != token_program.key.as_ref()
        || realm.collateral_mint() != mint.key.as_ref()
        || token_program.key.to_bytes() != LEGACY_TOKEN_PROGRAM_ID
    {
        return Err(ControllerError::MarketAuthority);
    }
    let mut release = None;
    for candidate in PRODUCTION_ADAPTER_RELEASES {
        if hash(&candidate.to_bytes()).to_bytes() == *realm.collateral_adapter_release_id() {
            release = Some(candidate);
        }
    }
    let release = release.ok_or(ControllerError::MarketAuthority)?;
    if release.token_program() != LEGACY_TOKEN_PROGRAM_ID
        || release.profile() != ExactTransferProfileV1::LegacyExactTransferV1
    {
        return Err(ControllerError::MarketAuthority);
    }
    let mint_data = mint
        .try_borrow_data()
        .map_err(|_| ControllerError::MarketAuthority)?;
    let checked_mint = release
        .profile()
        .check_mint(LEGACY_TOKEN_PROGRAM_ID, &mint_data)
        .map_err(|_| ControllerError::MarketAuthority)?;
    if (realm.mint_authority_policy() == MintAuthorityPolicy::RequireAbsent
        && checked_mint.mint_authority != COption::None)
        || (realm.freeze_authority_policy() == FreezeAuthorityPolicy::RequireAbsent
            && checked_mint.freeze_authority != COption::None)
    {
        return Err(ControllerError::MarketAuthority);
    }

    let manifest_data = manifest_account
        .try_borrow_data()
        .map_err(|_| ControllerError::MarketAuthority)?;
    let manifest = CapabilityManifestV1::decode(&manifest_data)
        .map_err(|_| ControllerError::MarketAuthority)?;
    let manifest_digest = hash(manifest.as_bytes()).to_bytes();
    let expected_manifest = raw_record_address(
        protocol_program,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        manifest_digest,
    );
    if manifest_account.key != &expected_manifest
        || root.identity().capability_manifest_id().to_bytes() != manifest_digest
    {
        return Err(ControllerError::MarketAuthority);
    }

    let policy_data = policy_account
        .try_borrow_data()
        .map_err(|_| ControllerError::MarketAuthority)?;
    let policy =
        VenueFeePolicyV3::decode(&policy_data).map_err(|_| ControllerError::MarketAuthority)?;
    let policy_digest = hash(&policy_data).to_bytes();
    let expected_policy = raw_record_address(
        protocol_program,
        VENUE_FEE_POLICY_SCHEMA_RELEASE_ID_V3,
        policy_digest,
    );
    if policy_account.key != &expected_policy || policy.recipient() != venue.key.as_ref() {
        return Err(ControllerError::MarketAuthority);
    }

    let mut selected = None;
    let mut index = 0_u16;
    while index < manifest.entry_count() {
        let entry = manifest
            .entry(index)
            .map_err(|_| ControllerError::MarketAuthority)?;
        if entry.kind_id().to_bytes() == DIRECT_CAPABILITY_KIND_ID_V2 {
            if selected.is_some() {
                return Err(ControllerError::MarketAuthority);
            }
            selected = Some(entry);
        }
        index = index.checked_add(1).ok_or(ControllerError::Arithmetic)?;
    }
    let entry = selected.ok_or(ControllerError::MarketAuthority)?;
    let funding = entry.funding_quote();
    if entry.release_id().to_bytes() != COMPILED_DIRECT_RELEASE_ID_V1
        || entry.config_id().to_bytes() != policy_digest
        || entry.capacity_profile_id().to_bytes() != COMPILED_DIRECT_CAPACITY_ID_V1
        || entry.child_schema_id().to_bytes() != COMPILED_DIRECT_CHILD_SCHEMA_ID_V1
        || entry.child_derivation_id().to_bytes() != COMPILED_DIRECT_DERIVATION_ID_V1
        || entry.activation_policy() != ActivationPolicy::RequiredAtFounding
        || entry.activation_deadline_slot() != 0
        || entry.dependency_count() != 0
        || funding.native_lamports_total() != 0
        || funding.realm_collateral_total() != 0
        || funding.realm_collateral().is_some()
    {
        return Err(ControllerError::MarketAuthority);
    }

    Ok(MarketAuthority {
        phase: 1,
        outcome_count,
        generation: root.identity().generation(),
        fee_basis_points: policy.fee_basis_points(),
        collateral_mint: *realm.collateral_mint(),
        fee_recipient: *policy.recipient(),
    })
}

fn raw_record_address(
    protocol_program: &Pubkey,
    schema_release_id: [u8; 32],
    digest: [u8; 32],
) -> Pubkey {
    Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema_release_id, &digest],
        protocol_program,
    )
    .0
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn authenticate_token_state(
    authority: MarketAuthority,
    buyer: Pubkey,
    buyer_replay: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    source: &AccountInfo<'_>,
    seller: &AccountInfo<'_>,
    venue: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<[u64; 3], ControllerError> {
    if token_program.key.to_bytes() != LEGACY_TOKEN_PROGRAM_ID
        || mint.owner != token_program.key
        || source.owner != token_program.key
        || seller.owner != token_program.key
        || venue.owner != token_program.key
    {
        return Err(ControllerError::TokenState);
    }
    let exact = ExactTransferProfileV1::LegacyExactTransferV1;
    let _checked_mint = exact
        .check_mint(
            LEGACY_TOKEN_PROGRAM_ID,
            &mint
                .try_borrow_data()
                .map_err(|_| ControllerError::TokenState)?,
        )
        .map_err(|_| ControllerError::TokenState)?;
    let source = exact
        .check_transfer_account(
            LEGACY_TOKEN_PROGRAM_ID,
            &source
                .try_borrow_data()
                .map_err(|_| ControllerError::TokenState)?,
        )
        .map_err(|_| ControllerError::TokenState)?;
    let (seller_mint, seller_amount) = legacy_destination_projection(seller)?;
    let (venue_mint, venue_amount) = legacy_destination_projection(venue)?;
    if source.mint != authority.collateral_mint
        || seller_mint != authority.collateral_mint
        || venue_mint != authority.collateral_mint
        || venue.key.to_bytes() != authority.fee_recipient
        || source.owner != buyer.to_bytes()
        || source.delegate != COption::Some(buyer_replay.key.to_bytes())
    {
        return Err(ControllerError::TokenState);
    }
    Ok([source.amount, seller_amount, venue_amount])
}

fn legacy_destination_projection(
    account: &AccountInfo<'_>,
) -> Result<([u8; 32], u64), ControllerError> {
    let data = account
        .try_borrow_data()
        .map_err(|_| ControllerError::TokenState)?;
    if data.len() != ACCOUNT_BYTES {
        return Err(ControllerError::TokenState);
    }
    Ok((read_array(&data, 0)?, read_u64(&data, 64)?))
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn build_registers(
    authority: MarketAuthority,
    seller: CompactIntentV1,
    buyer: CompactIntentV1,
    makers: [Pubkey; 2],
    claims: ClaimState,
    token: [u64; 3],
    fill: u64,
    execution_price: u64,
    slot: u64,
) -> Result<Registers, ControllerError> {
    let mut registers = Registers::zeroed();
    // Inputs occupy the schema prefix before the first program-owned output.
    // This type boundary fails compilation if Lean changes that partition.
    let scalars: [u64; SCALAR_GROSS_OUTPUT] = direct_input_scalars! {
        phase: authority.phase as u64,
        slot: slot,
        seller_from: seller.valid_from,
        seller_through: seller.valid_through,
        buyer_from: buyer.valid_from,
        buyer_through: buyer.valid_through,
        seller_side: seller.side as u64,
        buyer_side: buyer.side as u64,
        seller_generation: seller.generation,
        buyer_generation: buyer.generation,
        seller_outcome: seller.outcome as u64,
        buyer_outcome: buyer.outcome as u64,
        outcome_count: authority.outcome_count as u64,
        seller_lifecycle: seller.lifecycle as u64,
        seller_maximum: seller.maximum_fill,
        buyer_lifecycle: buyer.lifecycle as u64,
        buyer_maximum: buyer.maximum_fill,
        seller_nonce: seller.nonce,
        buyer_nonce: buyer.nonce,
        seller_next_nonce: claims.seller_nonce,
        buyer_next_nonce: claims.buyer_nonce,
        seller_limit: seller.limit_price,
        execution_price: execution_price,
        buyer_limit: buyer.limit_price,
        price_scale: PRICE_SCALE,
        seller_fee_bps: seller.fee_basis_points as u64,
        buyer_fee_bps: buyer.fee_basis_points as u64,
        policy_fee_bps: authority.fee_basis_points as u64,
        fill: fill,
        seller_claims: claims.seller_claims,
        buyer_claims: claims.buyer_claims,
        buyer_collateral: token[0],
        seller_collateral: token[1],
        venue_collateral: token[2],
    };
    for (index, value) in scalars.into_iter().enumerate() {
        registers
            .set_scalar(index, value)
            .map_err(|_| ControllerError::Transition)?;
    }
    let identities: [[u8; 32]; IDENTITY_COUNT] = direct_input_identities! {
        seller_market: seller.market,
        buyer_market: buyer.market,
        seller_maker: makers[0].to_bytes(),
        buyer_maker: makers[1].to_bytes(),
    };
    for (index, identity) in identities.into_iter().enumerate() {
        registers
            .set_identity(index, identity)
            .map_err(|_| ControllerError::Transition)?;
    }
    Ok(registers)
}

#[inline(never)]
fn claim_plan(
    registers: &Registers,
    outcome: u8,
) -> Result<[u8; CLAIM_PLAN_BYTES], ControllerError> {
    let mut plan = CLAIM_PLAN_TEMPLATE;
    patch_bytes(
        &mut plan,
        CLAIM_SELLER_NONCE_OFFSET,
        register_scalar(registers, SCALAR_SELLER_NONCE_OUTPUT)?.to_le_bytes(),
    )?;
    patch_bytes(
        &mut plan,
        CLAIM_BUYER_NONCE_OFFSET,
        register_scalar(registers, SCALAR_BUYER_NONCE_OUTPUT)?.to_le_bytes(),
    )?;
    let outcome = u32::from(outcome).to_le_bytes();
    patch_bytes(&mut plan, CLAIM_SELLER_OUTCOME_OFFSET, outcome)?;
    patch_bytes(&mut plan, CLAIM_BUYER_OUTCOME_OFFSET, outcome)?;
    let fill = register_scalar(registers, SCALAR_FILL)?.to_le_bytes();
    patch_bytes(&mut plan, CLAIM_SELLER_FILL_OFFSET, fill)?;
    patch_bytes(&mut plan, CLAIM_BUYER_FILL_OFFSET, fill)?;
    Ok(plan)
}

#[inline(never)]
fn custody_plan(registers: &Registers) -> Result<[u8; CUSTODY_PLAN_BYTES], ControllerError> {
    let mut plan = CUSTODY_PLAN_TEMPLATE;
    patch_bytes(
        &mut plan,
        CUSTODY_GROSS_OFFSET,
        register_scalar(registers, SCALAR_GROSS_OUTPUT)?.to_le_bytes(),
    )?;
    patch_bytes(
        &mut plan,
        CUSTODY_FEE_OFFSET,
        register_scalar(registers, SCALAR_FEE_OUTPUT)?.to_le_bytes(),
    )?;
    Ok(plan)
}

fn register_scalar(registers: &Registers, index: usize) -> Result<u64, ControllerError> {
    registers
        .scalar(index)
        .map_err(|_| ControllerError::Transition)
}

fn patch_bytes<const WIDTH: usize>(
    bytes: &mut [u8],
    offset: usize,
    value: [u8; WIDTH],
) -> Result<(), ControllerError> {
    let end = offset
        .checked_add(WIDTH)
        .ok_or(ControllerError::Arithmetic)?;
    bytes
        .get_mut(offset..end)
        .ok_or(ControllerError::Arithmetic)?
        .copy_from_slice(&value);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn invoke_claim<'info>(
    controller: &AccountInfo<'info>,
    seller_replay: &AccountInfo<'info>,
    buyer_replay: &AccountInfo<'info>,
    seller_position: &AccountInfo<'info>,
    buyer_position: &AccountInfo<'info>,
    claim_program: &AccountInfo<'info>,
    controller_seeds: &[&[u8]],
    plan: [u8; CLAIM_PLAN_BYTES],
) -> ProgramResult {
    invoke_signed(
        &Instruction {
            program_id: CLAIM_PROGRAM_ID,
            accounts: std::vec![
                AccountMeta::new_readonly(*controller.key, true),
                AccountMeta::new(*seller_replay.key, false),
                AccountMeta::new(*buyer_replay.key, false),
                AccountMeta::new(*seller_position.key, false),
                AccountMeta::new(*buyer_position.key, false),
            ],
            data: plan.to_vec(),
        },
        &[
            controller.clone(),
            seller_replay.clone(),
            buyer_replay.clone(),
            seller_position.clone(),
            buyer_position.clone(),
            claim_program.clone(),
        ],
        &[controller_seeds],
    )
}

#[allow(clippy::too_many_arguments)]
fn invoke_registered_claim<'info>(
    controller: &AccountInfo<'info>,
    seller_registration: &AccountInfo<'info>,
    buyer_registration: &AccountInfo<'info>,
    seller_position: &AccountInfo<'info>,
    buyer_position: &AccountInfo<'info>,
    claim_program: &AccountInfo<'info>,
    controller_seeds: &[&[u8]],
    instruction: [u8; REGISTERED_CLAIM_FILL_BYTES],
) -> ProgramResult {
    invoke_signed(
        &Instruction {
            program_id: CLAIM_PROGRAM_ID,
            accounts: std::vec![
                AccountMeta::new_readonly(*controller.key, true),
                AccountMeta::new(*seller_registration.key, false),
                AccountMeta::new(*buyer_registration.key, false),
                AccountMeta::new(*seller_position.key, false),
                AccountMeta::new(*buyer_position.key, false),
            ],
            data: instruction.to_vec(),
        },
        &[
            controller.clone(),
            seller_registration.clone(),
            buyer_registration.clone(),
            seller_position.clone(),
            buyer_position.clone(),
            claim_program.clone(),
        ],
        &[controller_seeds],
    )
}

#[allow(clippy::too_many_arguments)]
fn invoke_custody<'info>(
    controller: &AccountInfo<'info>,
    replay: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    source: &AccountInfo<'info>,
    seller: &AccountInfo<'info>,
    venue: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    custody_program: &AccountInfo<'info>,
    controller_seeds: &[&[u8]],
    replay_seeds: &[&[u8]],
    plan: [u8; CUSTODY_PLAN_BYTES],
) -> ProgramResult {
    invoke_signed(
        &Instruction {
            program_id: CUSTODY_PROGRAM_ID,
            accounts: std::vec![
                AccountMeta::new_readonly(*controller.key, true),
                AccountMeta::new_readonly(*replay.key, true),
                AccountMeta::new_readonly(*mint.key, false),
                AccountMeta::new(*source.key, false),
                AccountMeta::new(*seller.key, false),
                AccountMeta::new(*venue.key, false),
                AccountMeta::new_readonly(*token_program.key, false),
            ],
            data: plan.to_vec(),
        },
        &[
            controller.clone(),
            replay.clone(),
            mint.clone(),
            source.clone(),
            seller.clone(),
            venue.clone(),
            token_program.clone(),
            custody_program.clone(),
        ],
        &[controller_seeds, replay_seeds],
    )
}

fn increment_journal(journal: &AccountInfo<'_>) -> ProgramResult {
    let mut data = journal
        .try_borrow_mut_data()
        .map_err(|_| ControllerError::Journal)?;
    if data.len() != JOURNAL_BYTES
        || data.get(..4) != Some(JOURNAL_MAGIC.as_slice())
        || data.get(4..8) != Some([0_u8; 4].as_slice())
    {
        return Err(ControllerError::Journal.into());
    }
    let next = read_u64(&data, 8)?
        .checked_add(1)
        .ok_or(ControllerError::JournalOverflow)?;
    data.get_mut(8..16)
        .ok_or(ControllerError::Journal)?
        .copy_from_slice(&next.to_le_bytes());
    Ok(())
}

fn readonly_executable(account: &AccountInfo<'_>) -> bool {
    !account.is_signer && !account.is_writable && account.executable
}

fn next<'a, 'info>(
    iterator: &mut core::slice::Iter<'a, AccountInfo<'info>>,
) -> Result<&'a AccountInfo<'info>, ControllerError> {
    next_account_info(iterator).map_err(|_| ControllerError::AccountFrame)
}

fn read_slice(input: &[u8], offset: usize, width: usize) -> Result<&[u8], ControllerError> {
    let end = offset
        .checked_add(width)
        .ok_or(ControllerError::Instruction)?;
    input.get(offset..end).ok_or(ControllerError::Instruction)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N], ControllerError> {
    read_slice(input, offset, N)?
        .try_into()
        .map_err(|_| ControllerError::Instruction)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16, ControllerError> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, ControllerError> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}
