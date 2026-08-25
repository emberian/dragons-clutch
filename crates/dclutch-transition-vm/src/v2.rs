//! Runtime-width transition programs over caller-owned register banks.
//!
//! V1's fixed arrays are a measured compatibility profile, not a semantic
//! limit. V2 declares the exact scalar and identity widths in every canonical
//! program header. Execution uses three caller-owned banks: immutable input,
//! scratch, and candidate output. A refusal may alter scratch, but it leaves
//! both input and candidate output byte-for-byte unchanged.

use core::convert::TryInto;

#[rustfmt::skip]
#[allow(missing_docs)]
mod generated;

use generated::{
    A_OFFSET, B_OFFSET, C_OFFSET, D_OFFSET, FLAGS_OFFSET, HEADER_RESERVED_OFFSET,
    IDENTITY_COUNT_OFFSET, IMMEDIATE_OFFSET, INSTRUCTION_COUNT_OFFSET,
    INSTRUCTION_RESERVED_BYTE_OFFSET, INSTRUCTION_RESERVED_OFFSET, OP_ADD_FITS_U64, OP_ADD_LE,
    OP_CHECKED_ADD_INTO, OP_CHECKED_MUL_INTO, OP_COPY_IDENTITY, OP_COPY_SCALAR, OP_IDENTITY_EQ,
    OP_IDENTITY_NE, OP_INCREMENT_INTO, OP_LIFECYCLE_ACCEPTS, OP_LOAD_CONST, OP_MAX_INTO,
    OP_MIN_INTO, OP_MUL_DIV_EXACT, OP_MUL_DIV_FLOOR, OP_NONZERO, OP_SCALAR_EQ, OP_SCALAR_LE,
    OP_SCALAR_LT, OP_SELECT_EQ, OP_SELECT_ZERO, OP_SUB_INTO, OPCODE_OFFSET, SCALAR_COUNT_OFFSET,
    VERSION_OFFSET,
};
pub use generated::{HEADER_BYTES, INSTRUCTION_BYTES, MAGIC, VERSION, WIDE_AGREEMENT_PROGRAM_V2};

/// Stable hostile-decode or checked-execution refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Program bytes did not have the exact count-derived width.
    InvalidLength,
    /// Magic did not identify a transition program.
    InvalidMagic,
    /// The encoded VM version is not implemented by this decoder.
    UnsupportedVersion,
    /// Header flags or reserved bytes were not canonical zeros.
    NonCanonicalHeader,
    /// The program contained no instruction or declared no register bank.
    EmptyProgramOrRegisters,
    /// An opcode is outside the V2 vocabulary.
    UnknownOpcode,
    /// Reserved bytes, unused operands, or an unused immediate were nonzero.
    NonCanonicalInstruction,
    /// An instruction indexed outside its declared runtime-width bank.
    InvalidRegister,
    /// A supplied input, scratch, or output bank had the wrong exact width.
    RegisterWidthMismatch,
    /// An admission relation evaluated to false.
    CheckFailed,
    /// A lifecycle scalar was not FOK, IOC, or GTC.
    UnknownLifecycle,
    /// Checked scalar arithmetic overflowed or underflowed.
    ArithmeticOverflow,
    /// Exact division had a zero divisor or nonzero remainder.
    InexactDivision,
    /// Floor division had a zero divisor.
    ZeroDenominator,
}

/// Result alias for V2 decode and execution.
pub type Result<T> = core::result::Result<T, Error>;

/// Hostile-decoded borrowed V2 program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramV2<'a> {
    instruction_count: u16,
    scalar_count: u16,
    identity_count: u16,
    bytes: &'a [u8],
}

impl<'a> ProgramV2<'a> {
    /// Decode and validate one exact runtime-width program.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..MAGIC.len()) != Some(MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if byte(bytes, VERSION_OFFSET)? != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if byte(bytes, FLAGS_OFFSET)? != 0
            || bytes
                .get(HEADER_RESERVED_OFFSET..HEADER_BYTES)
                .ok_or(Error::InvalidLength)?
                .iter()
                .any(|value| *value != 0)
        {
            return Err(Error::NonCanonicalHeader);
        }
        let instruction_count = read_u16(bytes, INSTRUCTION_COUNT_OFFSET)?;
        let scalar_count = read_u16(bytes, SCALAR_COUNT_OFFSET)?;
        let identity_count = read_u16(bytes, IDENTITY_COUNT_OFFSET)?;
        if instruction_count == 0 || (scalar_count == 0 && identity_count == 0) {
            return Err(Error::EmptyProgramOrRegisters);
        }
        let expected = usize::from(instruction_count)
            .checked_mul(INSTRUCTION_BYTES)
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let program = Self {
            instruction_count,
            scalar_count,
            identity_count,
            bytes,
        };
        let mut index = 0_u16;
        while index < instruction_count {
            program.validate_instruction(index)?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(program)
    }

    /// Return the exact encoded instruction count.
    pub const fn instruction_count(self) -> u16 {
        self.instruction_count
    }

    /// Return the exact required scalar-bank width.
    pub const fn scalar_count(self) -> u16 {
        self.scalar_count
    }

    /// Return the exact required identity-bank width.
    pub const fn identity_count(self) -> u16 {
        self.identity_count
    }

    /// Borrow the complete canonical bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    fn instruction_offset(self, index: u16) -> Result<usize> {
        if index >= self.instruction_count {
            return Err(Error::InvalidLength);
        }
        usize::from(index)
            .checked_mul(INSTRUCTION_BYTES)
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)
    }

    fn validate_instruction(self, index: u16) -> Result<()> {
        let offset = self.instruction_offset(index)?;
        let instruction = Instruction::decode(self.bytes, offset)?;
        instruction.validate(self.scalar_count, self.identity_count)
    }
}

/// Immutable runtime-width register projection supplied by an authenticated
/// account/profile adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterInput<'a> {
    /// Exact scalar values.
    pub scalars: &'a [u64],
    /// Exact 32-byte identity values.
    pub identities: &'a [[u8; 32]],
}

/// Mutable runtime-width candidate register bank supplied by the caller.
pub struct RegisterOutput<'a> {
    /// Exact scalar storage.
    pub scalars: &'a mut [u64],
    /// Exact identity storage.
    pub identities: &'a mut [[u8; 32]],
}

/// Execute atomically into caller-owned output using separate scratch space.
///
/// `output` is written only after the whole program accepts. On refusal,
/// `input` and `output` remain unchanged; only `scratch` may contain a rejected
/// candidate. Exact bank widths are part of the hostile-decoded program.
pub fn execute_atomic(
    program: ProgramV2<'_>,
    input: RegisterInput<'_>,
    scratch: RegisterOutput<'_>,
    output: RegisterOutput<'_>,
) -> Result<()> {
    require_widths(program, input.scalars.len(), input.identities.len())?;
    require_widths(program, scratch.scalars.len(), scratch.identities.len())?;
    require_widths(program, output.scalars.len(), output.identities.len())?;

    scratch.scalars.copy_from_slice(input.scalars);
    scratch.identities.copy_from_slice(input.identities);
    execute_candidate(program, scratch.scalars, scratch.identities)?;
    output.scalars.copy_from_slice(scratch.scalars);
    output.identities.copy_from_slice(scratch.identities);
    Ok(())
}

fn require_widths(program: ProgramV2<'_>, scalars: usize, identities: usize) -> Result<()> {
    if scalars == usize::from(program.scalar_count)
        && identities == usize::from(program.identity_count)
    {
        Ok(())
    } else {
        Err(Error::RegisterWidthMismatch)
    }
}

fn execute_candidate(
    program: ProgramV2<'_>,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<()> {
    let mut index = 0_u16;
    while index < program.instruction_count {
        let offset = program.instruction_offset(index)?;
        Instruction::decode(program.bytes, offset)?.execute(scalars, identities)?;
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Instruction {
    opcode: u8,
    a: u16,
    b: u16,
    c: u16,
    d: u16,
    immediate: u64,
}

impl Instruction {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        if byte(bytes, add(offset, INSTRUCTION_RESERVED_BYTE_OFFSET)?)? != 0
            || bytes
                .get(add(offset, INSTRUCTION_RESERVED_OFFSET)?..add(offset, IMMEDIATE_OFFSET)?)
                .ok_or(Error::InvalidLength)?
                .iter()
                .any(|value| *value != 0)
        {
            return Err(Error::NonCanonicalInstruction);
        }
        Ok(Self {
            opcode: byte(bytes, add(offset, OPCODE_OFFSET)?)?,
            a: read_u16(bytes, add(offset, A_OFFSET)?)?,
            b: read_u16(bytes, add(offset, B_OFFSET)?)?,
            c: read_u16(bytes, add(offset, C_OFFSET)?)?,
            d: read_u16(bytes, add(offset, D_OFFSET)?)?,
            immediate: read_u64(bytes, add(offset, IMMEDIATE_OFFSET)?)?,
        })
    }

    fn validate(self, scalar_count: u16, identity_count: u16) -> Result<()> {
        match self.opcode {
            OP_LOAD_CONST => {
                scalar(self.a, scalar_count)?;
                self.canonical([self.b, self.c, self.d], true)
            }
            OP_SCALAR_EQ | OP_SCALAR_LT | OP_SCALAR_LE | OP_ADD_FITS_U64 => {
                scalar(self.a, scalar_count)?;
                scalar(self.b, scalar_count)?;
                self.canonical([self.c, self.d, 0], false)
            }
            OP_IDENTITY_EQ | OP_IDENTITY_NE => {
                identity(self.a, identity_count)?;
                identity(self.b, identity_count)?;
                self.canonical([self.c, self.d, 0], false)
            }
            OP_NONZERO => {
                scalar(self.a, scalar_count)?;
                self.canonical([self.b, self.c, self.d], false)
            }
            OP_LIFECYCLE_ACCEPTS | OP_ADD_LE | OP_SUB_INTO | OP_SELECT_ZERO => {
                scalar(self.a, scalar_count)?;
                scalar(self.b, scalar_count)?;
                scalar(self.c, scalar_count)?;
                self.canonical([self.d, 0, 0], false)
            }
            OP_INCREMENT_INTO => {
                scalar(self.a, scalar_count)?;
                scalar(self.b, scalar_count)?;
                self.canonical([self.c, self.d, 0], false)
            }
            OP_MUL_DIV_EXACT | OP_MUL_DIV_FLOOR | OP_SELECT_EQ => {
                scalar(self.a, scalar_count)?;
                scalar(self.b, scalar_count)?;
                scalar(self.c, scalar_count)?;
                scalar(self.d, scalar_count)?;
                self.canonical([0, 0, 0], false)
            }
            OP_CHECKED_ADD_INTO | OP_CHECKED_MUL_INTO | OP_MIN_INTO | OP_MAX_INTO => {
                scalar(self.a, scalar_count)?;
                scalar(self.b, scalar_count)?;
                scalar(self.c, scalar_count)?;
                self.canonical([self.d, 0, 0], false)
            }
            OP_COPY_SCALAR => {
                scalar(self.a, scalar_count)?;
                scalar(self.b, scalar_count)?;
                self.canonical([self.c, self.d, 0], false)
            }
            OP_COPY_IDENTITY => {
                identity(self.a, identity_count)?;
                identity(self.b, identity_count)?;
                self.canonical([self.c, self.d, 0], false)
            }
            _ => Err(Error::UnknownOpcode),
        }
    }

    fn canonical(self, unused: [u16; 3], immediate_allowed: bool) -> Result<()> {
        if unused != [0; 3] || (!immediate_allowed && self.immediate != 0) {
            Err(Error::NonCanonicalInstruction)
        } else {
            Ok(())
        }
    }

    fn execute(self, scalars: &mut [u64], identities: &mut [[u8; 32]]) -> Result<()> {
        match self.opcode {
            OP_LOAD_CONST => write_scalar(scalars, self.a, self.immediate),
            OP_SCALAR_EQ => require(read_scalar(scalars, self.a)? == read_scalar(scalars, self.b)?),
            OP_IDENTITY_EQ => {
                require(read_identity(identities, self.a)? == read_identity(identities, self.b)?)
            }
            OP_IDENTITY_NE => {
                require(read_identity(identities, self.a)? != read_identity(identities, self.b)?)
            }
            OP_SCALAR_LT => require(read_scalar(scalars, self.a)? < read_scalar(scalars, self.b)?),
            OP_SCALAR_LE => require(read_scalar(scalars, self.a)? <= read_scalar(scalars, self.b)?),
            OP_NONZERO => require(read_scalar(scalars, self.a)? != 0),
            OP_LIFECYCLE_ACCEPTS => {
                let lifecycle = read_scalar(scalars, self.a)?;
                let maximum = read_scalar(scalars, self.b)?;
                let fill = read_scalar(scalars, self.c)?;
                match lifecycle {
                    0 => require(fill == maximum),
                    1 | 2 => require(fill <= maximum),
                    _ => Err(Error::UnknownLifecycle),
                }
            }
            OP_INCREMENT_INTO => {
                let next = read_scalar(scalars, self.a)?
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
                write_scalar(scalars, self.b, next)
            }
            OP_MUL_DIV_EXACT => mul_div(scalars, self.a, self.b, self.c, self.d, true),
            OP_MUL_DIV_FLOOR => mul_div(scalars, self.a, self.b, self.c, self.d, false),
            OP_ADD_LE => {
                let sum = u128::from(read_scalar(scalars, self.a)?)
                    + u128::from(read_scalar(scalars, self.b)?);
                require(sum <= u128::from(read_scalar(scalars, self.c)?))
            }
            OP_ADD_FITS_U64 => read_scalar(scalars, self.a)?
                .checked_add(read_scalar(scalars, self.b)?)
                .map(|_| ())
                .ok_or(Error::ArithmeticOverflow),
            OP_SUB_INTO => {
                let value = read_scalar(scalars, self.a)?
                    .checked_sub(read_scalar(scalars, self.b)?)
                    .ok_or(Error::CheckFailed)?;
                write_scalar(scalars, self.c, value)
            }
            OP_SELECT_EQ => {
                if read_scalar(scalars, self.a)? == read_scalar(scalars, self.b)? {
                    let value = read_scalar(scalars, self.c)?;
                    write_scalar(scalars, self.d, value)?;
                } else {
                    let _ = read_scalar(scalars, self.d)?;
                }
                Ok(())
            }
            OP_SELECT_ZERO => {
                if read_scalar(scalars, self.a)? == 0 {
                    let value = read_scalar(scalars, self.b)?;
                    write_scalar(scalars, self.c, value)?;
                } else {
                    let _ = read_scalar(scalars, self.c)?;
                }
                Ok(())
            }
            OP_CHECKED_ADD_INTO => {
                checked_binary(scalars, self.a, self.b, self.c, u64::checked_add)
            }
            OP_CHECKED_MUL_INTO => {
                checked_binary(scalars, self.a, self.b, self.c, u64::checked_mul)
            }
            OP_MIN_INTO => {
                let value = read_scalar(scalars, self.a)?.min(read_scalar(scalars, self.b)?);
                write_scalar(scalars, self.c, value)
            }
            OP_MAX_INTO => {
                let value = read_scalar(scalars, self.a)?.max(read_scalar(scalars, self.b)?);
                write_scalar(scalars, self.c, value)
            }
            OP_COPY_SCALAR => {
                let value = read_scalar(scalars, self.a)?;
                write_scalar(scalars, self.b, value)
            }
            OP_COPY_IDENTITY => {
                let value = read_identity(identities, self.a)?;
                write_identity(identities, self.b, value)
            }
            _ => Err(Error::UnknownOpcode),
        }
    }
}

fn checked_binary(
    scalars: &mut [u64],
    left: u16,
    right: u16,
    destination: u16,
    operation: fn(u64, u64) -> Option<u64>,
) -> Result<()> {
    let value = operation(read_scalar(scalars, left)?, read_scalar(scalars, right)?)
        .ok_or(Error::ArithmeticOverflow)?;
    write_scalar(scalars, destination, value)
}

fn mul_div(
    scalars: &mut [u64],
    left: u16,
    right: u16,
    denominator: u16,
    destination: u16,
    exact: bool,
) -> Result<()> {
    let denominator = u128::from(read_scalar(scalars, denominator)?);
    if denominator == 0 {
        return Err(if exact {
            Error::InexactDivision
        } else {
            Error::ZeroDenominator
        });
    }
    let numerator = u128::from(read_scalar(scalars, left)?)
        .checked_mul(u128::from(read_scalar(scalars, right)?))
        .ok_or(Error::ArithmeticOverflow)?;
    if exact && numerator % denominator != 0 {
        return Err(Error::InexactDivision);
    }
    let quotient = u64::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)?;
    write_scalar(scalars, destination, quotient)
}

fn scalar(index: u16, count: u16) -> Result<()> {
    if index < count {
        Ok(())
    } else {
        Err(Error::InvalidRegister)
    }
}

fn identity(index: u16, count: u16) -> Result<()> {
    if index < count {
        Ok(())
    } else {
        Err(Error::InvalidRegister)
    }
}

fn read_scalar(values: &[u64], index: u16) -> Result<u64> {
    values
        .get(usize::from(index))
        .copied()
        .ok_or(Error::InvalidRegister)
}

fn write_scalar(values: &mut [u64], index: u16, value: u64) -> Result<()> {
    *values
        .get_mut(usize::from(index))
        .ok_or(Error::InvalidRegister)? = value;
    Ok(())
}

fn read_identity(values: &[[u8; 32]], index: u16) -> Result<[u8; 32]> {
    values
        .get(usize::from(index))
        .copied()
        .ok_or(Error::InvalidRegister)
}

fn write_identity(values: &mut [[u8; 32]], index: u16, value: [u8; 32]) -> Result<()> {
    *values
        .get_mut(usize::from(index))
        .ok_or(Error::InvalidRegister)? = value;
    Ok(())
}

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::CheckFailed)
    }
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    let end = offset.checked_add(2).ok_or(Error::InvalidLength)?;
    Ok(u16::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(Error::InvalidLength)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    let end = offset.checked_add(8).ok_or(Error::InvalidLength)?;
    Ok(u64::from_le_bytes(
        bytes
            .get(offset..end)
            .ok_or(Error::InvalidLength)?
            .try_into()
            .map_err(|_| Error::InvalidLength)?,
    ))
}

fn add(left: usize, right: usize) -> Result<usize> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec;
    use std::vec::Vec;

    use super::*;

    fn put(output: &mut [u8], offset: usize, value: &[u8]) {
        let end = offset.checked_add(value.len()).expect("fixture offset");
        output
            .get_mut(offset..end)
            .expect("fixture destination")
            .copy_from_slice(value);
    }

    fn set(output: &mut [u8], offset: usize, value: u8) {
        *output.get_mut(offset).expect("fixture byte") = value;
    }

    fn instruction(opcode: u8, a: u16, b: u16, c: u16, d: u16, immediate: u64) -> [u8; 24] {
        let mut output = [0_u8; INSTRUCTION_BYTES];
        set(&mut output, OPCODE_OFFSET, opcode);
        put(&mut output, A_OFFSET, &a.to_le_bytes());
        put(&mut output, B_OFFSET, &b.to_le_bytes());
        put(&mut output, C_OFFSET, &c.to_le_bytes());
        put(&mut output, D_OFFSET, &d.to_le_bytes());
        put(&mut output, IMMEDIATE_OFFSET, &immediate.to_le_bytes());
        output
    }

    fn program(scalars: u16, identities: u16, instructions: &[[u8; 24]]) -> Vec<u8> {
        let mut output = vec![0_u8; HEADER_BYTES + instructions.len() * INSTRUCTION_BYTES];
        put(&mut output, 0, &MAGIC);
        set(&mut output, 4, VERSION);
        put(
            &mut output,
            INSTRUCTION_COUNT_OFFSET,
            &u16::try_from(instructions.len())
                .expect("fixture instruction count")
                .to_le_bytes(),
        );
        put(&mut output, SCALAR_COUNT_OFFSET, &scalars.to_le_bytes());
        put(
            &mut output,
            IDENTITY_COUNT_OFFSET,
            &identities.to_le_bytes(),
        );
        for (index, encoded) in instructions.iter().enumerate() {
            let start = HEADER_BYTES + index * INSTRUCTION_BYTES;
            put(&mut output, start, encoded);
        }
        output
    }

    #[test]
    fn runtime_width_is_declared_by_program_not_compiled_bank() {
        let bytes = program(
            300,
            257,
            &[
                instruction(0, 299, 0, 0, 0, 41),
                instruction(8, 299, 298, 0, 0, 0),
                instruction(21, 256, 255, 0, 0, 0),
            ],
        );
        assert_eq!(bytes.as_slice(), WIDE_AGREEMENT_PROGRAM_V2.as_slice());
        let decoded = ProgramV2::decode(&bytes).expect("runtime-width program");
        assert_eq!(decoded.scalar_count(), 300);
        assert_eq!(decoded.identity_count(), 257);

        let input_scalars = vec![0_u64; 300];
        let mut input_identities = vec![[0_u8; 32]; 257];
        *input_identities.get_mut(256).expect("identity register") = [9; 32];
        let mut scratch_scalars = vec![0_u64; 300];
        let mut scratch_identities = vec![[0_u8; 32]; 257];
        let mut output_scalars = vec![7_u64; 300];
        let mut output_identities = vec![[7_u8; 32]; 257];
        execute_atomic(
            decoded,
            RegisterInput {
                scalars: &input_scalars,
                identities: &input_identities,
            },
            RegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            RegisterOutput {
                scalars: &mut output_scalars,
                identities: &mut output_identities,
            },
        )
        .expect("runtime-width execution");
        assert_eq!(output_scalars.get(299), Some(&41));
        assert_eq!(output_scalars.get(298), Some(&42));
        assert_eq!(output_identities.get(255), Some(&[9; 32]));
    }

    #[test]
    fn late_refusal_preserves_candidate_output() {
        let bytes = program(
            3,
            0,
            &[
                instruction(0, 2, 0, 0, 0, 99),
                instruction(1, 0, 1, 0, 0, 0),
            ],
        );
        let decoded = ProgramV2::decode(&bytes).expect("program");
        let input_scalars = [1, 2, 3];
        let mut scratch_scalars = [0; 3];
        let mut output_scalars = [8, 8, 8];
        let before = output_scalars;
        assert_eq!(
            execute_atomic(
                decoded,
                RegisterInput {
                    scalars: &input_scalars,
                    identities: &[],
                },
                RegisterOutput {
                    scalars: &mut scratch_scalars,
                    identities: &mut [],
                },
                RegisterOutput {
                    scalars: &mut output_scalars,
                    identities: &mut [],
                },
            ),
            Err(Error::CheckFailed)
        );
        assert_eq!(output_scalars, before);
        assert_eq!(input_scalars, [1, 2, 3]);
        assert_eq!(scratch_scalars.get(2), Some(&99));
    }

    #[test]
    fn hostile_headers_instructions_and_widths_refuse() {
        let canonical = program(2, 1, &[instruction(1, 0, 1, 0, 0, 0)]);
        assert!(ProgramV2::decode(&canonical).is_ok());

        for length in 0..canonical.len() {
            assert_eq!(
                ProgramV2::decode(canonical.get(..length).expect("fixture prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut extended = canonical.clone();
        extended.push(0);
        assert_eq!(ProgramV2::decode(&extended), Err(Error::InvalidLength));

        for (offset, expected) in [
            (0, Error::InvalidMagic),
            (4, Error::UnsupportedVersion),
            (5, Error::NonCanonicalHeader),
            (12, Error::NonCanonicalHeader),
            (HEADER_BYTES + 1, Error::NonCanonicalInstruction),
            (HEADER_BYTES + 10, Error::NonCanonicalInstruction),
        ] {
            let mut hostile = canonical.clone();
            *hostile.get_mut(offset).expect("hostile offset") ^= 1;
            assert_eq!(ProgramV2::decode(&hostile), Err(expected));
        }

        let bad_register = program(2, 0, &[instruction(6, 2, 0, 0, 0, 0)]);
        assert_eq!(
            ProgramV2::decode(&bad_register),
            Err(Error::InvalidRegister)
        );
        let bad_unused = program(2, 0, &[instruction(6, 0, 1, 0, 0, 0)]);
        assert_eq!(
            ProgramV2::decode(&bad_unused),
            Err(Error::NonCanonicalInstruction)
        );
        let bad_immediate = program(2, 0, &[instruction(1, 0, 1, 0, 0, 1)]);
        assert_eq!(
            ProgramV2::decode(&bad_immediate),
            Err(Error::NonCanonicalInstruction)
        );
        let unknown = program(1, 0, &[instruction(255, 0, 0, 0, 0, 0)]);
        assert_eq!(ProgramV2::decode(&unknown), Err(Error::UnknownOpcode));

        let decoded = ProgramV2::decode(&canonical).expect("canonical");
        let input = [0_u64; 2];
        let mut short_scratch = [0_u64; 1];
        let mut output = [4_u64; 2];
        let before = output;
        assert_eq!(
            execute_atomic(
                decoded,
                RegisterInput {
                    scalars: &input,
                    identities: &[[0; 32]],
                },
                RegisterOutput {
                    scalars: &mut short_scratch,
                    identities: &mut [[0; 32]],
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut [[0; 32]],
                },
            ),
            Err(Error::RegisterWidthMismatch)
        );
        assert_eq!(output, before);
    }

    #[test]
    fn new_arithmetic_and_copy_ops_are_checked() {
        let bytes = program(
            9,
            2,
            &[
                instruction(16, 0, 1, 3, 0, 0),
                instruction(17, 0, 1, 4, 0, 0),
                instruction(18, 0, 1, 5, 0, 0),
                instruction(19, 0, 1, 6, 0, 0),
                instruction(20, 3, 7, 0, 0, 0),
                instruction(21, 0, 1, 0, 0, 0),
            ],
        );
        let decoded = ProgramV2::decode(&bytes).expect("program");
        let input_scalars = [6, 7, 0, 0, 0, 0, 0, 0, 0];
        let input_identities = [[3_u8; 32], [0_u8; 32]];
        let mut scratch_scalars = [0; 9];
        let mut scratch_identities = [[0; 32]; 2];
        let mut output_scalars = [0; 9];
        let mut output_identities = [[0; 32]; 2];
        execute_atomic(
            decoded,
            RegisterInput {
                scalars: &input_scalars,
                identities: &input_identities,
            },
            RegisterOutput {
                scalars: &mut scratch_scalars,
                identities: &mut scratch_identities,
            },
            RegisterOutput {
                scalars: &mut output_scalars,
                identities: &mut output_identities,
            },
        )
        .expect("program accepts");
        assert_eq!(output_scalars.get(3), Some(&13));
        assert_eq!(output_scalars.get(4), Some(&42));
        assert_eq!(output_scalars.get(5), Some(&6));
        assert_eq!(output_scalars.get(6), Some(&7));
        assert_eq!(output_scalars.get(7), Some(&13));
        assert_eq!(output_identities.get(1), Some(&[3; 32]));

        let overflow = program(3, 0, &[instruction(16, 0, 1, 2, 0, 0)]);
        let decoded = ProgramV2::decode(&overflow).expect("overflow program");
        let input = [u64::MAX, 1, 0];
        let mut scratch = [0; 3];
        let mut output = [5; 3];
        let before = output;
        assert_eq!(
            execute_atomic(
                decoded,
                RegisterInput {
                    scalars: &input,
                    identities: &[],
                },
                RegisterOutput {
                    scalars: &mut scratch,
                    identities: &mut [],
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut [],
                },
            ),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(output, before);
    }
}
