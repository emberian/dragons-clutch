#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Real SPL custody adapter for the Lean-owned two-transfer Direct plan.

extern crate std;

use dclutch_token_svm::{
    COption, ExactTransferProfileV1, LEGACY_TOKEN_PROGRAM_ID, transfer_checked,
};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Semantic-controller program identity pinned by this experiment.
pub const CONTROLLER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([67_u8; 32]);
/// PDA seed defining the semantic-controller authority.
pub const CONTROLLER_SEED: &[u8] = b"dclutch-controller-v1";
/// Exact bytes in the Lean-owned two-transfer custody plan.
pub const CUSTODY_PLAN_BYTES: usize = 40;

const CUSTODY_HEADER: [u8; 8] = [b'D', b'C', b'C', b'P', 1, 2, 0, 0];
const BUYER_TO_SELLER: [u8; 8] = [1, 0, 0, 0, 0, 0, 0, 0];
const BUYER_TO_VENUE: [u8; 8] = [1, 2, 0, 0, 0, 0, 0, 0];

/// Stable custody-adapter refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CustodyError {
    /// Account count or instruction width was not exact.
    AccountFrame = 0,
    /// Signer, writable, executable, or alias privileges were invalid.
    AccountPrivilege = 1,
    /// The caller was not the pinned controller PDA.
    ControllerAuthority = 2,
    /// The plan was not the canonical buyer-to-seller then buyer-to-venue form.
    Plan = 3,
    /// Token program, Mint, or Account bytes were outside the exact profile.
    TokenState = 4,
    /// Source authority, balance, or delegate allowance was insufficient.
    SourceAuthority = 5,
    /// Custody arithmetic overflowed.
    Arithmetic = 6,
    /// A real Token CPI refused.
    TokenCpi = 7,
    /// Complete token state after both CPIs differed from the derived state.
    Postcondition = 8,
}

impl From<CustodyError> for ProgramError {
    fn from(error: CustodyError) -> Self {
        Self::Custom(error as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(process_instruction);

/// Execute the exact two-transfer custody plan against legacy SPL Token.
#[inline(never)]
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != 7 || instruction_data.len() != CUSTODY_PLAN_BYTES {
        return Err(CustodyError::AccountFrame.into());
    }
    let (gross, fee) = decode_plan(instruction_data)?;
    let total = gross.checked_add(fee).ok_or(CustodyError::Arithmetic)?;

    let mut iterator = accounts.iter();
    let controller = next_account_info(&mut iterator).map_err(|_| CustodyError::AccountFrame)?;
    let replay = next_account_info(&mut iterator).map_err(|_| CustodyError::AccountFrame)?;
    let mint = next_account_info(&mut iterator).map_err(|_| CustodyError::AccountFrame)?;
    let source = next_account_info(&mut iterator).map_err(|_| CustodyError::AccountFrame)?;
    let seller = next_account_info(&mut iterator).map_err(|_| CustodyError::AccountFrame)?;
    let venue = next_account_info(&mut iterator).map_err(|_| CustodyError::AccountFrame)?;
    let token_program = next_account_info(&mut iterator).map_err(|_| CustodyError::AccountFrame)?;

    if !controller.is_signer
        || controller.is_writable
        || controller.executable
        || !replay.is_signer
        || replay.is_writable
        || replay.executable
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
        || token_program.is_signer
        || token_program.is_writable
        || !token_program.executable
        || !all_distinct([
            controller.key,
            replay.key,
            mint.key,
            source.key,
            seller.key,
            venue.key,
            token_program.key,
        ])
    {
        return Err(CustodyError::AccountPrivilege.into());
    }
    let (expected_controller, _) =
        Pubkey::find_program_address(&[CONTROLLER_SEED], &CONTROLLER_PROGRAM_ID);
    if controller.key != &expected_controller {
        return Err(CustodyError::ControllerAuthority.into());
    }
    let token_id = Pubkey::new_from_array(LEGACY_TOKEN_PROGRAM_ID);
    if token_program.key != &token_id
        || mint.owner != &token_id
        || source.owner != &token_id
        || seller.owner != &token_id
        || venue.owner != &token_id
    {
        return Err(CustodyError::TokenState.into());
    }

    let profile = ExactTransferProfileV1::LegacyExactTransferV1;
    let (mint_before, source_before, seller_before) = {
        let mint_data = mint
            .try_borrow_data()
            .map_err(|_| CustodyError::TokenState)?;
        let source_data = source
            .try_borrow_data()
            .map_err(|_| CustodyError::TokenState)?;
        let seller_data = seller
            .try_borrow_data()
            .map_err(|_| CustodyError::TokenState)?;
        (
            profile
                .check_mint(LEGACY_TOKEN_PROGRAM_ID, &mint_data)
                .map_err(|_| CustodyError::TokenState)?,
            profile
                .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &source_data)
                .map_err(|_| CustodyError::TokenState)?,
            profile
                .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &seller_data)
                .map_err(|_| CustodyError::TokenState)?,
        )
    };
    if source_before.mint != mint.key.to_bytes()
        || seller_before.mint != mint.key.to_bytes()
        || source_before.owner == replay.key.to_bytes()
        || source_before.delegate != COption::Some(replay.key.to_bytes())
        || source_before.delegated_amount < total
        || source_before.amount < total
    {
        return Err(CustodyError::SourceAuthority.into());
    }
    seller_before
        .amount
        .checked_add(gross)
        .ok_or(CustodyError::Arithmetic)?;
    if gross != 0 {
        invoke_transfer(
            source,
            mint,
            seller,
            replay,
            token_program,
            gross,
            mint_before.decimals,
        )?;
    }

    // Authenticate the second destination only after the first real CPI. A
    // hostile frozen/wrong venue therefore exercises transaction rollback of
    // the already completed first transfer rather than validation-only refusal.
    let venue_before = {
        let venue_data = venue
            .try_borrow_data()
            .map_err(|_| CustodyError::TokenState)?;
        profile
            .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &venue_data)
            .map_err(|_| CustodyError::TokenState)?
    };
    if venue_before.mint != mint.key.to_bytes() {
        return Err(CustodyError::TokenState.into());
    }
    venue_before
        .amount
        .checked_add(fee)
        .ok_or(CustodyError::Arithmetic)?;
    if fee != 0 {
        invoke_transfer(
            source,
            mint,
            venue,
            replay,
            token_program,
            fee,
            mint_before.decimals,
        )?;
    }

    let (mint_after, source_after, seller_after, venue_after) = {
        let mint_data = mint
            .try_borrow_data()
            .map_err(|_| CustodyError::Postcondition)?;
        let source_data = source
            .try_borrow_data()
            .map_err(|_| CustodyError::Postcondition)?;
        let seller_data = seller
            .try_borrow_data()
            .map_err(|_| CustodyError::Postcondition)?;
        let venue_data = venue
            .try_borrow_data()
            .map_err(|_| CustodyError::Postcondition)?;
        (
            profile
                .check_mint(LEGACY_TOKEN_PROGRAM_ID, &mint_data)
                .map_err(|_| CustodyError::Postcondition)?,
            profile
                .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &source_data)
                .map_err(|_| CustodyError::Postcondition)?,
            profile
                .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &seller_data)
                .map_err(|_| CustodyError::Postcondition)?,
            profile
                .check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &venue_data)
                .map_err(|_| CustodyError::Postcondition)?,
        )
    };
    let mut expected_source = source_before;
    expected_source.amount = expected_source
        .amount
        .checked_sub(total)
        .ok_or(CustodyError::Postcondition)?;
    if total != 0 {
        expected_source.delegated_amount = expected_source
            .delegated_amount
            .checked_sub(total)
            .ok_or(CustodyError::Postcondition)?;
        if expected_source.delegated_amount == 0 {
            expected_source.delegate = COption::None;
        }
    }
    let mut expected_seller = seller_before;
    expected_seller.amount = expected_seller
        .amount
        .checked_add(gross)
        .ok_or(CustodyError::Postcondition)?;
    let mut expected_venue = venue_before;
    expected_venue.amount = expected_venue
        .amount
        .checked_add(fee)
        .ok_or(CustodyError::Postcondition)?;
    if mint_after != mint_before
        || source_after != expected_source
        || seller_after != expected_seller
        || venue_after != expected_venue
    {
        return Err(CustodyError::Postcondition.into());
    }
    Ok(())
}

fn decode_plan(input: &[u8]) -> Result<(u64, u64), CustodyError> {
    if input.len() != CUSTODY_PLAN_BYTES
        || input.get(..8) != Some(CUSTODY_HEADER.as_slice())
        || input.get(8..16) != Some(BUYER_TO_SELLER.as_slice())
        || input.get(24..32) != Some(BUYER_TO_VENUE.as_slice())
    {
        return Err(CustodyError::Plan);
    }
    Ok((read_u64(input, 16)?, read_u64(input, 32)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, CustodyError> {
    let end = offset.checked_add(8).ok_or(CustodyError::Plan)?;
    let bytes: [u8; 8] = input
        .get(offset..end)
        .ok_or(CustodyError::Plan)?
        .try_into()
        .map_err(|_| CustodyError::Plan)?;
    Ok(u64::from_le_bytes(bytes))
}

fn all_distinct(keys: [&Pubkey; 7]) -> bool {
    for (left_index, left) in keys.iter().enumerate() {
        for right in keys.iter().skip(left_index + 1) {
            if left == right {
                return false;
            }
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn invoke_transfer<'info>(
    source: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
    decimals: u8,
) -> ProgramResult {
    let spec = transfer_checked(
        LEGACY_TOKEN_PROGRAM_ID,
        source.key.to_bytes(),
        mint.key.to_bytes(),
        destination.key.to_bytes(),
        authority.key.to_bytes(),
        amount,
        decimals,
    )
    .map_err(|_| CustodyError::TokenState)?;
    let instruction = Instruction {
        program_id: *token_program.key,
        accounts: std::vec![
            AccountMeta::new(*source.key, false),
            AccountMeta::new_readonly(*mint.key, false),
            AccountMeta::new(*destination.key, false),
            AccountMeta::new_readonly(*authority.key, true),
        ],
        data: std::vec::Vec::from(*spec.data()),
    };
    invoke(
        &instruction,
        &[
            source.clone(),
            mint.clone(),
            destination.clone(),
            authority.clone(),
            token_program.clone(),
        ],
    )
    .map_err(|_| CustodyError::TokenCpi.into())
}

#[cfg(test)]
mod tests {
    use std::vec::Vec;

    use super::*;

    const VECTOR_HEX: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../formal/dclutch-semantics/vectors/direct-inline-ordinary-custody-v1.hex"
    ));

    fn vector() -> Vec<u8> {
        VECTOR_HEX
            .trim()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = core::str::from_utf8(pair).expect("fixture is UTF-8");
                u8::from_str_radix(pair, 16).expect("fixture is hexadecimal")
            })
            .collect()
    }

    #[test]
    fn lean_plan_is_exact_and_hostile_shapes_refuse() {
        let plan = vector();
        assert_eq!(decode_plan(&plan), Ok((1_000, 2)));
        assert_eq!(
            decode_plan(plan.get(..39).unwrap_or(&[])),
            Err(CustodyError::Plan)
        );

        for offset in [0, 5, 8, 9, 10, 24, 25, 31] {
            let mut hostile = plan.clone();
            let _ = hostile.get_mut(offset).map(|byte| *byte ^= 1);
            assert_eq!(decode_plan(&hostile), Err(CustodyError::Plan));
        }
    }

    #[test]
    fn every_custody_role_must_be_distinct() {
        let first = Pubkey::new_unique();
        let second = Pubkey::new_unique();
        let third = Pubkey::new_unique();
        let fourth = Pubkey::new_unique();
        let fifth = Pubkey::new_unique();
        let sixth = Pubkey::new_unique();
        let seventh = Pubkey::new_unique();
        assert!(all_distinct([
            &first, &second, &third, &fourth, &fifth, &sixth, &seventh,
        ]));
        assert!(!all_distinct([
            &first, &second, &third, &fourth, &fifth, &sixth, &first,
        ]));
    }
}
