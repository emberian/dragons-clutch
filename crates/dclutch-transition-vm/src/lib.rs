#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-memory interpreter for Lean-owned dClutch transition programs.

/// Canonical transition-program magic (`DCTV`).
pub const MAGIC: [u8; 4] = *b"DCTV";
/// Canonical transition-program version.
pub const VERSION: u8 = 1;
/// Bytes in the canonical program header.
pub const HEADER_BYTES: usize = 8;
/// Bytes in one fixed transition instruction.
pub const INSTRUCTION_BYTES: usize = 16;
/// Maximum instructions in the first measured VM profile.
pub const MAX_INSTRUCTIONS: usize = 64;
/// Scalar registers in the first measured VM profile.
pub const MAX_SCALARS: usize = 64;
/// Exact-identity registers in the first measured VM profile.
pub const MAX_IDENTITIES: usize = 16;

/// Stable parser or execution refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Program bytes did not have their one exact count-derived width.
    InvalidLength,
    /// Program magic was not canonical.
    InvalidMagic,
    /// Program version is not implemented.
    UnsupportedVersion,
    /// Instruction count exceeded this measured profile.
    InvalidCount,
    /// Header or instruction reserved bytes were nonzero.
    NonzeroReserved,
    /// Opcode is not in the canonical V1 vocabulary.
    UnknownOpcode,
    /// An unused argument or immediate was nonzero.
    NoncanonicalInstruction,
    /// A scalar or identity register index exceeded its fixed bank.
    InvalidRegister,
    /// A checked admission relation was false.
    CheckFailed,
    /// A lifecycle tag was not FOK, IOC, or GTC.
    UnknownLifecycle,
    /// Checked scalar arithmetic overflowed.
    ArithmeticOverflow,
    /// Exact division had a zero denominator or nonzero remainder.
    InexactDivision,
    /// Floor division had a zero denominator.
    ZeroDenominator,
}

/// Fixed authenticated register frame supplied by an outer adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Registers {
    scalars: [u64; MAX_SCALARS],
    identities: [[u8; 32]; MAX_IDENTITIES],
}

impl Registers {
    /// Construct one zeroed fixed register frame.
    #[must_use]
    pub const fn zeroed() -> Self {
        Self {
            scalars: [0; MAX_SCALARS],
            identities: [[0; 32]; MAX_IDENTITIES],
        }
    }

    /// Return one scalar register.
    pub fn scalar(&self, index: usize) -> Result<u64, Error> {
        self.scalars
            .get(index)
            .copied()
            .ok_or(Error::InvalidRegister)
    }

    /// Set one scalar register before execution.
    pub fn set_scalar(&mut self, index: usize, value: u64) -> Result<(), Error> {
        *self.scalars.get_mut(index).ok_or(Error::InvalidRegister)? = value;
        Ok(())
    }

    /// Return one exact identity register.
    pub fn identity(&self, index: usize) -> Result<[u8; 32], Error> {
        self.identities
            .get(index)
            .copied()
            .ok_or(Error::InvalidRegister)
    }

    /// Set one exact identity register before execution.
    pub fn set_identity(&mut self, index: usize, value: [u8; 32]) -> Result<(), Error> {
        *self
            .identities
            .get_mut(index)
            .ok_or(Error::InvalidRegister)? = value;
        Ok(())
    }
}

impl Default for Registers {
    fn default() -> Self {
        Self::zeroed()
    }
}

/// Hostile-decode and transactionally execute one canonical program.
///
/// Register writes are committed only after all instructions succeed.
pub fn execute(program: &[u8], registers: &mut Registers) -> Result<(), Error> {
    let count = decode_header(program)?;
    let mut next = *registers;
    let mut index = 0_usize;
    while index < count {
        let offset = HEADER_BYTES
            .checked_add(
                index
                    .checked_mul(INSTRUCTION_BYTES)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)?;
        execute_instruction(program, offset, &mut next)?;
        index = index.checked_add(1).ok_or(Error::InvalidCount)?;
    }
    *registers = next;
    Ok(())
}

fn decode_header(program: &[u8]) -> Result<usize, Error> {
    if program.len() < HEADER_BYTES {
        return Err(Error::InvalidLength);
    }
    if program.get(..4) != Some(MAGIC.as_slice()) {
        return Err(Error::InvalidMagic);
    }
    if byte(program, 4)? != VERSION {
        return Err(Error::UnsupportedVersion);
    }
    let count = usize::from(byte(program, 5)?);
    if count > MAX_INSTRUCTIONS {
        return Err(Error::InvalidCount);
    }
    if byte(program, 6)? != 0 || byte(program, 7)? != 0 {
        return Err(Error::NonzeroReserved);
    }
    let expected = count
        .checked_mul(INSTRUCTION_BYTES)
        .and_then(|bytes| HEADER_BYTES.checked_add(bytes))
        .ok_or(Error::InvalidLength)?;
    if program.len() != expected {
        return Err(Error::InvalidLength);
    }
    Ok(count)
}

fn execute_instruction(
    program: &[u8],
    offset: usize,
    registers: &mut Registers,
) -> Result<(), Error> {
    let opcode = byte(program, offset)?;
    let a = usize::from(byte(program, add(offset, 1)?)?);
    let b = usize::from(byte(program, add(offset, 2)?)?);
    let c = usize::from(byte(program, add(offset, 3)?)?);
    let d = usize::from(byte(program, add(offset, 4)?)?);
    if byte(program, add(offset, 5)?)? != 0
        || byte(program, add(offset, 6)?)? != 0
        || byte(program, add(offset, 7)?)? != 0
    {
        return Err(Error::NonzeroReserved);
    }
    let immediate = read_u64(program, add(offset, 8)?)?;
    match opcode {
        0 => {
            canonical([b, c, d], immediate, true)?;
            registers.set_scalar(a, immediate)
        }
        1 => {
            canonical([c, d, 0], immediate, false)?;
            require(registers.scalar(a)? == registers.scalar(b)?)
        }
        2 => {
            canonical([c, d, 0], immediate, false)?;
            require(registers.identity(a)? == registers.identity(b)?)
        }
        3 => {
            canonical([c, d, 0], immediate, false)?;
            require(registers.identity(a)? != registers.identity(b)?)
        }
        4 => {
            canonical([c, d, 0], immediate, false)?;
            require(registers.scalar(a)? < registers.scalar(b)?)
        }
        5 => {
            canonical([c, d, 0], immediate, false)?;
            require(registers.scalar(a)? <= registers.scalar(b)?)
        }
        6 => {
            canonical([b, c, d], immediate, false)?;
            require(registers.scalar(a)? != 0)
        }
        7 => {
            canonical([d, 0, 0], immediate, false)?;
            let fill = registers.scalar(c)?;
            let maximum = registers.scalar(b)?;
            match registers.scalar(a)? {
                0 => require(fill == maximum),
                1 => require(fill <= maximum),
                2 => require(fill <= maximum),
                _ => Err(Error::UnknownLifecycle),
            }
        }
        8 => {
            canonical([c, d, 0], immediate, false)?;
            let next = registers
                .scalar(a)?
                .checked_add(1)
                .ok_or(Error::ArithmeticOverflow)?;
            registers.set_scalar(b, next)
        }
        9 => {
            canonical([0, 0, 0], immediate, false)?;
            mul_div(registers, a, b, c, d, true)
        }
        10 => {
            canonical([0, 0, 0], immediate, false)?;
            mul_div(registers, a, b, c, d, false)
        }
        11 => {
            canonical([d, 0, 0], immediate, false)?;
            let sum = u128::from(registers.scalar(a)?) + u128::from(registers.scalar(b)?);
            require(sum <= u128::from(registers.scalar(c)?))
        }
        12 => {
            canonical([c, d, 0], immediate, false)?;
            let sum = u128::from(registers.scalar(a)?) + u128::from(registers.scalar(b)?);
            require(sum <= u128::from(u64::MAX))
        }
        13 => {
            canonical([d, 0, 0], immediate, false)?;
            let difference = registers
                .scalar(a)?
                .checked_sub(registers.scalar(b)?)
                .ok_or(Error::CheckFailed)?;
            registers.set_scalar(c, difference)
        }
        14 => {
            canonical([0, 0, 0], immediate, false)?;
            let left = registers.scalar(a)?;
            let right = registers.scalar(b)?;
            let selected = registers.scalar(c)?;
            let _destination = registers.scalar(d)?;
            if left == right {
                registers.set_scalar(d, selected)
            } else {
                Ok(())
            }
        }
        15 => {
            canonical([d, 0, 0], immediate, false)?;
            let source = registers.scalar(a)?;
            let selected = registers.scalar(b)?;
            let _destination = registers.scalar(c)?;
            if source == 0 {
                registers.set_scalar(c, selected)
            } else {
                Ok(())
            }
        }
        _ => Err(Error::UnknownOpcode),
    }
}

fn mul_div(
    registers: &mut Registers,
    left: usize,
    right: usize,
    denominator: usize,
    destination: usize,
    exact: bool,
) -> Result<(), Error> {
    let denominator = u128::from(registers.scalar(denominator)?);
    if denominator == 0 {
        return Err(if exact {
            Error::InexactDivision
        } else {
            Error::ZeroDenominator
        });
    }
    let numerator = u128::from(registers.scalar(left)?)
        .checked_mul(u128::from(registers.scalar(right)?))
        .ok_or(Error::ArithmeticOverflow)?;
    if exact && numerator % denominator != 0 {
        return Err(Error::InexactDivision);
    }
    let quotient = numerator / denominator;
    let value = u64::try_from(quotient).map_err(|_| Error::ArithmeticOverflow)?;
    registers.set_scalar(destination, value)
}

fn canonical(unused: [usize; 3], immediate: u64, immediate_allowed: bool) -> Result<(), Error> {
    if unused != [0; 3] || (!immediate_allowed && immediate != 0) {
        Err(Error::NoncanonicalInstruction)
    } else {
        Ok(())
    }
}

fn require(condition: bool) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::CheckFailed)
    }
}

fn byte(input: &[u8], offset: usize) -> Result<u8, Error> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, Error> {
    let end = offset.checked_add(8).ok_or(Error::InvalidLength)?;
    let bytes: [u8; 8] = input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)?;
    Ok(u64::from_le_bytes(bytes))
}

fn add(left: usize, right: usize) -> Result<usize, Error> {
    left.checked_add(right).ok_or(Error::InvalidLength)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    const PROGRAM_HEX: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../formal/dclutch-semantics/vectors/direct-inline-ordinary-program-v1.hex"
    ));

    fn program() -> Vec<u8> {
        PROGRAM_HEX
            .trim()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = core::str::from_utf8(pair).expect("fixture is UTF-8");
                u8::from_str_radix(pair, 16).expect("fixture is hexadecimal")
            })
            .collect()
    }

    fn instruction(opcode: u8, a: u8, b: u8, c: u8, d: u8, immediate: u64) -> [u8; 16] {
        let mut encoded = [0_u8; 16];
        encoded[0] = opcode;
        encoded[1] = a;
        encoded[2] = b;
        encoded[3] = c;
        encoded[4] = d;
        encoded[8..].copy_from_slice(&immediate.to_le_bytes());
        encoded
    }

    fn encoded_program(instructions: &[[u8; 16]]) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(HEADER_BYTES + instructions.len() * INSTRUCTION_BYTES);
        encoded.extend_from_slice(&MAGIC);
        encoded.push(VERSION);
        encoded.push(u8::try_from(instructions.len()).expect("test program count fits"));
        encoded.extend_from_slice(&[0, 0]);
        for operation in instructions {
            encoded.extend_from_slice(operation);
        }
        encoded
    }

    fn lifecycle_program() -> Vec<u8> {
        encoded_program(&[
            instruction(0, 4, 0, 0, 0, 0),
            instruction(0, 5, 0, 0, 0, 1),
            instruction(0, 6, 0, 0, 0, 2),
            instruction(0, 9, 0, 0, 0, 2),
            instruction(6, 2, 0, 0, 0, 0),
            instruction(7, 0, 1, 2, 0, 0),
            instruction(8, 3, 8, 0, 0, 0),
            instruction(13, 1, 2, 7, 0, 0),
            instruction(14, 0, 6, 4, 9, 0),
            instruction(15, 7, 5, 9, 0, 0),
        ])
    }

    fn lifecycle_registers(lifecycle: u64, remaining: u64, fill: u64) -> Registers {
        let mut registers = Registers::zeroed();
        for (index, value) in [lifecycle, remaining, fill, 4].into_iter().enumerate() {
            registers
                .set_scalar(index, value)
                .expect("lifecycle scalar register");
        }
        registers
    }

    fn example() -> Registers {
        let mut registers = Registers::zeroed();
        let values = [
            1, 100, 90, 110, 95, 120, 0, 1, 3, 3, 1, 1, 2, 0, 2_000, 0, 2_000, 0, 0, 0, 0, 400_000,
            500_000, 600_000, 1_000_000, 25, 25, 25, 2_000, 5_000, 200, 2_000, 100, 20,
        ];
        for (index, value) in values.into_iter().enumerate() {
            registers
                .set_scalar(index, value)
                .expect("example scalar register");
        }
        for (index, value) in [101_u8, 101, 11, 12].into_iter().enumerate() {
            registers
                .set_identity(index, [value; 32])
                .expect("example identity register");
        }
        registers
    }

    #[test]
    fn lean_program_derives_exact_direct_outputs() {
        let bytes = program();
        assert_eq!(bytes.len(), 568);
        let mut registers = example();
        execute(&bytes, &mut registers).expect("Lean example program");
        assert_eq!(registers.scalar(34), Ok(1_000));
        assert_eq!(registers.scalar(35), Ok(2));
        assert_eq!(registers.scalar(39), Ok(1));
        assert_eq!(registers.scalar(40), Ok(1));
    }

    #[test]
    fn hostile_frames_refuse_without_register_commit() {
        let bytes = program();
        for (index, value) in [
            (28, 0),
            (9, 4),
            (11, 0),
            (18, 1),
            (22, 399_999),
            (25, 26),
            (24, 999_999),
            (29, 1_999),
            (31, 1_001),
        ] {
            let mut registers = example();
            registers.set_scalar(index, value).expect("hostile scalar");
            let before = registers;
            assert!(execute(&bytes, &mut registers).is_err());
            assert_eq!(registers, before);
        }

        let mut same_maker = example();
        same_maker
            .set_identity(3, [11; 32])
            .expect("hostile identity");
        let before = same_maker;
        assert_eq!(execute(&bytes, &mut same_maker), Err(Error::CheckFailed));
        assert_eq!(same_maker, before);
    }

    #[test]
    fn exact_u64_maximum_post_values_are_representable() {
        let bytes = program();
        let mut registers = example();
        for (index, value) in [
            (17, u64::MAX - 1),
            (18, u64::MAX - 1),
            (19, u64::MAX - 1),
            (20, u64::MAX - 1),
            (30, u64::MAX - 2_000),
            (32, u64::MAX - 1_000),
            (33, u64::MAX - 2),
        ] {
            registers.set_scalar(index, value).expect("boundary scalar");
        }
        execute(&bytes, &mut registers).expect("u64 maximum is in range");
        assert_eq!(registers.scalar(39), Ok(u64::MAX));
        assert_eq!(registers.scalar(40), Ok(u64::MAX));
    }

    #[test]
    fn registered_gtc_residual_is_reusable_and_final_fill_closes() {
        let bytes = lifecycle_program();
        assert_eq!(bytes.len(), 168);

        let mut partial = lifecycle_registers(2, 100, 35);
        execute(&bytes, &mut partial).expect("GTC partial fill");
        assert_eq!(partial.scalar(7), Ok(65));
        assert_eq!(partial.scalar(8), Ok(5));
        assert_eq!(partial.scalar(9), Ok(0));

        let mut final_fill = lifecycle_registers(2, 100, 100);
        execute(&bytes, &mut final_fill).expect("GTC final fill");
        assert_eq!(final_fill.scalar(7), Ok(0));
        assert_eq!(final_fill.scalar(8), Ok(5));
        assert_eq!(final_fill.scalar(9), Ok(1));
    }

    #[test]
    fn one_shot_lifecycles_close_or_refuse_residuals() {
        let bytes = lifecycle_program();

        let mut ioc = lifecycle_registers(1, 100, 35);
        execute(&bytes, &mut ioc).expect("IOC partial fill");
        assert_eq!(ioc.scalar(7), Ok(65));
        assert_eq!(ioc.scalar(8), Ok(5));
        assert_eq!(ioc.scalar(9), Ok(2));

        for mut refused in [
            lifecycle_registers(0, 100, 35),
            lifecycle_registers(2, 100, 101),
            lifecycle_registers(3, 100, 35),
        ] {
            let before = refused;
            assert!(execute(&bytes, &mut refused).is_err());
            assert_eq!(refused, before);
        }
    }

    #[test]
    fn registered_sequence_overflow_refuses_transactionally() {
        let bytes = lifecycle_program();
        let mut registers = lifecycle_registers(2, 100, 35);
        registers
            .set_scalar(3, u64::MAX)
            .expect("sequence register");
        let before = registers;
        assert_eq!(
            execute(&bytes, &mut registers),
            Err(Error::ArithmeticOverflow)
        );
        assert_eq!(registers, before);
    }

    #[test]
    fn new_opcodes_refuse_noncanonical_or_out_of_range_operands() {
        let canonical = lifecycle_program();
        let mut cases = Vec::new();

        let mut subtraction_unused = canonical.clone();
        *subtraction_unused
            .get_mut(HEADER_BYTES + 7 * INSTRUCTION_BYTES + 4)
            .expect("subtraction unused argument") = 1;
        cases.push(subtraction_unused);

        let mut selection_immediate = canonical.clone();
        *selection_immediate
            .get_mut(HEADER_BYTES + 8 * INSTRUCTION_BYTES + 8)
            .expect("selection immediate") = 1;
        cases.push(selection_immediate);

        let mut zero_selection_unused = canonical.clone();
        *zero_selection_unused
            .get_mut(HEADER_BYTES + 9 * INSTRUCTION_BYTES + 4)
            .expect("zero selection unused argument") = 1;
        cases.push(zero_selection_unused);

        cases.push(encoded_program(&[instruction(14, 0, 1, 2, 64, 0)]));

        for bytes in cases {
            let mut registers = lifecycle_registers(2, 100, 35);
            let before = registers;
            assert!(execute(&bytes, &mut registers).is_err());
            assert_eq!(registers, before);
        }
    }

    #[test]
    fn hostile_program_encodings_refuse_without_register_commit() {
        let canonical = program();
        let mut cases = Vec::new();
        cases.push(
            canonical
                .get(..canonical.len().saturating_sub(1))
                .expect("nonempty canonical program")
                .to_vec(),
        );
        let mut trailing = canonical.clone();
        trailing.push(0);
        cases.push(trailing);
        for offset in [0, 4, 6, 13, 16] {
            let mut hostile = canonical.clone();
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            cases.push(hostile);
        }
        let mut unknown = canonical.clone();
        *unknown.get_mut(8).expect("opcode byte") = 0xff;
        cases.push(unknown);
        let mut out_of_range = canonical.clone();
        *out_of_range.get_mut(9).expect("register byte") = 64;
        cases.push(out_of_range);

        for bytes in cases {
            let mut registers = example();
            let before = registers;
            assert!(execute(&bytes, &mut registers).is_err());
            assert_eq!(registers, before);
        }
    }
}
