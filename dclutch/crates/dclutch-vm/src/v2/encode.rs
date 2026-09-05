//! Safe, allocation-free `TransitionVM` V2 artifact encoder.
//!
//! Typed constructors keep opcode and operand-position authority in this crate,
//! which is the semantic owner of the V2 instruction set. Without them an
//! artifact author has to restate `OPCODE_OFFSET`, `A_OFFSET` and the opcode
//! numbers, and the wire format acquires a second authority that drifts.
//!
//! The encoder builds into caller scratch, hostile-decodes the complete
//! candidate with [`ProgramV2::decode`], and copies to `output` only after the
//! decoder accepts. A refusal leaves `output` byte-for-byte unchanged, so the
//! encoder can never mint bytes the interpreter would reject.

use super::generated::{
    A_OFFSET, B_OFFSET, C_OFFSET, D_OFFSET, IDENTITY_COUNT_OFFSET, IMMEDIATE_OFFSET,
    INSTRUCTION_COUNT_OFFSET, OP_ADD_FITS_U64, OP_ADD_LE, OP_CHECKED_ADD_INTO, OP_CHECKED_MUL_INTO,
    OP_COPY_IDENTITY, OP_COPY_SCALAR, OP_IDENTITY_EQ, OP_IDENTITY_NE, OP_INCREMENT_INTO,
    OP_LIFECYCLE_ACCEPTS, OP_LOAD_CONST, OP_MAX_INTO, OP_MIN_INTO, OP_MUL_DIV_EXACT,
    OP_MUL_DIV_FLOOR, OP_NONZERO, OP_SCALAR_EQ, OP_SCALAR_LE, OP_SCALAR_LT, OP_SELECT_EQ,
    OP_SELECT_ZERO, OP_SUB_INTO, OPCODE_OFFSET, SCALAR_COUNT_OFFSET, VERSION_OFFSET,
};
use super::{Error, HEADER_BYTES, INSTRUCTION_BYTES, MAGIC, ProgramV2, VERSION};

/// Exact runtime register-bank widths declared by the encoded program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterGeometryV2 {
    /// Exact scalar-bank width every execution must supply.
    pub scalars: u16,
    /// Exact identity-bank width every execution must supply.
    pub identities: u16,
}

/// One typed V2 transition instruction.
///
/// Every constructor names its operands by role, so an author cannot silently
/// swap a destination for a source. Unused operand slots and the unused
/// immediate are held at the canonical zero the decoder requires.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransitionInstructionV2 {
    opcode: u8,
    a: u16,
    b: u16,
    c: u16,
    d: u16,
    immediate: u64,
}

impl TransitionInstructionV2 {
    const fn new(opcode: u8, a: u16, b: u16, c: u16, d: u16, immediate: u64) -> Self {
        Self {
            opcode,
            a,
            b,
            c,
            d,
            immediate,
        }
    }

    /// Write one program-constant scalar into a scalar register.
    #[must_use]
    pub const fn load_const(destination: u16, value: u64) -> Self {
        Self::new(OP_LOAD_CONST, destination, 0, 0, 0, value)
    }

    /// Require two scalar registers to be equal.
    #[must_use]
    pub const fn scalar_eq(left: u16, right: u16) -> Self {
        Self::new(OP_SCALAR_EQ, left, right, 0, 0, 0)
    }

    /// Require `left < right` over scalar registers.
    #[must_use]
    pub const fn scalar_lt(left: u16, right: u16) -> Self {
        Self::new(OP_SCALAR_LT, left, right, 0, 0, 0)
    }

    /// Require `left <= right` over scalar registers.
    #[must_use]
    pub const fn scalar_le(left: u16, right: u16) -> Self {
        Self::new(OP_SCALAR_LE, left, right, 0, 0, 0)
    }

    /// Require the checked sum of two scalar registers to fit `u64`.
    #[must_use]
    pub const fn add_fits_u64(left: u16, right: u16) -> Self {
        Self::new(OP_ADD_FITS_U64, left, right, 0, 0, 0)
    }

    /// Require two identity registers to be equal.
    #[must_use]
    pub const fn identity_eq(left: u16, right: u16) -> Self {
        Self::new(OP_IDENTITY_EQ, left, right, 0, 0, 0)
    }

    /// Require two identity registers to differ.
    #[must_use]
    pub const fn identity_ne(left: u16, right: u16) -> Self {
        Self::new(OP_IDENTITY_NE, left, right, 0, 0, 0)
    }

    /// Require one scalar register to be nonzero.
    #[must_use]
    pub const fn nonzero(scalar: u16) -> Self {
        Self::new(OP_NONZERO, scalar, 0, 0, 0, 0)
    }

    /// Require a fill against a maximum under a FOK/IOC/GTC lifecycle scalar.
    #[must_use]
    pub const fn lifecycle_accepts(lifecycle: u16, maximum: u16, fill: u16) -> Self {
        Self::new(OP_LIFECYCLE_ACCEPTS, lifecycle, maximum, fill, 0, 0)
    }

    /// Require `left + right <= bound` in exact 128-bit arithmetic.
    #[must_use]
    pub const fn add_le(left: u16, right: u16, bound: u16) -> Self {
        Self::new(OP_ADD_LE, left, right, bound, 0, 0)
    }

    /// Write `source + 1` into a scalar register, refusing overflow.
    #[must_use]
    pub const fn increment_into(source: u16, destination: u16) -> Self {
        Self::new(OP_INCREMENT_INTO, source, destination, 0, 0, 0)
    }

    /// Write `left - right`, refusing underflow.
    #[must_use]
    pub const fn sub_into(left: u16, right: u16, destination: u16) -> Self {
        Self::new(OP_SUB_INTO, left, right, destination, 0, 0)
    }

    /// Write the checked sum of two scalar registers.
    #[must_use]
    pub const fn checked_add_into(left: u16, right: u16, destination: u16) -> Self {
        Self::new(OP_CHECKED_ADD_INTO, left, right, destination, 0, 0)
    }

    /// Write the checked product of two scalar registers.
    #[must_use]
    pub const fn checked_mul_into(left: u16, right: u16, destination: u16) -> Self {
        Self::new(OP_CHECKED_MUL_INTO, left, right, destination, 0, 0)
    }

    /// Write the smaller of two scalar registers.
    #[must_use]
    pub const fn min_into(left: u16, right: u16, destination: u16) -> Self {
        Self::new(OP_MIN_INTO, left, right, destination, 0, 0)
    }

    /// Write the larger of two scalar registers.
    #[must_use]
    pub const fn max_into(left: u16, right: u16, destination: u16) -> Self {
        Self::new(OP_MAX_INTO, left, right, destination, 0, 0)
    }

    /// Write `left * right / denominator`, refusing a nonzero remainder.
    #[must_use]
    pub const fn mul_div_exact(left: u16, right: u16, denominator: u16, destination: u16) -> Self {
        Self::new(OP_MUL_DIV_EXACT, left, right, denominator, destination, 0)
    }

    /// Write `floor(left * right / denominator)`, refusing a zero denominator.
    #[must_use]
    pub const fn mul_div_floor(left: u16, right: u16, denominator: u16, destination: u16) -> Self {
        Self::new(OP_MUL_DIV_FLOOR, left, right, denominator, destination, 0)
    }

    /// Copy `value` into `destination` only when `left == right`.
    #[must_use]
    pub const fn select_eq(left: u16, right: u16, value: u16, destination: u16) -> Self {
        Self::new(OP_SELECT_EQ, left, right, value, destination, 0)
    }

    /// Copy `value` into `destination` only when `selector` is zero.
    #[must_use]
    pub const fn select_zero(selector: u16, value: u16, destination: u16) -> Self {
        Self::new(OP_SELECT_ZERO, selector, value, destination, 0, 0)
    }

    /// Copy one scalar register into another.
    #[must_use]
    pub const fn copy_scalar(source: u16, destination: u16) -> Self {
        Self::new(OP_COPY_SCALAR, source, destination, 0, 0, 0)
    }

    /// Copy one identity register into another.
    #[must_use]
    pub const fn copy_identity(source: u16, destination: u16) -> Self {
        Self::new(OP_COPY_IDENTITY, source, destination, 0, 0, 0)
    }
}

/// Exact encoded width of a V2 program with `instructions` instructions.
///
/// # Errors
///
/// Refuses a width that does not fit `usize`.
pub const fn transition_program_v2_bytes(instructions: usize) -> Result<usize, Error> {
    match instructions.checked_mul(INSTRUCTION_BYTES) {
        Some(body) => match HEADER_BYTES.checked_add(body) {
            Some(total) => Ok(total),
            None => Err(Error::InvalidLength),
        },
        None => Err(Error::InvalidLength),
    }
}

/// Encode one complete `TransitionVM` V2 program into caller-owned buffers.
///
/// `scratch` and `output` must both be exactly
/// [`transition_program_v2_bytes`] wide. The candidate is assembled in
/// `scratch`, hostile-decoded in full, and copied to `output` only on success:
/// on any refusal `output` is unchanged.
///
/// # Errors
///
/// Refuses buffer widths that differ from the exact encoded width, and every
/// refusal [`ProgramV2::decode`] itself raises against the candidate --
/// unknown opcodes, out-of-bank register coordinates, an empty program, and
/// declared banks of width zero.
pub fn encode_transition_program_v2_atomic(
    registers: RegisterGeometryV2,
    instructions: &[TransitionInstructionV2],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<(), Error> {
    let instruction_count = u16::try_from(instructions.len()).map_err(|_| Error::InvalidLength)?;
    let expected = transition_program_v2_bytes(instructions.len())?;
    if scratch.len() != expected || output.len() != expected {
        return Err(Error::InvalidLength);
    }
    scratch.fill(0);
    write(scratch, 0, &MAGIC)?;
    write_byte(scratch, VERSION_OFFSET, VERSION)?;
    for (offset, value) in [
        (INSTRUCTION_COUNT_OFFSET, instruction_count),
        (SCALAR_COUNT_OFFSET, registers.scalars),
        (IDENTITY_COUNT_OFFSET, registers.identities),
    ] {
        write(scratch, offset, &value.to_le_bytes())?;
    }
    let mut cursor = HEADER_BYTES;
    for instruction in instructions {
        encode_instruction(*instruction, scratch, cursor)?;
        cursor = add(cursor, INSTRUCTION_BYTES)?;
    }
    if cursor != expected {
        return Err(Error::InvalidLength);
    }
    ProgramV2::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

fn encode_instruction(
    instruction: TransitionInstructionV2,
    output: &mut [u8],
    offset: usize,
) -> Result<(), Error> {
    write_byte(output, add(offset, OPCODE_OFFSET)?, instruction.opcode)?;
    for (local, value) in [
        (A_OFFSET, instruction.a),
        (B_OFFSET, instruction.b),
        (C_OFFSET, instruction.c),
        (D_OFFSET, instruction.d),
    ] {
        write(output, add(offset, local)?, &value.to_le_bytes())?;
    }
    write(
        output,
        add(offset, IMMEDIATE_OFFSET)?,
        &instruction.immediate.to_le_bytes(),
    )
}

fn add(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

fn write(output: &mut [u8], offset: usize, bytes: &[u8]) -> Result<(), Error> {
    let end = add(offset, bytes.len())?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(bytes);
    Ok(())
}

fn write_byte(output: &mut [u8], offset: usize, value: u8) -> Result<(), Error> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}
