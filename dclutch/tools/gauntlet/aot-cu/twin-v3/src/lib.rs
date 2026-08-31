#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Measurement-only twin for the *current* Direct relation.
//!
//! The shipped stateless accelerator evaluates the superseded V2 descriptor.
//! The relation the live route actually runs is the InlineOrdinary
//! TransitionVMV3 program: 70 instructions, 1,712 bytes, folded over the
//! Product tail. Nothing has ever compiled that relation to SBF in either
//! form, so this crate does it in both and measures the difference.
//!
//! As with the V2 twin, one source is built three ways and only
//! `evaluate_relation` differs:
//!
//! * `aot` — `execute_inline_ordinary_atomic`, the hand-written translation;
//! * `interpreted` — `ProgramV3::decode` then the generic `execute_fold_atomic`;
//! * `null` — the surrounding work with the relation removed.
//!
//! Every build performs the same bank construction and the same transition
//! encode, so those costs cancel in the differences and are subtracted exactly
//! by the null build. The return data is a disposition byte followed by a
//! digest of the output banks, which lets the harness check that the two
//! evaluators agree on acceptance *and* on every output byte — refusal
//! equivalence for the current relation, on real ELFs.

extern crate std;

use dclutch_direct_codec::ordinary_v3::*;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};

#[cfg(any(
    all(feature = "aot", feature = "interpreted"),
    all(feature = "aot", feature = "null"),
    all(feature = "interpreted", feature = "null"),
    all(feature = "decode-only", feature = "aot"),
    all(feature = "decode-only", feature = "interpreted"),
    all(feature = "decode-only", feature = "null"),
))]
compile_error!("select exactly one evaluator: --features aot | interpreted | decode-only | null");
#[cfg(not(any(
    feature = "aot",
    feature = "interpreted",
    feature = "decode-only",
    feature = "null"
)))]
compile_error!("select exactly one evaluator: --features aot | interpreted | decode-only | null");

/// Product tail width this twin folds over, matching the differential corpus.
pub const N: u32 = 3;
/// Exact scalar-bank width at `N`.
pub const SCALARS: usize =
    DIRECT_ORDINARY_COMMON_SCALARS_V3 + 3 * (DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3 as usize);
/// Exact identity-bank width.
pub const IDENTITIES: usize = DIRECT_ORDINARY_COMMON_IDENTITIES_V3;
/// Return-data bytes: one disposition byte and one 32-byte bank digest.
pub const RETURN_BYTES: usize = 33;

/// The first seed index carrying a designed refusal.
pub const FIRST_REFUSAL_SEED: u32 = 24;

/// Physical refusal from the measurement twin, outside every production band.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TwinError {
    /// The invocation supplied any account.
    NonStatelessFrame = 0x7F00,
    /// The instruction data was not the exact four-byte seed selector.
    InvalidRequest = 0x7F01,
    /// The transition program could not be encoded or decoded.
    InvalidProgram = 0x7F02,
    /// The relation refused for a physical, non-semantic reason.
    InvalidBank = 0x7F03,
}

impl From<TwinError> for ProgramError {
    fn from(value: TwinError) -> Self {
        Self::Custom(value as u32)
    }
}

/// Classification shared by both evaluators.
enum Refusal {
    /// The relation evaluated to false.
    Semantic,
    /// The bank widths were wrong.
    Physical,
}

#[cfg(not(feature = "no-entrypoint"))]
solana_program::entrypoint_no_alloc!(program_entrypoint);

#[cfg(not(feature = "no-entrypoint"))]
fn program_entrypoint(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    process_instruction(program_id, accounts, instruction_data)
}

/// Build the bank for `seed`, evaluate the relation, and publish the outcome.
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if !accounts.is_empty() {
        return Err(TwinError::NonStatelessFrame.into());
    }
    let selector: [u8; 4] = instruction_data
        .get(..4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .ok_or(TwinError::InvalidRequest)?;
    if instruction_data.len() != 4 {
        return Err(TwinError::InvalidRequest.into());
    }
    let seed = u32::from_le_bytes(selector);

    // Shared by every build, so it cancels in the differences.
    let (input_scalars, input_identities) = seed_bank(seed);
    let mut transition_scratch = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
    let mut transition = [0_u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3];
    encode_direct_ordinary_transition_v3(&mut transition_scratch, &mut transition)
        .map_err(|_| TwinError::InvalidProgram)?;

    let mut scratch_scalars = [0_u64; SCALARS];
    let mut scratch_identities = [[0_u8; 32]; IDENTITIES];
    let mut output_scalars = [0_u64; SCALARS];
    let mut output_identities = [[0_u8; 32]; IDENTITIES];

    let result = evaluate_relation(
        &transition,
        &input_scalars,
        &input_identities,
        &mut scratch_scalars,
        &mut scratch_identities,
        &mut output_scalars,
        &mut output_identities,
    );

    let mut report = [0_u8; RETURN_BYTES];
    match result {
        Ok(()) => {
            *report.get_mut(0).ok_or(TwinError::InvalidBank)? = 1;
            let digest = bank_digest(&output_scalars, &output_identities);
            report
                .get_mut(1..)
                .ok_or(TwinError::InvalidBank)?
                .copy_from_slice(&digest);
        }
        Err(Refusal::Semantic) => {
            *report.get_mut(0).ok_or(TwinError::InvalidBank)? = 0;
        }
        Err(Refusal::Physical) => return Err(TwinError::InvalidBank.into()),
    }
    set_return_data(&report);
    Ok(())
}

/// The ahead-of-time evaluator: the hand-written translation.
#[cfg(feature = "aot")]
fn evaluate_relation(
    _transition: &[u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3],
    input_scalars: &[u64; SCALARS],
    input_identities: &[[u8; 32]; IDENTITIES],
    scratch_scalars: &mut [u64; SCALARS],
    scratch_identities: &mut [[u8; 32]; IDENTITIES],
    output_scalars: &mut [u64; SCALARS],
    output_identities: &mut [[u8; 32]; IDENTITIES],
) -> core::result::Result<(), Refusal> {
    use dclutch_direct_aot_v3_contract::{
        Error, RegisterInput, RegisterOutput, execute_inline_ordinary_atomic,
    };

    execute_inline_ordinary_atomic(
        N,
        RegisterInput {
            scalars: input_scalars,
            identities: input_identities,
        },
        RegisterOutput {
            scalars: scratch_scalars,
            identities: scratch_identities,
        },
        RegisterOutput {
            scalars: output_scalars,
            identities: output_identities,
        },
    )
    .map_err(|error| match error {
        Error::RegisterWidthMismatch => Refusal::Physical,
        _ => Refusal::Semantic,
    })
}

/// The interpreted evaluator: hostile-decode the emitted program, then fold it.
#[cfg(feature = "interpreted")]
fn evaluate_relation(
    transition: &[u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3],
    input_scalars: &[u64; SCALARS],
    input_identities: &[[u8; 32]; IDENTITIES],
    scratch_scalars: &mut [u64; SCALARS],
    scratch_identities: &mut [[u8; 32]; IDENTITIES],
    output_scalars: &mut [u64; SCALARS],
    output_identities: &mut [[u8; 32]; IDENTITIES],
) -> core::result::Result<(), Refusal> {
    use dclutch_transition_vm::v3::{
        Error, ProgramV3, RegisterInput as VmInput, RegisterOutput as VmOutput, execute_fold_atomic,
    };

    let program = ProgramV3::decode(transition).map_err(|_| Refusal::Physical)?;
    execute_fold_atomic(
        program,
        N,
        VmInput {
            scalars: input_scalars,
            identities: input_identities,
        },
        VmOutput {
            scalars: scratch_scalars,
            identities: scratch_identities,
        },
        VmOutput {
            scalars: output_scalars,
            identities: output_identities,
        },
    )
    .map_err(|error| match error {
        Error::CheckFailed
        | Error::UnknownLifecycle
        | Error::ArithmeticOverflow
        | Error::InexactDivision
        | Error::ZeroDenominator => Refusal::Semantic,
        _ => Refusal::Physical,
    })
}

/// The decode-only evaluator: hostile-decode the program and stop.
///
/// This exists because the live route does *not* pay for a full decode.
/// `hot_v3.rs:2250` calls `TransitionProgramV3::from_sealed`, which skips
/// `validate_body` -- the per-instruction sweep, and the expensive half of
/// `decode` -- because the write-once seal instruction already performed it.
/// A twin that calls `ProgramV3::decode` therefore charges the interpreter for
/// work the route performs once at seal time, not per invocation. Subtracting
/// this build from the interpreted build leaves the fold alone, which is the
/// only part an AOT translation actually displaces.
#[cfg(feature = "decode-only")]
fn evaluate_relation(
    transition: &[u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3],
    input_scalars: &[u64; SCALARS],
    input_identities: &[[u8; 32]; IDENTITIES],
    scratch_scalars: &mut [u64; SCALARS],
    scratch_identities: &mut [[u8; 32]; IDENTITIES],
    output_scalars: &mut [u64; SCALARS],
    output_identities: &mut [[u8; 32]; IDENTITIES],
) -> core::result::Result<(), Refusal> {
    use dclutch_transition_vm::v3::ProgramV3;

    let _program = ProgramV3::decode(transition).map_err(|_| Refusal::Physical)?;
    scratch_scalars.copy_from_slice(input_scalars);
    scratch_identities.copy_from_slice(input_identities);
    output_scalars.copy_from_slice(scratch_scalars);
    output_identities.copy_from_slice(scratch_identities);
    Ok(())
}

/// The null evaluator: everything but deciding the relation.
#[cfg(feature = "null")]
fn evaluate_relation(
    _transition: &[u8; DIRECT_ORDINARY_TRANSITION_BYTES_V3],
    input_scalars: &[u64; SCALARS],
    input_identities: &[[u8; 32]; IDENTITIES],
    scratch_scalars: &mut [u64; SCALARS],
    scratch_identities: &mut [[u8; 32]; IDENTITIES],
    output_scalars: &mut [u64; SCALARS],
    output_identities: &mut [[u8; 32]; IDENTITIES],
) -> core::result::Result<(), Refusal> {
    scratch_scalars.copy_from_slice(input_scalars);
    scratch_identities.copy_from_slice(input_identities);
    output_scalars.copy_from_slice(scratch_scalars);
    output_identities.copy_from_slice(scratch_identities);
    Ok(())
}

fn bank_digest(scalars: &[u64; SCALARS], identities: &[[u8; 32]; IDENTITIES]) -> [u8; 32] {
    let mut bytes = [0_u8; SCALARS * 8 + IDENTITIES * 32];
    let mut cursor = 0_usize;
    for value in scalars {
        if let Some(slot) = bytes.get_mut(cursor..cursor + 8) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        cursor += 8;
    }
    for identity in identities {
        if let Some(slot) = bytes.get_mut(cursor..cursor + 32) {
            slot.copy_from_slice(identity);
        }
        cursor += 32;
    }
    hash(&bytes).to_bytes()
}

/// Deterministic splitmix64, identical in every build.
fn mix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn pick(state: &mut u64, low: u64, high: u64) -> u64 {
    if high <= low {
        return low;
    }
    low + (mix(state) % (high - low + 1))
}

/// Build seed `seed`'s bank.
///
/// Seed 0 is the differential corpus baseline, which both executors accept.
/// Seeds 1..24 vary the branch-carrying scalars while staying admissible: the
/// price scale and execution price are held so `fill * price / scale` stays
/// exact, both sides share one policy fee rate, both nonces equal their next
/// nonce, and the slot stays inside both validity windows. Seeds 24.. each
/// violate one conjunct at a different depth.
#[must_use]
#[allow(clippy::indexing_slicing)]
pub fn seed_bank(seed: u32) -> ([u64; SCALARS], [[u8; 32]; IDENTITIES]) {
    let mut scalars = [0_u64; SCALARS];
    for (index, value) in [
        (SCALAR_SLOT_V3, 100),
        (SCALAR_SELLER_VALID_FROM_V3, 90),
        (SCALAR_SELLER_VALID_THROUGH_V3, 110),
        (SCALAR_BUYER_VALID_FROM_V3, 90),
        (SCALAR_BUYER_VALID_THROUGH_V3, 110),
        (SCALAR_BUYER_SIDE_V3, 1),
        (SCALAR_SELLER_GENERATION_V3, 7),
        (SCALAR_BUYER_GENERATION_V3, 7),
        (SCALAR_MARKET_GENERATION_V3, 7),
        (SCALAR_SELLER_OUTCOME_V3, 1),
        (SCALAR_BUYER_OUTCOME_V3, 1),
        (SCALAR_OUTCOME_COUNT_V3, N as u64),
        (SCALAR_SELLER_LIFECYCLE_V3, 1),
        (SCALAR_SELLER_MAXIMUM_V3, 10),
        (SCALAR_BUYER_LIFECYCLE_V3, 1),
        (SCALAR_BUYER_MAXIMUM_V3, 10),
        (SCALAR_SELLER_NONCE_V3, 1),
        (SCALAR_BUYER_NONCE_V3, 2),
        (SCALAR_SELLER_NEXT_NONCE_V3, 1),
        (SCALAR_BUYER_NEXT_NONCE_V3, 2),
        (SCALAR_SELLER_LIMIT_V3, 40),
        (SCALAR_EXECUTION_PRICE_V3, 50),
        (SCALAR_BUYER_LIMIT_V3, 60),
        (SCALAR_PRICE_SCALE_V3, 100),
        (SCALAR_SELLER_FEE_BPS_V3, 100),
        (SCALAR_BUYER_FEE_BPS_V3, 100),
        (SCALAR_POLICY_FEE_BPS_V3, 100),
        (SCALAR_FILL_V3, 10),
        (SCALAR_CUSTODY_REVISION_V3, 3),
        (SCALAR_ROOT_OPEN_COUNT_V3, 2),
    ] {
        scalars[index] = value;
    }
    for item in 0..(N as usize) {
        let base =
            DIRECT_ORDINARY_COMMON_SCALARS_V3 + item * (DIRECT_ORDINARY_ITEM_SCALAR_STRIDE_V3 as usize);
        scalars[base] = item as u64;
    }
    let mut identities = [[1_u8; 32]; IDENTITIES];
    identities[IDENTITY_MARKET_V3] = [2; 32];
    identities[IDENTITY_SELLER_INTENT_MARKET_V3] = [2; 32];
    identities[IDENTITY_BUYER_INTENT_MARKET_V3] = [2; 32];
    identities[IDENTITY_SELLER_NATIVE_SIGNER_V3] = [3; 32];
    identities[IDENTITY_SELLER_REQUEST_MAKER_V3] = [3; 32];
    identities[IDENTITY_BUYER_NATIVE_SIGNER_V3] = [4; 32];
    identities[IDENTITY_BUYER_REQUEST_MAKER_V3] = [4; 32];
    identities[IDENTITY_SELLER_COLLATERAL_REQUEST_V3] = [5; 32];
    identities[IDENTITY_SELLER_TOKEN_ACCOUNT_V3] = [5; 32];
    identities[IDENTITY_BUYER_COLLATERAL_REQUEST_V3] = [6; 32];
    identities[IDENTITY_BUYER_TOKEN_ACCOUNT_V3] = [6; 32];
    identities[IDENTITY_TRADING_PROGRAM_V3] = [7; 32];
    identities[IDENTITY_SELLER_STATE_OWNER_V3] = [7; 32];
    identities[IDENTITY_BUYER_STATE_OWNER_V3] = [7; 32];

    if seed == 0 {
        return (scalars, identities);
    }

    let mut state = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);

    // Even fills keep `fill * 50 / 100` exact at the baseline price and scale.
    let fill = 2 * pick(&mut state, 1, 20);
    scalars[SCALAR_FILL_V3] = fill;
    let maximum = fill + pick(&mut state, 0, 20);
    scalars[SCALAR_SELLER_MAXIMUM_V3] = maximum;
    scalars[SCALAR_BUYER_MAXIMUM_V3] = maximum;

    let fee_bps = pick(&mut state, 0, 500);
    scalars[SCALAR_SELLER_FEE_BPS_V3] = fee_bps;
    scalars[SCALAR_BUYER_FEE_BPS_V3] = fee_bps;
    scalars[SCALAR_POLICY_FEE_BPS_V3] = fee_bps;

    let slot = pick(&mut state, 90, 110);
    scalars[SCALAR_SLOT_V3] = slot;

    let generation = pick(&mut state, 0, 20);
    scalars[SCALAR_SELLER_GENERATION_V3] = generation;
    scalars[SCALAR_BUYER_GENERATION_V3] = generation;
    scalars[SCALAR_MARKET_GENERATION_V3] = generation;

    let nonce = pick(&mut state, 0, 1_000);
    scalars[SCALAR_SELLER_NONCE_V3] = nonce;
    scalars[SCALAR_SELLER_NEXT_NONCE_V3] = nonce;
    let buyer_nonce = pick(&mut state, 0, 1_000);
    scalars[SCALAR_BUYER_NONCE_V3] = buyer_nonce;
    scalars[SCALAR_BUYER_NEXT_NONCE_V3] = buyer_nonce;

    scalars[SCALAR_CUSTODY_REVISION_V3] = pick(&mut state, 0, 50);
    scalars[SCALAR_ROOT_OPEN_COUNT_V3] = pick(&mut state, 1, 50);

    if seed < FIRST_REFUSAL_SEED {
        return (scalars, identities);
    }
    match seed - FIRST_REFUSAL_SEED {
        // The slot falls outside the seller validity window.
        0 => scalars[SCALAR_SLOT_V3] = scalars[SCALAR_SELLER_VALID_THROUGH_V3] + 1,
        // A zero fill.
        1 => scalars[SCALAR_FILL_V3] = 0,
        // The two sides disagree about the Market generation.
        2 => scalars[SCALAR_SELLER_GENERATION_V3] = scalars[SCALAR_BUYER_GENERATION_V3] + 1,
        // The buyer takes the seller's side.
        3 => scalars[SCALAR_BUYER_SIDE_V3] = 0,
        // An unknown lifecycle tag.
        4 => scalars[SCALAR_SELLER_LIFECYCLE_V3] = 7,
        // The fill exceeds the stated maximum.
        5 => scalars[SCALAR_SELLER_MAXIMUM_V3] = scalars[SCALAR_FILL_V3] - 1,
        // The execution price falls outside the two limits.
        6 => scalars[SCALAR_EXECUTION_PRICE_V3] = scalars[SCALAR_BUYER_LIMIT_V3] + 1,
        // The replay nonce does not match its successor.
        _ => scalars[SCALAR_SELLER_NEXT_NONCE_V3] = scalars[SCALAR_SELLER_NONCE_V3] + 1,
    }
    (scalars, identities)
}
