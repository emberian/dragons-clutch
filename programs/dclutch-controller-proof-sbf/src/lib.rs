#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Controller membrane compiling signed Direct intents into child plans.

extern crate std;

mod generated_direct_program;

use dclutch_token_svm::{ACCOUNT_BYTES, COption, ExactTransferProfileV1, LEGACY_TOKEN_PROGRAM_ID};
use dclutch_transition_vm::Registers;
use generated_direct_program::DIRECT_PROGRAM;
use solana_instructions_sysvar::{load_current_index_checked, load_instruction_at_checked};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    clock::Clock,
    entrypoint::ProgramResult,
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
/// Canonical maker/outcome Position domain for the successor experiment.
pub const POSITION_SEED: &[u8] = b"dclutch/position/v1";
/// Exact-account claim proof-program identity used by this experiment.
pub const CLAIM_PROGRAM_ID: Pubkey = Pubkey::new_from_array([81_u8; 32]);
/// Real custody proof-program identity used by this experiment.
pub const CUSTODY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([75_u8; 32]);
/// Bytes in the canonical controller journal.
pub const JOURNAL_BYTES: usize = 16;
/// Bytes in one compact independently signed Direct intent.
pub const SIGNED_INTENT_BYTES: usize = 136;
/// Bytes in the exact successor controller instruction.
pub const CONTROLLER_INSTRUCTION_BYTES: usize = 304;
/// Bytes in one read-only experimental Market execution profile.
pub const MARKET_PROFILE_BYTES: usize = 136;

const JOURNAL_MAGIC: &[u8; 4] = b"DCCJ";
const CONTROLLER_MAGIC: &[u8; 8] = b"DCLTCTL1";
const INTENT_MAGIC: &[u8; 8] = b"DCLTDIR3";
const PROFILE_MAGIC: &[u8; 8] = b"DCLTPRF1";
const SCHEMA_VERSION: u16 = 1;
const CLAIM_PLAN_BYTES: usize = 72;
const CUSTODY_PLAN_BYTES: usize = 40;
const REPLAY_STATE_BYTES: usize = 48;
const POSITION_STATE_BYTES: usize = 56;
const ED25519_DESCRIPTOR_BYTES: usize = 14;
const ED25519_PAYLOAD_START: usize = 2 + 2 * ED25519_DESCRIPTOR_BYTES;
const ED25519_INSTRUCTION_BYTES: usize = ED25519_PAYLOAD_START + 2 * 96;
const SELLER_INTENT_OFFSET: usize = 32;
const BUYER_INTENT_OFFSET: usize = SELLER_INTENT_OFFSET + SIGNED_INTENT_BYTES;

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
    /// Read-only Market execution profile was not exact.
    MarketProfile = 10,
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
struct SignedIntent {
    side: u8,
    outcome: u8,
    lifecycle: u8,
    market: [u8; 32],
    generation: u64,
    nonce: u64,
    valid_from: u64,
    valid_through: u64,
    maximum: u64,
    limit: u64,
    fee_basis_points: u16,
    collateral_account: [u8; 32],
}

#[derive(Clone, Copy)]
struct MarketProfile {
    phase: u8,
    outcome_count: u8,
    generation: u64,
    price_scale: u64,
    fee_basis_points: u16,
    token_program: [u8; 32],
    collateral_mint: [u8; 32],
    fee_recipient: [u8; 32],
}

#[derive(Clone, Copy)]
struct ControllerHeader {
    controller_bump: u8,
    seller_replay_bump: u8,
    buyer_replay_bump: u8,
    seller_position_bump: u8,
    buyer_position_bump: u8,
    fill: u64,
    execution_price: u64,
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

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Authenticate compact signed intents, run Lean bytecode, and invoke children.
///
/// The instruction contains no claim or custody plan. Both are derived only
/// after exact Ed25519, Market-profile, replay, Position, PDA, and token checks.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != 15 || instruction_data.len() != CONTROLLER_INSTRUCTION_BYTES {
        return Err(ControllerError::AccountFrame.into());
    }
    let header = decode_controller_header(instruction_data)?;
    let seller_intent = decode_intent(read_slice(
        instruction_data,
        SELLER_INTENT_OFFSET,
        SIGNED_INTENT_BYTES,
    )?)?;
    let buyer_intent = decode_intent(read_slice(
        instruction_data,
        BUYER_INTENT_OFFSET,
        SIGNED_INTENT_BYTES,
    )?)?;

    let mut iterator = accounts.iter();
    let controller = next(&mut iterator)?;
    let seller_replay = next(&mut iterator)?;
    let buyer_replay = next(&mut iterator)?;
    let journal = next(&mut iterator)?;
    let seller_position = next(&mut iterator)?;
    let buyer_position = next(&mut iterator)?;
    let claim_program = next(&mut iterator)?;
    let custody_program = next(&mut iterator)?;
    let market_profile = next(&mut iterator)?;
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
        market_profile,
        mint,
        source,
        seller,
        venue,
        token_program,
        instructions,
    )?;
    let profile = decode_market_profile(
        &market_profile
            .try_borrow_data()
            .map_err(|_| ControllerError::MarketProfile)?,
    )?;
    if seller_intent.market != market_profile.key.to_bytes()
        || buyer_intent.market != market_profile.key.to_bytes()
        || seller_intent.generation != profile.generation
        || buyer_intent.generation != profile.generation
        || seller_intent.collateral_account != seller.key.to_bytes()
        || buyer_intent.collateral_account != source.key.to_bytes()
        || profile.token_program != token_program.key.to_bytes()
        || profile.collateral_mint != mint.key.to_bytes()
        || profile.fee_recipient != venue.key.to_bytes()
    {
        return Err(ControllerError::MarketProfile.into());
    }

    let makers = authenticate_ed25519_batch(program_id, accounts, instruction_data, instructions)?;
    let controller_bump_seed = [header.controller_bump];
    let controller_seeds: [&[u8]; 2] = [CONTROLLER_SEED, &controller_bump_seed];
    let expected_controller = Pubkey::create_program_address(&controller_seeds, program_id)
        .map_err(|_| ControllerError::ControllerPda)?;
    if controller.key != &expected_controller {
        return Err(ControllerError::ControllerPda.into());
    }
    let generation = profile.generation.to_le_bytes();
    let seller_bump_seed = [header.seller_replay_bump];
    let seller_seeds: [&[u8]; 5] = [
        REPLAY_SEED,
        market_profile.key.as_ref(),
        &generation,
        makers[0].as_ref(),
        &seller_bump_seed,
    ];
    let buyer_bump_seed = [header.buyer_replay_bump];
    let buyer_seeds: [&[u8]; 5] = [
        REPLAY_SEED,
        market_profile.key.as_ref(),
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
    let seller_position_bump_seed = [header.seller_position_bump];
    let buyer_position_bump_seed = [header.buyer_position_bump];
    let seller_position_seeds: [&[u8]; 5] = [
        POSITION_SEED,
        market_profile.key.as_ref(),
        makers[0].as_ref(),
        &seller_outcome_seed,
        &seller_position_bump_seed,
    ];
    let buyer_position_seeds: [&[u8]; 5] = [
        POSITION_SEED,
        market_profile.key.as_ref(),
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
        profile,
        makers[1],
        buyer_replay,
        mint,
        source,
        seller,
        venue,
        token_program,
    )?;
    let mut registers = build_registers(
        profile,
        seller_intent,
        buyer_intent,
        makers,
        claim_state,
        token_state,
        header.fill,
        header.execution_price,
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

fn decode_controller_header(input: &[u8]) -> Result<ControllerHeader, ControllerError> {
    if input.get(..8) != Some(CONTROLLER_MAGIC.as_slice())
        || read_u16(input, 8)? != SCHEMA_VERSION
        || read_byte(input, 15)? != 0
    {
        return Err(ControllerError::Instruction);
    }
    Ok(ControllerHeader {
        controller_bump: read_byte(input, 10)?,
        seller_replay_bump: read_byte(input, 11)?,
        buyer_replay_bump: read_byte(input, 12)?,
        seller_position_bump: read_byte(input, 13)?,
        buyer_position_bump: read_byte(input, 14)?,
        fill: read_u64(input, 16)?,
        execution_price: read_u64(input, 24)?,
    })
}

fn decode_intent(input: &[u8]) -> Result<SignedIntent, ControllerError> {
    if input.len() != SIGNED_INTENT_BYTES
        || input.get(..8) != Some(INTENT_MAGIC.as_slice())
        || read_u16(input, 8)? != SCHEMA_VERSION
        || read_slice(input, 13, 3)?.iter().any(|byte| *byte != 0)
        || read_slice(input, 98, 6)?.iter().any(|byte| *byte != 0)
    {
        return Err(ControllerError::Intent);
    }
    Ok(SignedIntent {
        side: read_byte(input, 10)?,
        outcome: read_byte(input, 11)?,
        lifecycle: read_byte(input, 12)?,
        market: read_array(input, 16)?,
        generation: read_u64(input, 48)?,
        nonce: read_u64(input, 56)?,
        valid_from: read_u64(input, 64)?,
        valid_through: read_u64(input, 72)?,
        maximum: read_u64(input, 80)?,
        limit: read_u64(input, 88)?,
        fee_basis_points: read_u16(input, 96)?,
        collateral_account: read_array(input, 104)?,
    })
}

fn decode_market_profile(input: &[u8]) -> Result<MarketProfile, ControllerError> {
    if input.len() != MARKET_PROFILE_BYTES
        || input.get(..8) != Some(PROFILE_MAGIC.as_slice())
        || read_u16(input, 8)? != SCHEMA_VERSION
        || read_slice(input, 12, 4)?.iter().any(|byte| *byte != 0)
        || read_slice(input, 34, 6)?.iter().any(|byte| *byte != 0)
    {
        return Err(ControllerError::MarketProfile);
    }
    Ok(MarketProfile {
        phase: read_byte(input, 10)?,
        outcome_count: read_byte(input, 11)?,
        generation: read_u64(input, 16)?,
        price_scale: read_u64(input, 24)?,
        fee_basis_points: read_u16(input, 32)?,
        token_program: read_array(input, 40)?,
        collateral_mint: read_array(input, 72)?,
        fee_recipient: read_array(input, 104)?,
    })
}

#[allow(clippy::too_many_arguments)]
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
    market_profile: &AccountInfo<'_>,
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
        || market_profile.is_signer
        || market_profile.is_writable
        || market_profile.executable
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
        || market_profile.owner != program_id
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
        market_profile.key,
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
            || usize::from(read_u16(&preceding.data, descriptor + 10)?) != SIGNED_INTENT_BYTES
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
fn authenticate_token_state(
    profile: MarketProfile,
    buyer: Pubkey,
    buyer_replay: &AccountInfo<'_>,
    mint: &AccountInfo<'_>,
    source: &AccountInfo<'_>,
    seller: &AccountInfo<'_>,
    venue: &AccountInfo<'_>,
    token_program: &AccountInfo<'_>,
) -> Result<[u64; 3], ControllerError> {
    if profile.token_program != LEGACY_TOKEN_PROGRAM_ID
        || token_program.key.to_bytes() != LEGACY_TOKEN_PROGRAM_ID
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
    if source.mint != profile.collateral_mint
        || seller_mint != profile.collateral_mint
        || venue_mint != profile.collateral_mint
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
fn build_registers(
    profile: MarketProfile,
    seller: SignedIntent,
    buyer: SignedIntent,
    makers: [Pubkey; 2],
    claims: ClaimState,
    token: [u64; 3],
    fill: u64,
    execution_price: u64,
    slot: u64,
) -> Result<Registers, ControllerError> {
    let mut registers = Registers::zeroed();
    let scalars = [
        profile.phase as u64,
        slot,
        seller.valid_from,
        seller.valid_through,
        buyer.valid_from,
        buyer.valid_through,
        seller.side as u64,
        buyer.side as u64,
        seller.generation,
        buyer.generation,
        seller.outcome as u64,
        buyer.outcome as u64,
        profile.outcome_count as u64,
        seller.lifecycle as u64,
        seller.maximum,
        buyer.lifecycle as u64,
        buyer.maximum,
        seller.nonce,
        buyer.nonce,
        claims.seller_nonce,
        claims.buyer_nonce,
        seller.limit,
        execution_price,
        buyer.limit,
        profile.price_scale,
        seller.fee_basis_points as u64,
        buyer.fee_basis_points as u64,
        profile.fee_basis_points as u64,
        fill,
        claims.seller_claims,
        claims.buyer_claims,
        token[0],
        token[1],
        token[2],
    ];
    for (index, value) in scalars.into_iter().enumerate() {
        registers
            .set_scalar(index, value)
            .map_err(|_| ControllerError::Transition)?;
    }
    for (index, identity) in [
        seller.market,
        buyer.market,
        makers[0].to_bytes(),
        makers[1].to_bytes(),
    ]
    .into_iter()
    .enumerate()
    {
        registers
            .set_identity(index, identity)
            .map_err(|_| ControllerError::Transition)?;
    }
    Ok(registers)
}

fn claim_plan(
    registers: &Registers,
    outcome: u8,
) -> Result<[u8; CLAIM_PLAN_BYTES], ControllerError> {
    let mut plan = [0_u8; CLAIM_PLAN_BYTES];
    plan[..8].copy_from_slice(&[b'D', b'C', b'E', b'F', 1, 4, 0, 0]);
    write_effect(
        &mut plan,
        8,
        [0, 0, 0, 0],
        0,
        register_scalar(registers, 39)?,
    )?;
    write_effect(
        &mut plan,
        24,
        [0, 1, 0, 0],
        0,
        register_scalar(registers, 40)?,
    )?;
    write_effect(
        &mut plan,
        40,
        [1, 0, 1, 0],
        u32::from(outcome),
        register_scalar(registers, 28)?,
    )?;
    write_effect(
        &mut plan,
        56,
        [2, 1, 1, 0],
        u32::from(outcome),
        register_scalar(registers, 28)?,
    )?;
    Ok(plan)
}

fn custody_plan(registers: &Registers) -> Result<[u8; CUSTODY_PLAN_BYTES], ControllerError> {
    let mut plan = [0_u8; CUSTODY_PLAN_BYTES];
    plan[..8].copy_from_slice(&[b'D', b'C', b'C', b'P', 1, 2, 0, 0]);
    plan[8..16].copy_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
    plan[16..24].copy_from_slice(&register_scalar(registers, 34)?.to_le_bytes());
    plan[24..32].copy_from_slice(&[1, 2, 0, 0, 0, 0, 0, 0]);
    plan[32..40].copy_from_slice(&register_scalar(registers, 35)?.to_le_bytes());
    Ok(plan)
}

fn register_scalar(registers: &Registers, index: usize) -> Result<u64, ControllerError> {
    registers
        .scalar(index)
        .map_err(|_| ControllerError::Transition)
}

fn write_effect(
    plan: &mut [u8],
    offset: usize,
    tag: [u8; 4],
    outcome: u32,
    value: u64,
) -> Result<(), ControllerError> {
    let end = offset.checked_add(16).ok_or(ControllerError::Arithmetic)?;
    let record = plan
        .get_mut(offset..end)
        .ok_or(ControllerError::Arithmetic)?;
    record
        .get_mut(..4)
        .ok_or(ControllerError::Arithmetic)?
        .copy_from_slice(&tag);
    record
        .get_mut(4..8)
        .ok_or(ControllerError::Arithmetic)?
        .copy_from_slice(&outcome.to_le_bytes());
    record
        .get_mut(8..16)
        .ok_or(ControllerError::Arithmetic)?
        .copy_from_slice(&value.to_le_bytes());
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

fn read_byte(input: &[u8], offset: usize) -> Result<u8, ControllerError> {
    input
        .get(offset)
        .copied()
        .ok_or(ControllerError::Instruction)
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
