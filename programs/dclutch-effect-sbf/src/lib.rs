#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Isolated SBF adapter for the Lean-owned dClutch Effect IR.
//!
//! This crate is a measurement artifact, not a complete market program. The
//! signer stored in the state projection is the explicit trust boundary for
//! semantic admission. See the adjacent README before interpreting results.

#[cfg(test)]
extern crate std;

use dclutch_effect_kernel::{Plan, State, execute};
use solana_program::{
    account_info::{AccountInfo, next_account_info},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Canonical projection account magic (`DCES`).
pub const STATE_MAGIC: [u8; 4] = *b"DCES";
/// Canonical projection account version.
pub const STATE_VERSION: u8 = 1;
/// Exact bytes in the experimental projection account.
pub const STATE_BYTES: usize = 104;

const AUTHORITY_START: usize = 8;
const OUTCOME_START: usize = 40;
const STATE_RESERVED_START: usize = 44;
const VALUES_START: usize = 48;

/// Stable experimental adapter refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterError {
    /// The account list was not exactly authority then projection.
    AccountFrame = 0,
    /// Signer, writable, executable, or alias privileges were wrong.
    AccountPrivilege = 1,
    /// The projection was not owned by this exact program.
    AccountOwner = 2,
    /// Projection bytes were not the one canonical V1 representation.
    AccountData = 3,
    /// The authority signer did not match the immutable stored authority.
    Authority = 4,
    /// The effect plan was not canonical or could not execute.
    Effect = 5,
    /// Account data borrowing refused.
    Borrow = 6,
}

impl From<AdapterError> for ProgramError {
    fn from(error: AdapterError) -> Self {
        Self::Custom(error as u32)
    }
}

/// Fixed-layout physical projection owned by the experimental executor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionV1 {
    /// Signer authorized to submit already-admitted semantic effects.
    pub semantic_authority: [u8; 32],
    /// Direct state projection consumed by the effect microkernel.
    pub state: State,
}

impl ProjectionV1 {
    /// Hostile-decode one exact canonical projection.
    pub fn decode(input: &[u8]) -> Result<Self, AdapterError> {
        if input.len() != STATE_BYTES
            || input.get(..STATE_MAGIC.len()) != Some(STATE_MAGIC.as_slice())
            || read_byte(input, 4)? != STATE_VERSION
            || input.get(5..8) != Some([0_u8; 3].as_slice())
            || input.get(STATE_RESERVED_START..VALUES_START) != Some([0_u8; 4].as_slice())
        {
            return Err(AdapterError::AccountData);
        }
        let semantic_authority = read_array_32(input, AUTHORITY_START)?;
        if semantic_authority == [0_u8; 32] {
            return Err(AdapterError::AccountData);
        }
        Ok(Self {
            semantic_authority,
            state: State {
                outcome: read_u32(input, OUTCOME_START)?,
                seller_next_nonce: read_u64(input, value_offset(0)?)?,
                buyer_next_nonce: read_u64(input, value_offset(1)?)?,
                seller_claims: read_u64(input, value_offset(2)?)?,
                buyer_claims: read_u64(input, value_offset(3)?)?,
                buyer_collateral: read_u64(input, value_offset(4)?)?,
                seller_collateral: read_u64(input, value_offset(5)?)?,
                venue_collateral: read_u64(input, value_offset(6)?)?,
            },
        })
    }

    /// Encode one projection into an exact caller-owned buffer.
    pub fn encode_into(&self, output: &mut [u8]) -> Result<(), AdapterError> {
        if output.len() != STATE_BYTES || self.semantic_authority == [0_u8; 32] {
            return Err(AdapterError::AccountData);
        }
        output.fill(0);
        write_slice(output, 0, &STATE_MAGIC)?;
        write_byte(output, 4, STATE_VERSION)?;
        write_slice(output, AUTHORITY_START, &self.semantic_authority)?;
        write_slice(output, OUTCOME_START, &self.state.outcome.to_le_bytes())?;
        write_u64(output, value_offset(0)?, self.state.seller_next_nonce)?;
        write_u64(output, value_offset(1)?, self.state.buyer_next_nonce)?;
        write_u64(output, value_offset(2)?, self.state.seller_claims)?;
        write_u64(output, value_offset(3)?, self.state.buyer_claims)?;
        write_u64(output, value_offset(4)?, self.state.buyer_collateral)?;
        write_u64(output, value_offset(5)?, self.state.seller_collateral)?;
        write_u64(output, value_offset(6)?, self.state.venue_collateral)
    }

    /// Return the canonical account bytes by value.
    pub fn to_bytes(&self) -> Result<[u8; STATE_BYTES], AdapterError> {
        let mut output = [0_u8; STATE_BYTES];
        self.encode_into(&mut output)?;
        Ok(output)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Execute one already-admitted effect plan against one owned projection.
///
/// Accounts are exactly: a read-only semantic-authority signer, followed by a
/// writable non-signer projection owned by this program.
#[inline(never)]
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != 2 {
        return Err(AdapterError::AccountFrame.into());
    }
    let mut iterator = accounts.iter();
    let authority = next_account_info(&mut iterator).map_err(|_| AdapterError::AccountFrame)?;
    let projection = next_account_info(&mut iterator).map_err(|_| AdapterError::AccountFrame)?;
    if !authority.is_signer
        || authority.is_writable
        || authority.executable
        || projection.is_signer
        || !projection.is_writable
        || projection.executable
        || authority.key == projection.key
    {
        return Err(AdapterError::AccountPrivilege.into());
    }
    if projection.owner != program_id {
        return Err(AdapterError::AccountOwner.into());
    }

    let mut decoded = {
        let data = projection
            .try_borrow_data()
            .map_err(|_| AdapterError::Borrow)?;
        ProjectionV1::decode(&data)?
    };
    if authority.key.to_bytes() != decoded.semantic_authority {
        return Err(AdapterError::Authority.into());
    }
    let plan = Plan::decode(instruction_data).map_err(|_| AdapterError::Effect)?;
    execute(&plan, &mut decoded.state).map_err(|_| AdapterError::Effect)?;

    let next = decoded.to_bytes()?;
    let mut destination = projection
        .try_borrow_mut_data()
        .map_err(|_| AdapterError::Borrow)?;
    if destination.len() != STATE_BYTES {
        return Err(AdapterError::AccountData.into());
    }
    destination.copy_from_slice(&next);
    Ok(())
}

fn value_offset(index: usize) -> Result<usize, AdapterError> {
    VALUES_START
        .checked_add(index.checked_mul(8).ok_or(AdapterError::AccountData)?)
        .ok_or(AdapterError::AccountData)
}

fn checked_end(offset: usize, width: usize) -> Result<usize, AdapterError> {
    offset.checked_add(width).ok_or(AdapterError::AccountData)
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8, AdapterError> {
    input.get(offset).copied().ok_or(AdapterError::AccountData)
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32, AdapterError> {
    let end = checked_end(offset, 4)?;
    let bytes: &[u8; 4] = input
        .get(offset..end)
        .ok_or(AdapterError::AccountData)?
        .try_into()
        .map_err(|_| AdapterError::AccountData)?;
    Ok(u32::from_le_bytes(*bytes))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, AdapterError> {
    let end = checked_end(offset, 8)?;
    let bytes: &[u8; 8] = input
        .get(offset..end)
        .ok_or(AdapterError::AccountData)?
        .try_into()
        .map_err(|_| AdapterError::AccountData)?;
    Ok(u64::from_le_bytes(*bytes))
}

fn read_array_32(input: &[u8], offset: usize) -> Result<[u8; 32], AdapterError> {
    let end = checked_end(offset, 32)?;
    input
        .get(offset..end)
        .ok_or(AdapterError::AccountData)?
        .try_into()
        .map_err(|_| AdapterError::AccountData)
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), AdapterError> {
    *output.get_mut(offset).ok_or(AdapterError::AccountData)? = value;
    Ok(())
}

fn write_u64(output: &mut [u8], offset: usize, value: u64) -> Result<(), AdapterError> {
    write_slice(output, offset, &value.to_le_bytes())
}

fn write_slice(output: &mut [u8], offset: usize, value: &[u8]) -> Result<(), AdapterError> {
    let end = checked_end(offset, value.len())?;
    output
        .get_mut(offset..end)
        .ok_or(AdapterError::AccountData)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{boxed::Box, vec::Vec};

    use super::*;

    const VECTOR_HEX: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../formal/dclutch-semantics/vectors/direct-inline-ordinary-v1.hex"
    ));

    fn decode_hex(value: &str) -> Vec<u8> {
        value
            .trim()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = core::str::from_utf8(pair).expect("fixture is UTF-8");
                u8::from_str_radix(pair, 16).expect("fixture is hexadecimal")
            })
            .collect()
    }

    fn fixture(authority: Pubkey) -> ProjectionV1 {
        ProjectionV1 {
            semantic_authority: authority.to_bytes(),
            state: State {
                outcome: 1,
                seller_next_nonce: 0,
                buyer_next_nonce: 0,
                seller_claims: 5_000,
                buyer_claims: 200,
                buyer_collateral: 2_000,
                seller_collateral: 100,
                venue_collateral: 20,
            },
        }
    }

    fn account(
        key: Pubkey,
        signer: bool,
        writable: bool,
        data: Vec<u8>,
        owner: Pubkey,
        executable: bool,
    ) -> AccountInfo<'static> {
        AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(1)),
            Box::leak(data.into_boxed_slice()),
            Box::leak(Box::new(owner)),
            executable,
        )
    }

    fn frame(program_id: Pubkey, authority: Pubkey) -> [AccountInfo<'static>; 2] {
        [
            account(authority, true, false, Vec::new(), Pubkey::default(), false),
            account(
                Pubkey::new_unique(),
                false,
                true,
                fixture(authority).to_bytes().expect("fixture").to_vec(),
                program_id,
                false,
            ),
        ]
    }

    #[test]
    fn projection_round_trips_canonically() {
        let projection = fixture(Pubkey::new_unique());
        let bytes = projection.to_bytes().expect("encode");
        assert_eq!(ProjectionV1::decode(&bytes), Ok(projection));

        let mut reserved = bytes;
        *reserved.get_mut(STATE_RESERVED_START).expect("reserved") = 1;
        assert_eq!(
            ProjectionV1::decode(&reserved),
            Err(AdapterError::AccountData)
        );
    }

    #[test]
    fn lean_plan_executes_through_physical_account_frame() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let accounts = frame(program_id, authority);
        process_instruction(&program_id, &accounts, &decode_hex(VECTOR_HEX)).expect("execute");
        let data = accounts[1].try_borrow_data().expect("state data");
        let post = ProjectionV1::decode(&data).expect("post state");
        assert_eq!(post.state.seller_next_nonce, 1);
        assert_eq!(post.state.buyer_next_nonce, 1);
        assert_eq!(post.state.seller_claims, 3_000);
        assert_eq!(post.state.buyer_claims, 2_200);
        assert_eq!(post.state.buyer_collateral, 998);
        assert_eq!(post.state.seller_collateral, 1_100);
        assert_eq!(post.state.venue_collateral, 22);
    }

    #[test]
    fn hostile_frames_and_late_effects_leave_account_unchanged() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let wrong_authority = frame(program_id, authority);
        let before = wrong_authority[1]
            .try_borrow_data()
            .expect("state")
            .to_vec();
        let impostor = account(
            Pubkey::new_unique(),
            true,
            false,
            Vec::new(),
            Pubkey::default(),
            false,
        );
        assert_eq!(
            process_instruction(
                &program_id,
                &[impostor, wrong_authority[1].clone()],
                &decode_hex(VECTOR_HEX)
            ),
            Err(AdapterError::Authority.into())
        );
        assert_eq!(
            wrong_authority[1]
                .try_borrow_data()
                .expect("unchanged")
                .as_ref(),
            before.as_slice()
        );

        let late = frame(program_id, authority);
        let before = late[1].try_borrow_data().expect("state").to_vec();
        let mut plan = decode_hex(VECTOR_HEX);
        let last_value = 8 + 6 * 16 + 8;
        plan.get_mut(last_value..last_value + 8)
            .expect("last value")
            .copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            process_instruction(&program_id, &late, &plan),
            Err(AdapterError::Effect.into())
        );
        assert_eq!(
            late[1].try_borrow_data().expect("unchanged").as_ref(),
            before.as_slice()
        );
    }
}
