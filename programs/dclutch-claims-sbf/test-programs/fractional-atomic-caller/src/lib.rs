#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF caller for the production Fractional atomic Claims route.
//!
//! The Claims handler `fractional_atomic_v3` is reachable only from a program
//! that can sign two distinct PDAs at once: the release-scoped Trading
//! caller-authority for coordinate zero, and the Trading-owned Fractional root
//! for coordinate twenty-six. No test in the repository had such a caller, so
//! the shipped handler had never executed. This program is that caller and
//! nothing more. It owns no protocol state and publishes no production ABI.
//!
//! It forwards the exact 31-account frame and the exact 416-byte request
//! unchanged -- it never rewrites a coordinate or a byte -- validates the
//! receipt Claims returns, and can refuse afterwards so a late-failure rollback
//! is provable against real account state.

extern crate alloc;

use alloc::vec::Vec;
use dclutch_capability_program_contract::CapabilityRootHeaderV1;
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_ROOT_V3,
    FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4, FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2,
    FractionalAtomicReceiptV3, FractionalExposureActionV2, FractionalExposureRequestV2,
};
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::{get_return_data, invoke_signed},
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Exact test-only wrapper: one control byte then the exact family request.
pub const FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES: usize = 1 + FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2;

/// Coordinate of the Claims program inside this caller's own account list.
pub const FRACTIONAL_ATOMIC_TEST_CLAIMS_PROGRAM_COORDINATE: usize = 0;

const REQUEST_OFFSET: usize = 1;
const CALLER_AUTHORITY: usize = 0;

/// Stable test-only caller refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FractionalAtomicTestCallerError {
    /// Wrapper width, control byte, or family request bytes were malformed.
    Instruction = 0x10_B000,
    /// Claims program or forwarded account frame was malformed.
    AccountFrame = 0x10_B001,
    /// The forwarded root coordinate was not a decodable activated root.
    Root = 0x10_B002,
    /// Claims refused, or returned another producer or receipt commitment.
    ClaimsCpi = 0x10_B003,
    /// Deliberate refusal after Claims returned and the receipt validated.
    DeliberateLateFailure = 0x10_B004,
}

// Registered refusal band (`docs/decisions/0007-namespaced-refusal-codes.md`).
// The discriminants stay literal so a code seen in a validator log is greppable;
// these assertions are what stops them drifting out of the allocated band.
const _: () = assert!(
    FractionalAtomicTestCallerError::Instruction as u32
        == dclutch_refusal_registry::TEST_CLAIMS_FRACTIONAL_ATOMIC_CALLER_BASE,
    "FractionalAtomicTestCallerError must start at its registered refusal band base"
);
const _: () = assert!(
    (FractionalAtomicTestCallerError::DeliberateLateFailure as u32)
        < dclutch_refusal_registry::TEST_CLAIMS_FRACTIONAL_ATOMIC_CALLER_BASE
            + dclutch_refusal_registry::BAND_SPAN,
    "FractionalAtomicTestCallerError must not run past its registered refusal band"
);

impl From<FractionalAtomicTestCallerError> for ProgramError {
    fn from(value: FractionalAtomicTestCallerError) -> Self {
        Self::Custom(value as u32)
    }
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Sign both Trading PDAs and forward one exact Fractional atomic request.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.len() != FRACTIONAL_ATOMIC_TEST_WRAPPER_BYTES {
        return Err(FractionalAtomicTestCallerError::Instruction.into());
    }
    let fail_after = *instruction_data
        .first()
        .ok_or(FractionalAtomicTestCallerError::Instruction)?;
    if fail_after > 1 {
        return Err(FractionalAtomicTestCallerError::Instruction.into());
    }
    let request_bytes = instruction_data
        .get(REQUEST_OFFSET..)
        .ok_or(FractionalAtomicTestCallerError::Instruction)?;
    let request = FractionalExposureRequestV2::decode(request_bytes)
        .map_err(|_| FractionalAtomicTestCallerError::Instruction)?;
    if !matches!(
        request.action(),
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap
    ) {
        return Err(FractionalAtomicTestCallerError::Instruction.into());
    }

    let claims_program = accounts
        .get(FRACTIONAL_ATOMIC_TEST_CLAIMS_PROGRAM_COORDINATE)
        .ok_or(FractionalAtomicTestCallerError::AccountFrame)?;
    let forwarded = accounts
        .get(1..)
        .ok_or(FractionalAtomicTestCallerError::AccountFrame)?;
    if !claims_program.executable
        || claims_program.is_signer
        || claims_program.is_writable
        || forwarded.len() != FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3
    {
        return Err(FractionalAtomicTestCallerError::AccountFrame.into());
    }

    // The root's own immutable header is the sole seed source. Deriving from
    // anything the caller was told would let a test fabricate a root PDA.
    let root_account = forwarded
        .get(FRACTIONAL_ATOMIC_ROOT_V3)
        .ok_or(FractionalAtomicTestCallerError::AccountFrame)?;
    let root_seeds = {
        let root_data = root_account
            .try_borrow_data()
            .map_err(|_| FractionalAtomicTestCallerError::Root)?;
        let header_bytes = root_data
            .get(..FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4)
            .ok_or(FractionalAtomicTestCallerError::Root)?;
        CapabilityRootHeaderV1::decode(header_bytes)
            .map_err(|_| FractionalAtomicTestCallerError::Root)?
            .seeds()
    };
    let (expected_root, root_bump) =
        Pubkey::find_program_address(&root_seeds.as_slices(), program_id);
    if root_account.key != &expected_root {
        return Err(FractionalAtomicTestCallerError::Root.into());
    }

    let input = request.input();
    let request_digest = hash(request_bytes).to_bytes();
    let caller_seeds = CallerAuthoritySeedsV1::from_bytes(
        input.release_set,
        input.market,
        ExecutionRoleV1::Trading,
        input.terms,
        request_digest,
    )
    .map_err(|_| FractionalAtomicTestCallerError::Instruction)?;
    let (expected_authority, caller_bump) =
        Pubkey::find_program_address(&caller_seeds.as_slices(), program_id);
    let authority_account = forwarded
        .get(CALLER_AUTHORITY)
        .ok_or(FractionalAtomicTestCallerError::AccountFrame)?;
    if authority_account.key != &expected_authority {
        return Err(FractionalAtomicTestCallerError::AccountFrame.into());
    }

    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        let signer = account.is_signer
            || index == CALLER_AUTHORITY
            || index == FRACTIONAL_ATOMIC_ROOT_V3;
        metas.push(if account.is_writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: request_bytes.to_vec(),
    };

    let caller_bump_seed = [caller_bump];
    let [caller_domain, caller_release, caller_market, caller_role, caller_context, caller_digest] =
        caller_seeds.as_slices();
    let root_bump_seed = [root_bump];
    let [
        root_domain,
        root_market,
        root_generation,
        root_manifest,
        root_entry,
        root_kind,
        root_release,
        root_config,
    ] = root_seeds.as_slices();

    let mut infos = Vec::with_capacity(accounts.len());
    infos.extend_from_slice(forwarded);
    infos.push(claims_program.clone());
    invoke_signed(
        &instruction,
        &infos,
        &[
            &[
                caller_domain,
                caller_release,
                caller_market,
                caller_role,
                caller_context,
                caller_digest,
                &caller_bump_seed,
            ],
            &[
                root_domain,
                root_market,
                root_generation,
                root_manifest,
                root_entry,
                root_kind,
                root_release,
                root_config,
                &root_bump_seed,
            ],
        ],
    )
    .map_err(|_| FractionalAtomicTestCallerError::ClaimsCpi)?;

    let (producer, receipt_bytes) =
        get_return_data().ok_or(FractionalAtomicTestCallerError::ClaimsCpi)?;
    let receipt = FractionalAtomicReceiptV3::decode(&receipt_bytes)
        .map_err(|_| FractionalAtomicTestCallerError::ClaimsCpi)?;
    if producer != *claims_program.key
        || receipt.action() != request.action()
        || receipt.request_digest() != request_digest
        || receipt.root() != expected_root.to_bytes()
    {
        return Err(FractionalAtomicTestCallerError::ClaimsCpi.into());
    }

    if fail_after == 1 {
        return Err(FractionalAtomicTestCallerError::DeliberateLateFailure.into());
    }
    Ok(())
}
