#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Test-only real-SBF caller for the Claims FoundingV5/V6 route.
//!
//! `founding_v5::process` refuses any authority that is not the PDA
//! `CallerAuthoritySeedsV1::from_bytes(release_set, market,
//! ExecutionRoleV1::Trading, founding_intent_digest, request_digest)` addresses
//! **under the request's own trading program**, and `invoke_signed` signs only
//! for the calling program's own addresses. In production that caller is
//! Trading's `generic_market_founding_v1::execute_claims`. Nothing in this
//! repository could produce that signature outside a live cluster, which is why
//! no program-test in this tree had ever executed a Claims founding at all.
//! This program is that caller and owns no protocol state and no production
//! ABI.
//!
//! # It declares no refusal code, and that is the point
//!
//! Every sibling test caller carries a registered refusal band. This one
//! deliberately does not. The subject under test is the CLAIMS route's
//! behaviour given a correctly-signed authority, and a wrapper code would be
//! exactly the `map_err(|_| Coarse)` this tree pays for repeatedly: a founding
//! that refuses `0x5185 ClaimsState` must reach the test as `0x5185
//! ClaimsState`, not as one undifferentiated wrapper code covering thirty-three
//! account conjuncts. So `invoke_signed`'s error is returned VERBATIM, and the
//! caller's own frame checks use the runtime's built-in `ProgramError`s, which
//! are outside every registered band and therefore cannot be mistaken for a
//! Claims refusal.
//!
//! # The residual, named rather than papered over
//!
//! `invoke_signed` signs for addresses under THIS program id, so the request
//! this caller forwards must name this program as its `trading_program`. What
//! is under test is therefore the Claims route's behaviour given a correctly
//! derived Trading authority, not Trading's derivation of it — which
//! `generic_market_founding_v1`'s own witnesses cover. The two halves meet in
//! the design, not in one test.

extern crate alloc;

use alloc::{vec, vec::Vec};

use dclutch_claims_svm::founding_v5::{
    CLAIMS_FOUNDING_ACCOUNT_COUNT_V6, CLAIMS_FOUNDING_REQUEST_BYTES_V5, ClaimsFoundingRequestV5,
};
use dclutch_core_contract::ContentId;
use dclutch_release_set_contract::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
};

/// Coordinate of the Claims program account this caller invokes.
///
/// The Claims program rides at the FRONT of this wrapper's frame and the
/// thirty-three founding accounts follow it verbatim, so the founding frame's
/// own indices are unmoved by the wrapper and a reader can compare this
/// caller's account list against `founding_v5`'s constants directly.
pub const CLAIMS_FOUNDING_TEST_CLAIMS_PROGRAM_COORDINATE: usize = 0;

/// Exact wrapper frame width: the Claims program, then the founding frame.
pub const CLAIMS_FOUNDING_TEST_ACCOUNT_COUNT: usize = 1 + CLAIMS_FOUNDING_ACCOUNT_COUNT_V6;

/// Writable coordinates INSIDE the forwarded founding frame.
///
/// Read off `founding_v5`'s own privilege pass rather than restated as a list
/// this file owns: aggregate, Position, admission and the two appended escrow
/// accounts, and nothing else. The escrow pair is writable on EVERY founding,
/// categorical included, because the frame is fixed and a caller may not signal
/// a Market's shape by which accounts it makes writable.
pub const CLAIMS_FOUNDING_TEST_WRITABLE_COORDINATES: [usize; 5] = [2, 3, 4, 31, 32];

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint!(process_instruction);

/// Forward one canonical Claims founding, signing the Trading caller authority.
///
/// The instruction data is the founding instruction verbatim — the 832-byte
/// request, the projected-custody lock receipt and the realization receipt —
/// and is passed through untouched. Only the REQUEST's bytes are hashed for the
/// authority seeds, because that is the digest `authenticate_authority` derives
/// with, and hashing the whole instruction here would produce an address the
/// route refuses with `0x518B CallerAuthority` for a reason no reader could see.
///
/// # Errors
///
/// Returns the runtime's own `ProgramError`s for a malformed frame, and
/// whatever the Claims founding returned, unchanged.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if accounts.len() != CLAIMS_FOUNDING_TEST_ACCOUNT_COUNT {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let claims_program = accounts
        .get(CLAIMS_FOUNDING_TEST_CLAIMS_PROGRAM_COORDINATE)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let forwarded = accounts
        .get(CLAIMS_FOUNDING_TEST_CLAIMS_PROGRAM_COORDINATE + 1..)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    if !claims_program.executable || claims_program.is_signer || claims_program.is_writable {
        return Err(ProgramError::InvalidArgument);
    }
    let request_bytes = instruction_data
        .get(..CLAIMS_FOUNDING_REQUEST_BYTES_V5)
        .ok_or(ProgramError::InvalidInstructionData)?;
    let request = ClaimsFoundingRequestV5::decode(request_bytes)
        .map_err(|_| ProgramError::InvalidInstructionData)?;
    // The request must name THIS program as its trading program, because
    // `invoke_signed` can sign only for addresses under this program id. A
    // request naming another one would derive an authority this caller cannot
    // produce, and the route would refuse `CallerAuthority` several checks
    // later with nothing in the log about why.
    if request.trading_program() != program_id.to_bytes() {
        return Err(ProgramError::IncorrectProgramId);
    }

    let mut metas = Vec::with_capacity(forwarded.len());
    for (index, account) in forwarded.iter().enumerate() {
        let signer = index == 0;
        let writable = CLAIMS_FOUNDING_TEST_WRITABLE_COORDINATES.contains(&index);
        metas.push(if writable {
            AccountMeta::new(*account.key, signer)
        } else {
            AccountMeta::new_readonly(*account.key, signer)
        });
    }
    let instruction = Instruction {
        program_id: *claims_program.key,
        accounts: metas,
        data: instruction_data.to_vec(),
    };
    let seeds = CallerAuthoritySeedsV1::from_bytes(
        request.release_set(),
        request.market(),
        ExecutionRoleV1::Trading,
        request.founding_intent_digest(),
        hash(request_bytes).to_bytes(),
    )
    .map_err(|_| ProgramError::InvalidInstructionData)?;
    let _ = ContentId::new(request.release_set()).map_err(|_| ProgramError::InvalidArgument)?;
    let bump = [Pubkey::find_program_address(&seeds.as_slices(), program_id).1];
    let [domain, release, market, role, context, digest] = seeds.as_slices();
    let mut infos = vec![];
    infos.extend_from_slice(forwarded);
    infos.push(claims_program.clone());
    // VERBATIM. The founding's own refusal is the whole output of this program.
    invoke_signed(
        &instruction,
        &infos,
        &[&[domain, release, market, role, context, digest, &bump]],
    )
}
