#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF signer for the claim-check escrow's burn approval.
//!
//! FRACCHECK's permissioned-burn campaign proved Token-2022's rule with two
//! ordinary keypairs and named its own gap: *"the escrow in the hand-off test
//! is an ordinary keypair, not a PDA. What is under test is who Token-2022 will
//! accept as an approver, not the derivation of the approver."* This program
//! closes that gap. It derives the escrow from
//! [`dclutch_claims::claim_check_v1::ClaimCheckEscrowSeedsV1`] -- the exact
//! recipe the shipped Claims escrow uses, bump included -- and signs with it,
//! so what the Token program accepts is a signature this tree actually knows
//! how to produce rather than one a test could always manufacture.
//!
//! It owns no protocol state and publishes no production ABI. The Claims
//! program will produce the same signature from the same seeds under its own
//! program id; the one thing this campaign cannot borrow is that id, because
//! `invoke_signed` only signs for the calling program's own addresses. That
//! residual is named in the evidence rather than papered over.
//!
//! The Fractional capability root is a **Trading**-derived PDA and this program
//! is not Trading either, so the authority the hand-off moves *from* is a
//! stand-in derived here under a seed that says so. What the stand-in buys over
//! the keypair FRACCHECK used is that both sides of the hand-off are then
//! program-derived, which is the shape production has: a program that cannot
//! sign for itself hands to a program that can.

extern crate alloc;

use alloc::vec::Vec;
use dclutch_claims::claim_check_v1::ClaimCheckEscrowSeedsV1;
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    pubkey::Pubkey,
};
use spl_token_2022_interface::{
    extension::permissioned_burn::instruction as permissioned_burn,
    instruction as token_instruction,
};

/// Seed domain of the Fractional-root stand-in this program signs with.
///
/// Deliberately not the capability-root domain. A test program that derived the
/// real `dclutch:capability-root:v1` address under its own id would produce an
/// address that looks like a root and is not one, and the next reader would
/// have to work out which.
pub const ESCROW_SIGNER_ROOT_STAND_IN_SEED: &[u8] = b"fraccheck2:root-stand-in:v1";

/// Seed domain of the stranger PDA used to prove the hand-off is authorized.
pub const ESCROW_SIGNER_STRANGER_SEED: &[u8] = b"fraccheck2:stranger:v1";

// Both domains above are `find_program_address` seeds, so both are held inside
// Solana's per-seed maximum at compile time rather than by whoever next edits
// the string. The limit has one author -- `dclutch-claims`'s, which this
// program already depends on -- because a test program restating "32" would be
// a second place for the number to be wrong.
//
// This guard exists because its absence had already cost something: the
// fractional claim-check domain shipped at 33 bytes and was underivable, and
// nothing in the family stopped it.
const _: () = assert!(
    ESCROW_SIGNER_ROOT_STAND_IN_SEED.len()
        <= dclutch_claims::fractional_claim_check_v1::MAX_PDA_SEED_BYTES_V1,
    "the root stand-in domain must be a usable PDA seed"
);
const _: () = assert!(
    ESCROW_SIGNER_STRANGER_SEED.len()
        <= dclutch_claims::fractional_claim_check_v1::MAX_PDA_SEED_BYTES_V1,
    "the stranger domain must be a usable PDA seed"
);

/// Exact instruction width: one action byte, the aggregate, amount, decimals.
pub const ESCROW_SIGNER_INSTRUCTION_BYTES: usize = 1 + 32 + 8 + 1;

/// Exact account frame this program forwards.
pub const ESCROW_SIGNER_ACCOUNT_COUNT: usize = 7;

/// Token program coordinate.
pub const ESCROW_SIGNER_TOKEN_PROGRAM: usize = 0;
/// Shard Mint coordinate.
pub const ESCROW_SIGNER_MINT: usize = 1;
/// Holder shard token account coordinate.
pub const ESCROW_SIGNER_HOLDER_TOKENS: usize = 2;
/// Claim-check escrow PDA coordinate.
pub const ESCROW_SIGNER_ESCROW: usize = 3;
/// Fractional-root stand-in PDA coordinate.
pub const ESCROW_SIGNER_ROOT: usize = 4;
/// Holder wallet coordinate; signs at the top level, never here.
pub const ESCROW_SIGNER_HOLDER: usize = 5;
/// Stranger PDA coordinate.
pub const ESCROW_SIGNER_STRANGER: usize = 6;

/// What one invocation of this program does.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum EscrowSignerActionV1 {
    /// The root stand-in hands the permissioned burn to the escrow PDA.
    HandOverBurn = 0,
    /// The escrow PDA approves one holder-signed permissioned burn.
    ApproveBurn = 1,
    /// A stranger PDA attempts the hand-off. Must be refused by Token-2022.
    StrangerHandOver = 2,
    /// The root stand-in attempts a burn after handing the authority away.
    StaleRootBurn = 3,
    /// The root stand-in mints shards to the holder.
    ///
    /// Present so the campaign's Mint can name the root as its *mint* authority
    /// and still be funded. Without it the fixture would have to leave the mint
    /// authority with a keypair, and a Mint whose three authorities are not the
    /// one controller is not a shard Mint -- `read_mint` says so, which is the
    /// whole point of running the profile against these bytes.
    MintToHolder = 4,
}

impl EscrowSignerActionV1 {
    /// Decode one action byte, refusing every value outside the five defined.
    pub const fn decode(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::HandOverBurn),
            1 => Some(Self::ApproveBurn),
            2 => Some(Self::StrangerHandOver),
            3 => Some(Self::StaleRootBurn),
            4 => Some(Self::MintToHolder),
            _ => None,
        }
    }
}

/// Stable test-only escrow-signer refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ClaimCheckEscrowSignerError {
    /// Instruction width or action byte was malformed.
    Instruction = 0x10_C000,
    /// The account frame width, ownership, or writability refused.
    AccountFrame = 0x10_C001,
    /// A passed account was not at its derived address.
    Derivation = 0x10_C002,
    /// The Token-2022 instruction could not be built.
    TokenInstruction = 0x10_C003,
    /// Token-2022 refused.
    ///
    /// **Unreachable in a transaction result, and deliberately kept anyway.** A
    /// failed CPI is not recoverable: the runtime propagates the inner
    /// program's refusal and never consults what this program returns
    /// afterwards. So a stranger's hand-off surfaces Token-2022's own
    /// `OwnerMismatch` (`0x4`), not this code -- which is also what a validator
    /// log will show for the real Claims route, and the campaign asserts the
    /// Token code rather than this one for exactly that reason. The arm stays
    /// because deleting it would mean writing `invoke_signed(…)?` and implying
    /// this program has no opinion about a Token refusal.
    TokenCpi = 0x10_C004,
}

dclutch_refusal_registry::pin_refusal_band!(
    ClaimCheckEscrowSignerError,
    dclutch_refusal_registry::TEST_CLAIMS_CLAIM_CHECK_ESCROW_SIGNER_BASE,
    [
        Instruction,
        AccountFrame,
        Derivation,
        TokenInstruction,
        TokenCpi
    ]
);

/// The claim-check escrow address for one aggregate, under one program.
///
/// Exported so a campaign derives the address it will pass with exactly the
/// code the program derives the address it will sign with. A test that computed
/// the expected address a second way would be testing its own second way.
pub fn escrow_address(aggregate: [u8; 32], program_id: &Pubkey) -> Option<(Pubkey, u8)> {
    let seeds = ClaimCheckEscrowSeedsV1::new(aggregate).ok()?;
    Some(Pubkey::find_program_address(&seeds.as_slices(), program_id))
}

/// The Fractional-root stand-in address for one aggregate.
pub fn root_stand_in_address(aggregate: [u8; 32], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ESCROW_SIGNER_ROOT_STAND_IN_SEED, &aggregate], program_id)
}

/// The stranger address for one aggregate.
pub fn stranger_address(aggregate: [u8; 32], program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[ESCROW_SIGNER_STRANGER_SEED, &aggregate], program_id)
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Sign one Token-2022 authority hand-off or burn approval as a derived PDA.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != ESCROW_SIGNER_INSTRUCTION_BYTES
        || accounts.len() != ESCROW_SIGNER_ACCOUNT_COUNT
    {
        return Err(ClaimCheckEscrowSignerError::Instruction.into());
    }
    let action = instruction_data
        .first()
        .copied()
        .and_then(EscrowSignerActionV1::decode)
        .ok_or(ClaimCheckEscrowSignerError::Instruction)?;
    let aggregate: [u8; 32] = instruction_data
        .get(1..33)
        .ok_or(ClaimCheckEscrowSignerError::Instruction)?
        .try_into()
        .map_err(|_| ClaimCheckEscrowSignerError::Instruction)?;
    let amount = u64::from_le_bytes(
        instruction_data
            .get(33..41)
            .ok_or(ClaimCheckEscrowSignerError::Instruction)?
            .try_into()
            .map_err(|_| ClaimCheckEscrowSignerError::Instruction)?,
    );
    let decimals = *instruction_data
        .get(41)
        .ok_or(ClaimCheckEscrowSignerError::Instruction)?;

    let account = |index: usize| {
        accounts
            .get(index)
            .ok_or(ClaimCheckEscrowSignerError::AccountFrame)
    };
    let token_program = account(ESCROW_SIGNER_TOKEN_PROGRAM)?;
    let mint = account(ESCROW_SIGNER_MINT)?;
    let holder_tokens = account(ESCROW_SIGNER_HOLDER_TOKENS)?;
    let escrow = account(ESCROW_SIGNER_ESCROW)?;
    let root = account(ESCROW_SIGNER_ROOT)?;
    let holder = account(ESCROW_SIGNER_HOLDER)?;
    let stranger = account(ESCROW_SIGNER_STRANGER)?;
    if !token_program.executable || !mint.is_writable || !holder_tokens.is_writable {
        return Err(ClaimCheckEscrowSignerError::AccountFrame.into());
    }

    // Every address this program will sign for is re-derived here rather than
    // trusted from the frame, so a campaign that passed the wrong account gets
    // a named refusal instead of a `create_program_address` failure inside the
    // runtime.
    let escrow_seeds = ClaimCheckEscrowSeedsV1::new(aggregate)
        .map_err(|_| ClaimCheckEscrowSignerError::Derivation)?;
    let (expected_escrow, escrow_bump) =
        Pubkey::find_program_address(&escrow_seeds.as_slices(), program_id);
    let (expected_root, root_bump) = root_stand_in_address(aggregate, program_id);
    let (expected_stranger, stranger_bump) = stranger_address(aggregate, program_id);
    if escrow.key != &expected_escrow
        || root.key != &expected_root
        || stranger.key != &expected_stranger
    {
        return Err(ClaimCheckEscrowSignerError::Derivation.into());
    }

    let escrow_signer = escrow_seeds.with_bump(escrow_bump);
    let [escrow_domain, escrow_aggregate, escrow_bump_seed] = escrow_signer.as_slices();
    let root_bump_seed = [root_bump];
    let stranger_bump_seed = [stranger_bump];

    let (instruction, infos): (Instruction, Vec<AccountInfo<'_>>) = match action {
        EscrowSignerActionV1::HandOverBurn | EscrowSignerActionV1::StrangerHandOver => {
            let current = if action == EscrowSignerActionV1::HandOverBurn {
                root
            } else {
                stranger
            };
            let instruction = token_instruction::set_authority(
                token_program.key,
                mint.key,
                Some(escrow.key),
                token_instruction::AuthorityType::PermissionedBurn,
                current.key,
                &[],
            )
            .map_err(|_| ClaimCheckEscrowSignerError::TokenInstruction)?;
            (
                instruction,
                alloc::vec![mint.clone(), current.clone(), token_program.clone()],
            )
        }
        EscrowSignerActionV1::MintToHolder => {
            let instruction = token_instruction::mint_to_checked(
                token_program.key,
                mint.key,
                holder_tokens.key,
                root.key,
                &[],
                amount,
                decimals,
            )
            .map_err(|_| ClaimCheckEscrowSignerError::TokenInstruction)?;
            (
                instruction,
                alloc::vec![
                    mint.clone(),
                    holder_tokens.clone(),
                    root.clone(),
                    token_program.clone()
                ],
            )
        }
        EscrowSignerActionV1::ApproveBurn | EscrowSignerActionV1::StaleRootBurn => {
            let approver = if action == EscrowSignerActionV1::ApproveBurn {
                escrow
            } else {
                root
            };
            if !holder.is_signer {
                return Err(ClaimCheckEscrowSignerError::AccountFrame.into());
            }
            let instruction = permissioned_burn::burn_checked(
                token_program.key,
                holder_tokens.key,
                mint.key,
                approver.key,
                holder.key,
                &[],
                amount,
                decimals,
            )
            .map_err(|_| ClaimCheckEscrowSignerError::TokenInstruction)?;
            (
                instruction,
                alloc::vec![
                    holder_tokens.clone(),
                    mint.clone(),
                    approver.clone(),
                    holder.clone(),
                    token_program.clone()
                ],
            )
        }
    };

    // The signer set is exactly the one address this action authorizes. Passing
    // all three seed families on every action would mean a campaign could never
    // tell which signature the Token program actually consumed.
    let signers: &[&[&[u8]]] = match action {
        EscrowSignerActionV1::HandOverBurn
        | EscrowSignerActionV1::StaleRootBurn
        | EscrowSignerActionV1::MintToHolder => &[&[
            ESCROW_SIGNER_ROOT_STAND_IN_SEED,
            &aggregate,
            &root_bump_seed,
        ]],
        EscrowSignerActionV1::StrangerHandOver => {
            &[&[ESCROW_SIGNER_STRANGER_SEED, &aggregate, &stranger_bump_seed]]
        }
        EscrowSignerActionV1::ApproveBurn => {
            &[&[escrow_domain, escrow_aggregate, escrow_bump_seed]]
        }
    };
    let mut metas: Vec<AccountMeta> = Vec::with_capacity(instruction.accounts.len());
    metas.extend_from_slice(&instruction.accounts);
    let instruction = Instruction {
        program_id: instruction.program_id,
        accounts: metas,
        data: instruction.data,
    };
    invoke_signed(&instruction, &infos, signers)
        .map_err(|_| ClaimCheckEscrowSignerError::TokenCpi)?;
    Ok(())
}
