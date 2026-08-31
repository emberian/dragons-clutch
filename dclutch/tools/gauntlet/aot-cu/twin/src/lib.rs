#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Measurement-only twin of the stateless Direct V2 relation boundary.
//!
//! This crate exists to answer one question honestly: on a real SBF ELF, what
//! does it cost to evaluate the Lean-owned Direct V2 register relation by
//! *interpreting* the emitted `DCTV` program, versus by running the *ahead-of-
//! time* Rust implementation of the same relation?
//!
//! The comparison is only worth anything if nothing else differs. So the
//! request decode, bank decode, digesting, acknowledgement construction, and
//! acknowledgement encode below are compiled verbatim into both ELFs, and the
//! sole difference between them is which evaluator `evaluate_relation` calls.
//! Select it with exactly one of the `aot` or `interpreted` features; the
//! frame is shared source, so the measured CU difference is the evaluator and
//! nothing else.
//!
//! This is not a release artifact and is not deployable protocol. It holds no
//! account, writes no state, and makes no CPI, exactly like the comparison-only
//! accelerator whose frame it mirrors.

extern crate std;

use dclutch_core_contract::ContentId;
use dclutch_direct_aot_contract::{DIRECT_PROGRAM_V2_IDENTITIES, DIRECT_PROGRAM_V2_SCALARS};
use dclutch_execution_strategy_contract::{
    ACCELERATOR_ACK_HEADER_BYTES_V1, AcceleratorAckV1, AcceleratorRequestV1,
    decode_register_bank_into, encode_register_bank_into,
};
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, hash::hash, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};

#[cfg(any(
    all(feature = "aot", feature = "interpreted"),
    all(feature = "aot", feature = "null"),
    all(feature = "interpreted", feature = "null"),
))]
compile_error!("select exactly one evaluator: --features aot | interpreted | null");
#[cfg(not(any(feature = "aot", feature = "interpreted", feature = "null")))]
compile_error!("select exactly one evaluator: --features aot | interpreted | null");

/// Exact Direct scalar-bank width.
pub const SCALARS: usize = DIRECT_PROGRAM_V2_SCALARS as usize;
/// Exact Direct identity-bank width.
pub const IDENTITIES: usize = DIRECT_PROGRAM_V2_IDENTITIES as usize;
/// Exact Direct scalar-then-identity bank bytes.
pub const BANK_BYTES: usize = 456;
/// Exact Direct accelerator request bytes.
pub const REQUEST_BYTES: usize = 584;
/// Exact accepted Direct accelerator acknowledgement bytes.
pub const ACCEPTED_ACK_BYTES: usize = 616;
/// Exact refused Direct accelerator acknowledgement bytes.
pub const REFUSED_ACK_BYTES: usize = ACCELERATOR_ACK_HEADER_BYTES_V1;

const _: () = assert!(SCALARS == 41);
const _: () = assert!(IDENTITIES == 4);
const _: () = assert!(BANK_BYTES == 41 * 8 + 4 * 32);
const _: () = assert!(REQUEST_BYTES == 128 + BANK_BYTES);
const _: () = assert!(ACCEPTED_ACK_BYTES == 160 + BANK_BYTES);

/// Which evaluator this ELF was built with, published for harness assertions.
#[cfg(feature = "aot")]
pub const EVALUATOR: &str = "aot";
/// Which evaluator this ELF was built with, published for harness assertions.
#[cfg(feature = "interpreted")]
pub const EVALUATOR: &str = "interpreted";
/// Which evaluator this ELF was built with, published for harness assertions.
#[cfg(feature = "null")]
pub const EVALUATOR: &str = "null";

/// Physical refusal from the measurement twin.
///
/// The discriminants deliberately sit outside every registered production
/// refusal band: this artifact is never deployed, and a code seen in a log
/// must not be mistakable for a protocol refusal.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TwinError {
    /// The invocation supplied any account, signer, writable state, or child.
    NonStatelessFrame = 0x7E00,
    /// The request wire or its runtime bank counts were not exact Direct V2.
    InvalidRequest = 0x7E01,
    /// The scalar-then-identity input bank was malformed.
    InvalidBank = 0x7E02,
    /// An accepted output or acknowledgement could not be encoded canonically.
    InvalidAck = 0x7E03,
}

impl From<TwinError> for ProgramError {
    fn from(value: TwinError) -> Self {
        Self::Custom(value as u32)
    }
}

/// Classification shared by both evaluators.
///
/// A semantic refusal is a relation that evaluated to false and yields a
/// refusal acknowledgement; a physical refusal is a malformed frame and yields
/// a program error. Both evaluators map onto this identically, so acceptance
/// and refusal are directly comparable byte-for-byte.
enum Refusal {
    /// The relation evaluated to false.
    Semantic,
    /// The frame or bank was malformed.
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

/// Evaluate one exact stateless request and publish its canonical ack.
pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    if !accounts.is_empty() {
        return Err(TwinError::NonStatelessFrame.into());
    }
    let mut output = [0_u8; ACCEPTED_ACK_BYTES];
    let written = evaluate_into(instruction_data, &mut output)?;
    let ack = output.get(..written).ok_or(TwinError::InvalidAck)?;
    set_return_data(ack);
    Ok(())
}

/// Evaluate into caller-owned maximum acknowledgement storage.
///
/// The returned prefix length is 616 on acceptance and 160 on semantic
/// refusal. Physical decode errors return an error and no acknowledgement.
pub fn evaluate_into(
    instruction_data: &[u8],
    output: &mut [u8; ACCEPTED_ACK_BYTES],
) -> core::result::Result<usize, ProgramError> {
    let request =
        AcceleratorRequestV1::decode(instruction_data).map_err(|_| TwinError::InvalidRequest)?;
    if request.scalar_count() != DIRECT_PROGRAM_V2_SCALARS
        || request.identity_count() != DIRECT_PROGRAM_V2_IDENTITIES
        || request.bank().len() != BANK_BYTES
        || instruction_data.len() != REQUEST_BYTES
    {
        return Err(TwinError::InvalidRequest.into());
    }

    let mut input_scalars = [0_u64; SCALARS];
    let mut input_identities = [[0_u8; 32]; IDENTITIES];
    decode_register_bank_into(request.bank(), &mut input_scalars, &mut input_identities)
        .map_err(|_| TwinError::InvalidBank)?;
    let request_digest = content_digest(instruction_data)?;

    let mut scratch_scalars = [0_u64; SCALARS];
    let mut scratch_identities = [[0_u8; 32]; IDENTITIES];
    let mut candidate_scalars = [0_u64; SCALARS];
    let mut candidate_identities = [[0_u8; 32]; IDENTITIES];
    let result = evaluate_relation(
        &input_scalars,
        &input_identities,
        &mut scratch_scalars,
        &mut scratch_identities,
        &mut candidate_scalars,
        &mut candidate_identities,
    );

    match result {
        Ok(()) => {
            let mut bank = [0_u8; BANK_BYTES];
            encode_register_bank_into(&candidate_scalars, &candidate_identities, &mut bank)
                .map_err(|_| TwinError::InvalidBank)?;
            let bank_digest = content_digest(&bank)?;
            let ack = AcceleratorAckV1::accepted(request, request_digest, bank_digest, &bank)
                .map_err(|_| TwinError::InvalidAck)?;
            ack.encode_into(output).map_err(|_| TwinError::InvalidAck)?;
            Ok(ACCEPTED_ACK_BYTES)
        }
        Err(Refusal::Semantic) => {
            let ack = AcceleratorAckV1::refused(request, request_digest);
            let destination = output
                .get_mut(..REFUSED_ACK_BYTES)
                .ok_or(TwinError::InvalidAck)?;
            ack.encode_into(destination)
                .map_err(|_| TwinError::InvalidAck)?;
            Ok(REFUSED_ACK_BYTES)
        }
        Err(Refusal::Physical) => Err(TwinError::InvalidBank.into()),
    }
}

/// The ahead-of-time evaluator: the compiled Rust form of the relation.
#[cfg(feature = "aot")]
fn evaluate_relation(
    input_scalars: &[u64; SCALARS],
    input_identities: &[[u8; 32]; IDENTITIES],
    scratch_scalars: &mut [u64; SCALARS],
    scratch_identities: &mut [[u8; 32]; IDENTITIES],
    candidate_scalars: &mut [u64; SCALARS],
    candidate_identities: &mut [[u8; 32]; IDENTITIES],
) -> core::result::Result<(), Refusal> {
    use dclutch_direct_aot_contract::{Error, RegisterInput, RegisterOutput, execute_atomic};

    execute_atomic(
        RegisterInput {
            scalars: input_scalars,
            identities: input_identities,
        },
        RegisterOutput {
            scalars: scratch_scalars,
            identities: scratch_identities,
        },
        RegisterOutput {
            scalars: candidate_scalars,
            identities: candidate_identities,
        },
    )
    .map_err(|error| match error {
        Error::RegisterWidthMismatch => Refusal::Physical,
        Error::CheckFailed
        | Error::UnknownLifecycle
        | Error::ArithmeticOverflow
        | Error::InexactDivision
        | Error::ZeroDenominator => Refusal::Semantic,
    })
}

/// The interpreted evaluator: hostile-decode the emitted program, then run it.
///
/// The decode is inside the measured region on purpose. In the live route the
/// descriptor bytes are supplied rather than compiled in, so an interpreter
/// that trusted them without revalidating would not be the interpreter the
/// protocol actually requires.
#[cfg(feature = "interpreted")]
fn evaluate_relation(
    input_scalars: &[u64; SCALARS],
    input_identities: &[[u8; 32]; IDENTITIES],
    scratch_scalars: &mut [u64; SCALARS],
    scratch_identities: &mut [[u8; 32]; IDENTITIES],
    candidate_scalars: &mut [u64; SCALARS],
    candidate_identities: &mut [[u8; 32]; IDENTITIES],
) -> core::result::Result<(), Refusal> {
    use dclutch_direct_aot_contract::DIRECT_PROGRAM_V2;
    use dclutch_transition_vm::v2::{
        Error, ProgramV2, RegisterInput as VmInput, RegisterOutput as VmOutput,
        execute_atomic as execute_vm,
    };

    let program = ProgramV2::decode(&DIRECT_PROGRAM_V2).map_err(|_| Refusal::Physical)?;
    execute_vm(
        program,
        VmInput {
            scalars: input_scalars,
            identities: input_identities,
        },
        VmOutput {
            scalars: scratch_scalars,
            identities: scratch_identities,
        },
        VmOutput {
            scalars: candidate_scalars,
            identities: candidate_identities,
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

/// The null evaluator: the frame with the relation removed.
///
/// It performs the same input-to-scratch-to-output bank copies both real
/// evaluators perform and then returns, so its CU is everything the two ELFs
/// pay *other than* deciding the relation: request decode, bank decode, two
/// SHA-256 digests, acknowledgement construction, and encode. Subtracting it
/// turns the two headline numbers into evaluator-only costs, which is what a
/// per-instruction interpretation rate has to be derived from.
///
/// Its acknowledgement is deliberately not comparable to the other two: it
/// accepts unconditionally and copies the input through, so the harness reads
/// its compute units only.
#[cfg(feature = "null")]
fn evaluate_relation(
    input_scalars: &[u64; SCALARS],
    input_identities: &[[u8; 32]; IDENTITIES],
    scratch_scalars: &mut [u64; SCALARS],
    scratch_identities: &mut [[u8; 32]; IDENTITIES],
    candidate_scalars: &mut [u64; SCALARS],
    candidate_identities: &mut [[u8; 32]; IDENTITIES],
) -> core::result::Result<(), Refusal> {
    scratch_scalars.copy_from_slice(input_scalars);
    scratch_identities.copy_from_slice(input_identities);
    candidate_scalars.copy_from_slice(scratch_scalars);
    candidate_identities.copy_from_slice(scratch_identities);
    Ok(())
}

fn content_digest(bytes: &[u8]) -> core::result::Result<ContentId, ProgramError> {
    ContentId::new(hash(bytes).to_bytes()).map_err(|_| TwinError::InvalidAck.into())
}
