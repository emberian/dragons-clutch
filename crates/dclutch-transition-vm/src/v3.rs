//! Runtime-tail transition folds over common and per-item register banks.
//!
//! A V3 program is one fixed prelude, one item body repeated for an
//! authenticated `u32` tail count, and one fixed epilogue. Register indices
//! remain `u16` physical-program coordinates; they never redefine the width of
//! the Product-owned tail. Execution prevalidates the complete program and all
//! checked affine bank widths before touching scratch. Candidate output is
//! copied only after every repeated item and final check accepts.

use core::convert::TryInto;

/// Canonical V3 transition magic.
pub const MAGIC: [u8; 4] = *b"DCTV";
/// Finalized-record schema label for runtime-tail TransitionVM programs.
pub const SCHEMA_RELEASE_PREIMAGE: &[u8] = b"dclutch/schema/transition-program-v3";
/// SHA-256 of [`SCHEMA_RELEASE_PREIMAGE`].
pub const SCHEMA_RELEASE_ID: [u8; 32] = [
    0xf5, 0x04, 0xb0, 0x1b, 0xa1, 0xa5, 0xeb, 0x17, 0x9b, 0x7f, 0x2f, 0x94, 0x7f, 0xbc, 0xaf, 0xdb,
    0x9d, 0xdd, 0x82, 0xb6, 0xfb, 0x63, 0xb5, 0xdb, 0x29, 0xcf, 0x23, 0x3b, 0x56, 0xa7, 0x78, 0xbb,
];
/// Canonical V3 transition version.
pub const VERSION: u8 = 3;
/// Exact V3 header width.
pub const HEADER_BYTES: usize = 32;
/// Exact V3 instruction width.
pub const INSTRUCTION_BYTES: usize = 24;

const OP_LOAD_CONST: u8 = 0;
const OP_SCALAR_EQ: u8 = 1;
const OP_IDENTITY_EQ: u8 = 2;
const OP_IDENTITY_NE: u8 = 3;
const OP_SCALAR_LT: u8 = 4;
const OP_SCALAR_LE: u8 = 5;
const OP_NONZERO: u8 = 6;
const OP_LIFECYCLE_ACCEPTS: u8 = 7;
const OP_INCREMENT_INTO: u8 = 8;
const OP_MUL_DIV_EXACT: u8 = 9;
const OP_MUL_DIV_FLOOR: u8 = 10;
const OP_ADD_LE: u8 = 11;
const OP_ADD_FITS_U64: u8 = 12;
const OP_SUB_INTO: u8 = 13;
const OP_SELECT_EQ: u8 = 14;
const OP_SELECT_ZERO: u8 = 15;
const OP_CHECKED_ADD_INTO: u8 = 16;
const OP_CHECKED_MUL_INTO: u8 = 17;
const OP_MIN_INTO: u8 = 18;
const OP_MAX_INTO: u8 = 19;
const OP_COPY_SCALAR: u8 = 20;
const OP_COPY_IDENTITY: u8 = 21;

/// Stable hostile-decode or checked-execution refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Bytes did not have the exact count-derived width.
    InvalidLength,
    /// Magic did not identify a transition program.
    InvalidMagic,
    /// The encoded VM version is not implemented.
    UnsupportedVersion,
    /// Header flags or reserved bytes were not canonical zeros.
    NonCanonicalHeader,
    /// The program contained no operation or no register bank.
    EmptyProgramOrRegisters,
    /// An opcode is outside the V3 vocabulary.
    UnknownOpcode,
    /// Reserved bytes, inactive operands, or inactive space bits were nonzero.
    NonCanonicalInstruction,
    /// An operand exceeded its common or per-item physical address space.
    InvalidRegister,
    /// Caller-owned banks had another exact checked affine width.
    RegisterWidthMismatch,
    /// A checked admission relation evaluated to false.
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

/// Result alias for V3 decode and execution.
pub type Result<T> = core::result::Result<T, Error>;

/// Hostile-decoded borrowed V3 fold program.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramV3<'a> {
    prelude_ops: u16,
    item_ops: u16,
    epilogue_ops: u16,
    common_scalars: u16,
    item_scalar_stride: u16,
    common_identities: u16,
    item_identity_stride: u16,
    bytes: &'a [u8],
}

impl<'a> ProgramV3<'a> {
    /// Hostile-decode and completely prevalidate one V3 fold program.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if bytes.get(..4) != Some(MAGIC.as_slice()) {
            return Err(Error::InvalidMagic);
        }
        if byte(bytes, 4)? != VERSION {
            return Err(Error::UnsupportedVersion);
        }
        if byte(bytes, 5)? != 0
            || bytes
                .get(20..HEADER_BYTES)
                .ok_or(Error::InvalidLength)?
                .iter()
                .any(|value| *value != 0)
        {
            return Err(Error::NonCanonicalHeader);
        }
        let value = Self {
            prelude_ops: read_u16(bytes, 6)?,
            item_ops: read_u16(bytes, 8)?,
            epilogue_ops: read_u16(bytes, 10)?,
            common_scalars: read_u16(bytes, 12)?,
            item_scalar_stride: read_u16(bytes, 14)?,
            common_identities: read_u16(bytes, 16)?,
            item_identity_stride: read_u16(bytes, 18)?,
            bytes,
        };
        let operation_count = value.operation_count()?;
        if operation_count == 0
            || (value.common_scalars == 0
                && value.item_scalar_stride == 0
                && value.common_identities == 0
                && value.item_identity_stride == 0)
        {
            return Err(Error::EmptyProgramOrRegisters);
        }
        let expected = operation_count
            .checked_mul(INSTRUCTION_BYTES)
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let prelude_end = usize::from(value.prelude_ops);
        let item_end = prelude_end
            .checked_add(usize::from(value.item_ops))
            .ok_or(Error::InvalidLength)?;
        let mut index = 0_usize;
        while index < operation_count {
            let instruction = value.instruction(index)?;
            let item_body = index >= prelude_end && index < item_end;
            instruction.validate(value, item_body)?;
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(value)
    }

    /// Common scalar-bank width.
    pub const fn common_scalar_count(self) -> u16 {
        self.common_scalars
    }

    /// Per-item scalar-bank stride.
    pub const fn item_scalar_stride(self) -> u16 {
        self.item_scalar_stride
    }

    /// Common identity-bank width.
    pub const fn common_identity_count(self) -> u16 {
        self.common_identities
    }

    /// Per-item identity-bank stride.
    pub const fn item_identity_stride(self) -> u16 {
        self.item_identity_stride
    }

    /// Borrow the complete canonical program bytes.
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }

    /// Whether any transition instruction writes `target`.
    pub fn writes_register(self, target: RegisterWriteTargetV3) -> Result<bool> {
        let mut index = 0_usize;
        let count = self.operation_count()?;
        while index < count {
            if self.instruction(index)?.write_target()? == Some(target) {
                return Ok(true);
            }
            index = index.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        Ok(false)
    }

    fn operation_count(self) -> Result<usize> {
        usize::from(self.prelude_ops)
            .checked_add(usize::from(self.item_ops))
            .and_then(|count| count.checked_add(usize::from(self.epilogue_ops)))
            .ok_or(Error::InvalidLength)
    }

    fn instruction(self, index: usize) -> Result<Instruction> {
        if index >= self.operation_count()? {
            return Err(Error::InvalidLength);
        }
        let offset = index
            .checked_mul(INSTRUCTION_BYTES)
            .and_then(|body| HEADER_BYTES.checked_add(body))
            .ok_or(Error::InvalidLength)?;
        Instruction::decode(self.bytes, offset)
    }
}

/// Immutable exact flat register banks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterInput<'a> {
    /// Common scalars followed by every canonical per-item scalar stride.
    pub scalars: &'a [u64],
    /// Common identities followed by every canonical per-item identity stride.
    pub identities: &'a [[u8; 32]],
}

/// Mutable exact flat register banks.
pub struct RegisterOutput<'a> {
    /// Common scalars followed by every canonical per-item scalar stride.
    pub scalars: &'a mut [u64],
    /// Common identities followed by every canonical per-item identity stride.
    pub identities: &'a mut [[u8; 32]],
}

/// Common-prefix or current-item register address used by the canonical
/// no-allocation program encoder.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterSpaceV3 {
    /// One register in the fixed common bank.
    Common,
    /// One register in the current runtime-item stride.
    Item,
}

/// Scalar or identity transition write bank.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterKindV3 {
    /// Scalar register bank.
    Scalar,
    /// Identity register bank.
    Identity,
}

/// One typed logical transition destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterWriteTargetV3 {
    /// Scalar or identity bank.
    pub kind: RegisterKindV3,
    /// Common or current-item register space.
    pub space: RegisterSpaceV3,
    /// Local register coordinate.
    pub index: u16,
}

/// One typed scalar-register operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarRegisterV3 {
    index: u16,
    space: RegisterSpaceV3,
}

impl ScalarRegisterV3 {
    /// Select one common scalar register.
    pub const fn common(index: u16) -> Self {
        Self {
            index,
            space: RegisterSpaceV3::Common,
        }
    }

    /// Select one scalar in the current runtime-item stride.
    pub const fn item(index: u16) -> Self {
        Self {
            index,
            space: RegisterSpaceV3::Item,
        }
    }
}

/// One typed identity-register operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdentityRegisterV3 {
    index: u16,
    space: RegisterSpaceV3,
}

impl IdentityRegisterV3 {
    /// Select one common identity register.
    pub const fn common(index: u16) -> Self {
        Self {
            index,
            space: RegisterSpaceV3::Common,
        }
    }

    /// Select one identity in the current runtime-item stride.
    pub const fn item(index: u16) -> Self {
        Self {
            index,
            space: RegisterSpaceV3::Item,
        }
    }
}

/// One typed canonical V3 instruction.
///
/// Constructors expose the VM vocabulary without leaking private numeric
/// opcode tags into family artifact emitters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InstructionV3 {
    opcode: u8,
    spaces: u8,
    operands: [u16; 4],
    immediate: u64,
}

impl InstructionV3 {
    /// Load one exact immediate constant.
    pub const fn load_const(destination: ScalarRegisterV3, value: u64) -> Self {
        Self::one_scalar(OP_LOAD_CONST, destination, value)
    }

    /// Require two scalars to be equal.
    pub const fn scalar_eq(left: ScalarRegisterV3, right: ScalarRegisterV3) -> Self {
        Self::two_scalars(OP_SCALAR_EQ, left, right)
    }

    /// Require two identities to be equal.
    pub const fn identity_eq(left: IdentityRegisterV3, right: IdentityRegisterV3) -> Self {
        Self::two_identities(OP_IDENTITY_EQ, left, right)
    }

    /// Require two identities to differ.
    pub const fn identity_ne(left: IdentityRegisterV3, right: IdentityRegisterV3) -> Self {
        Self::two_identities(OP_IDENTITY_NE, left, right)
    }

    /// Require the left scalar to be strictly less than the right scalar.
    pub const fn scalar_lt(left: ScalarRegisterV3, right: ScalarRegisterV3) -> Self {
        Self::two_scalars(OP_SCALAR_LT, left, right)
    }

    /// Require the left scalar to be at most the right scalar.
    pub const fn scalar_le(left: ScalarRegisterV3, right: ScalarRegisterV3) -> Self {
        Self::two_scalars(OP_SCALAR_LE, left, right)
    }

    /// Require one scalar to be nonzero.
    pub const fn nonzero(source: ScalarRegisterV3) -> Self {
        Self::one_scalar(OP_NONZERO, source, 0)
    }

    /// Require FOK/IOC/GTC lifecycle admission for one fill.
    pub const fn lifecycle_accepts(
        lifecycle: ScalarRegisterV3,
        maximum: ScalarRegisterV3,
        fill: ScalarRegisterV3,
    ) -> Self {
        Self::three_scalars(OP_LIFECYCLE_ACCEPTS, lifecycle, maximum, fill)
    }

    /// Checked increment into a distinct or aliased destination.
    pub const fn increment_into(source: ScalarRegisterV3, destination: ScalarRegisterV3) -> Self {
        Self::two_scalars(OP_INCREMENT_INTO, source, destination)
    }

    /// Multiply two scalars and require exact division into the destination.
    pub const fn mul_div_exact(
        left: ScalarRegisterV3,
        right: ScalarRegisterV3,
        denominator: ScalarRegisterV3,
        destination: ScalarRegisterV3,
    ) -> Self {
        Self::four_scalars(OP_MUL_DIV_EXACT, left, right, denominator, destination)
    }

    /// Multiply two scalars and floor-divide into the destination.
    pub const fn mul_div_floor(
        left: ScalarRegisterV3,
        right: ScalarRegisterV3,
        denominator: ScalarRegisterV3,
        destination: ScalarRegisterV3,
    ) -> Self {
        Self::four_scalars(OP_MUL_DIV_FLOOR, left, right, denominator, destination)
    }

    /// Require `left + right <= limit` in widened arithmetic.
    pub const fn add_le(
        left: ScalarRegisterV3,
        right: ScalarRegisterV3,
        limit: ScalarRegisterV3,
    ) -> Self {
        Self::three_scalars(OP_ADD_LE, left, right, limit)
    }

    /// Require `left + right` to fit in `u64`.
    pub const fn add_fits_u64(left: ScalarRegisterV3, right: ScalarRegisterV3) -> Self {
        Self::two_scalars(OP_ADD_FITS_U64, left, right)
    }

    /// Checked subtraction into the destination.
    pub const fn sub_into(
        minuend: ScalarRegisterV3,
        subtrahend: ScalarRegisterV3,
        destination: ScalarRegisterV3,
    ) -> Self {
        Self::three_scalars(OP_SUB_INTO, minuend, subtrahend, destination)
    }

    /// Copy `if_equal` into the destination exactly when `left == right`.
    pub const fn select_eq(
        left: ScalarRegisterV3,
        right: ScalarRegisterV3,
        if_equal: ScalarRegisterV3,
        destination: ScalarRegisterV3,
    ) -> Self {
        Self::four_scalars(OP_SELECT_EQ, left, right, if_equal, destination)
    }

    /// Copy `if_zero` into the destination exactly when `source == 0`.
    pub const fn select_zero(
        source: ScalarRegisterV3,
        if_zero: ScalarRegisterV3,
        destination: ScalarRegisterV3,
    ) -> Self {
        Self::three_scalars(OP_SELECT_ZERO, source, if_zero, destination)
    }

    /// Checked addition into the destination.
    pub const fn checked_add_into(
        left: ScalarRegisterV3,
        right: ScalarRegisterV3,
        destination: ScalarRegisterV3,
    ) -> Self {
        Self::three_scalars(OP_CHECKED_ADD_INTO, left, right, destination)
    }

    /// Checked multiplication into the destination.
    pub const fn checked_mul_into(
        left: ScalarRegisterV3,
        right: ScalarRegisterV3,
        destination: ScalarRegisterV3,
    ) -> Self {
        Self::three_scalars(OP_CHECKED_MUL_INTO, left, right, destination)
    }

    /// Write the minimum of two scalars.
    pub const fn min_into(
        left: ScalarRegisterV3,
        right: ScalarRegisterV3,
        destination: ScalarRegisterV3,
    ) -> Self {
        Self::three_scalars(OP_MIN_INTO, left, right, destination)
    }

    /// Write the maximum of two scalars.
    pub const fn max_into(
        left: ScalarRegisterV3,
        right: ScalarRegisterV3,
        destination: ScalarRegisterV3,
    ) -> Self {
        Self::three_scalars(OP_MAX_INTO, left, right, destination)
    }

    /// Copy one scalar.
    pub const fn copy_scalar(source: ScalarRegisterV3, destination: ScalarRegisterV3) -> Self {
        Self::two_scalars(OP_COPY_SCALAR, source, destination)
    }

    /// Copy one identity.
    pub const fn copy_identity(
        source: IdentityRegisterV3,
        destination: IdentityRegisterV3,
    ) -> Self {
        Self::two_identities(OP_COPY_IDENTITY, source, destination)
    }

    const fn one_scalar(opcode: u8, first: ScalarRegisterV3, immediate: u64) -> Self {
        Self {
            opcode,
            spaces: space_bit(first.space, 0),
            operands: [first.index, 0, 0, 0],
            immediate,
        }
    }

    const fn two_scalars(opcode: u8, first: ScalarRegisterV3, second: ScalarRegisterV3) -> Self {
        Self {
            opcode,
            spaces: space_bit(first.space, 0) | space_bit(second.space, 1),
            operands: [first.index, second.index, 0, 0],
            immediate: 0,
        }
    }

    const fn three_scalars(
        opcode: u8,
        first: ScalarRegisterV3,
        second: ScalarRegisterV3,
        third: ScalarRegisterV3,
    ) -> Self {
        Self {
            opcode,
            spaces: space_bit(first.space, 0)
                | space_bit(second.space, 1)
                | space_bit(third.space, 2),
            operands: [first.index, second.index, third.index, 0],
            immediate: 0,
        }
    }

    const fn four_scalars(
        opcode: u8,
        first: ScalarRegisterV3,
        second: ScalarRegisterV3,
        third: ScalarRegisterV3,
        fourth: ScalarRegisterV3,
    ) -> Self {
        Self {
            opcode,
            spaces: space_bit(first.space, 0)
                | space_bit(second.space, 1)
                | space_bit(third.space, 2)
                | space_bit(fourth.space, 3),
            operands: [first.index, second.index, third.index, fourth.index],
            immediate: 0,
        }
    }

    const fn two_identities(
        opcode: u8,
        first: IdentityRegisterV3,
        second: IdentityRegisterV3,
    ) -> Self {
        Self {
            opcode,
            spaces: space_bit(first.space, 0) | space_bit(second.space, 1),
            operands: [first.index, second.index, 0, 0],
            immediate: 0,
        }
    }

    fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != INSTRUCTION_BYTES {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        *output.get_mut(0).ok_or(Error::InvalidLength)? = self.opcode;
        *output.get_mut(1).ok_or(Error::InvalidLength)? = self.spaces;
        for (slot, operand) in self.operands.into_iter().enumerate() {
            let offset = 2_usize
                .checked_add(slot.checked_mul(2).ok_or(Error::InvalidLength)?)
                .ok_or(Error::InvalidLength)?;
            output
                .get_mut(offset..offset + 2)
                .ok_or(Error::InvalidLength)?
                .copy_from_slice(&operand.to_le_bytes());
        }
        output
            .get_mut(16..24)
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(&self.immediate.to_le_bytes());
        Ok(())
    }
}

const fn space_bit(space: RegisterSpaceV3, slot: u32) -> u8 {
    match space {
        RegisterSpaceV3::Common => 0,
        RegisterSpaceV3::Item => 1_u8 << slot,
    }
}

/// Exact register geometry encoded in a V3 program header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgramGeometryV3 {
    /// Common scalar-register count.
    pub common_scalars: u16,
    /// Per-item scalar-register stride.
    pub item_scalar_stride: u16,
    /// Common identity-register count.
    pub common_identities: u16,
    /// Per-item identity-register stride.
    pub item_identity_stride: u16,
}

/// Encode one exact program into caller-owned scratch and copy to output only
/// after hostile decoding accepts the complete result.
pub fn encode_program_atomic(
    geometry: ProgramGeometryV3,
    prelude: &[InstructionV3],
    item: &[InstructionV3],
    epilogue: &[InstructionV3],
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let operation_count = prelude
        .len()
        .checked_add(item.len())
        .and_then(|value| value.checked_add(epilogue.len()))
        .ok_or(Error::InvalidLength)?;
    let expected = operation_count
        .checked_mul(INSTRUCTION_BYTES)
        .and_then(|body| HEADER_BYTES.checked_add(body))
        .ok_or(Error::InvalidLength)?;
    if operation_count == 0
        || scratch.len() != expected
        || output.len() != expected
        || (geometry.common_scalars == 0
            && geometry.item_scalar_stride == 0
            && geometry.common_identities == 0
            && geometry.item_identity_stride == 0)
    {
        return Err(Error::InvalidLength);
    }
    let prelude_count = u16::try_from(prelude.len()).map_err(|_| Error::InvalidLength)?;
    let item_count = u16::try_from(item.len()).map_err(|_| Error::InvalidLength)?;
    let epilogue_count = u16::try_from(epilogue.len()).map_err(|_| Error::InvalidLength)?;
    scratch.fill(0);
    scratch
        .get_mut(0..4)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(&MAGIC);
    *scratch.get_mut(4).ok_or(Error::InvalidLength)? = VERSION;
    for (offset, value) in [
        (6, prelude_count),
        (8, item_count),
        (10, epilogue_count),
        (12, geometry.common_scalars),
        (14, geometry.item_scalar_stride),
        (16, geometry.common_identities),
        (18, geometry.item_identity_stride),
    ] {
        scratch
            .get_mut(offset..offset + 2)
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(&value.to_le_bytes());
    }
    let mut cursor = HEADER_BYTES;
    for instruction in prelude.iter().chain(item).chain(epilogue).copied() {
        let end = cursor
            .checked_add(INSTRUCTION_BYTES)
            .ok_or(Error::InvalidLength)?;
        instruction.encode_into(scratch.get_mut(cursor..end).ok_or(Error::InvalidLength)?)?;
        cursor = end;
    }
    ProgramV3::decode(scratch)?;
    output.copy_from_slice(scratch);
    Ok(())
}

/// Execute a V3 fold atomically for one authenticated Product tail count.
///
/// `tail_count` is a semantic `u32` supplied by the separately authenticated
/// Product result domain. Every checked physical bank width must equal the
/// corresponding `common + tail_count * stride` expression. On refusal only
/// scratch may change; input and output are byte-for-byte unchanged.
pub fn execute_fold_atomic(
    program: ProgramV3<'_>,
    tail_count: u32,
    input: RegisterInput<'_>,
    scratch: RegisterOutput<'_>,
    output: RegisterOutput<'_>,
) -> Result<()> {
    let scalar_width = affine_width(
        program.common_scalars,
        program.item_scalar_stride,
        tail_count,
    )?;
    let identity_width = affine_width(
        program.common_identities,
        program.item_identity_stride,
        tail_count,
    )?;
    if input.scalars.len() != scalar_width
        || scratch.scalars.len() != scalar_width
        || output.scalars.len() != scalar_width
        || input.identities.len() != identity_width
        || scratch.identities.len() != identity_width
        || output.identities.len() != identity_width
    {
        return Err(Error::RegisterWidthMismatch);
    }
    scratch.scalars.copy_from_slice(input.scalars);
    scratch.identities.copy_from_slice(input.identities);

    let prelude_end = usize::from(program.prelude_ops);
    let item_end = prelude_end
        .checked_add(usize::from(program.item_ops))
        .ok_or(Error::InvalidLength)?;
    execute_range(
        program,
        0,
        prelude_end,
        None,
        scratch.scalars,
        scratch.identities,
    )?;
    let mut item = 0_u32;
    while item < tail_count {
        execute_range(
            program,
            prelude_end,
            item_end,
            Some(item),
            scratch.scalars,
            scratch.identities,
        )?;
        item = item.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    execute_range(
        program,
        item_end,
        program.operation_count()?,
        None,
        scratch.scalars,
        scratch.identities,
    )?;
    output.scalars.copy_from_slice(scratch.scalars);
    output.identities.copy_from_slice(scratch.identities);
    Ok(())
}

fn execute_range(
    program: ProgramV3<'_>,
    start: usize,
    end: usize,
    item: Option<u32>,
    scalars: &mut [u64],
    identities: &mut [[u8; 32]],
) -> Result<()> {
    let mut index = start;
    while index < end {
        program
            .instruction(index)?
            .execute(program, item, scalars, identities)?;
        index = index.checked_add(1).ok_or(Error::InvalidLength)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Instruction {
    opcode: u8,
    spaces: u8,
    operands: [u16; 4],
    immediate: u64,
}

impl Instruction {
    fn decode(bytes: &[u8], offset: usize) -> Result<Self> {
        let spaces = byte(bytes, add(offset, 1)?)?;
        if spaces & 0xf0 != 0
            || bytes
                .get(add(offset, 10)?..add(offset, 16)?)
                .ok_or(Error::InvalidLength)?
                .iter()
                .any(|value| *value != 0)
        {
            return Err(Error::NonCanonicalInstruction);
        }
        Ok(Self {
            opcode: byte(bytes, offset)?,
            spaces,
            operands: [
                read_u16(bytes, add(offset, 2)?)?,
                read_u16(bytes, add(offset, 4)?)?,
                read_u16(bytes, add(offset, 6)?)?,
                read_u16(bytes, add(offset, 8)?)?,
            ],
            immediate: read_u64(bytes, add(offset, 16)?)?,
        })
    }

    fn validate(self, program: ProgramV3<'_>, item_body: bool) -> Result<()> {
        if !item_body && self.spaces != 0 {
            return Err(Error::NonCanonicalInstruction);
        }
        let (kinds, used, immediate) = opcode_shape(self.opcode)?;
        let mut slot = 0_usize;
        while slot < 4 {
            let bit = self.space(slot)?;
            let active = *used.get(slot).ok_or(Error::InvalidRegister)?;
            let operand = self.operand(slot)?;
            if !active {
                if operand != 0 || bit {
                    return Err(Error::NonCanonicalInstruction);
                }
            } else {
                let (common, stride) = match kinds.get(slot).ok_or(Error::InvalidRegister)? {
                    OperandKind::Scalar => (program.common_scalars, program.item_scalar_stride),
                    OperandKind::Identity => {
                        (program.common_identities, program.item_identity_stride)
                    }
                };
                let bound = if bit { stride } else { common };
                if operand >= bound {
                    return Err(Error::InvalidRegister);
                }
            }
            slot = slot.checked_add(1).ok_or(Error::InvalidLength)?;
        }
        if !immediate && self.immediate != 0 {
            return Err(Error::NonCanonicalInstruction);
        }
        Ok(())
    }

    fn execute(
        self,
        program: ProgramV3<'_>,
        item: Option<u32>,
        scalars: &mut [u64],
        identities: &mut [[u8; 32]],
    ) -> Result<()> {
        let s = |slot| self.scalar_index(program, item, slot);
        let i = |slot| self.identity_index(program, item, slot);
        match self.opcode {
            OP_LOAD_CONST => write_scalar(scalars, s(0)?, self.immediate),
            OP_SCALAR_EQ => require(read_scalar(scalars, s(0)?)? == read_scalar(scalars, s(1)?)?),
            OP_IDENTITY_EQ => {
                require(read_identity(identities, i(0)?)? == read_identity(identities, i(1)?)?)
            }
            OP_IDENTITY_NE => {
                require(read_identity(identities, i(0)?)? != read_identity(identities, i(1)?)?)
            }
            OP_SCALAR_LT => require(read_scalar(scalars, s(0)?)? < read_scalar(scalars, s(1)?)?),
            OP_SCALAR_LE => require(read_scalar(scalars, s(0)?)? <= read_scalar(scalars, s(1)?)?),
            OP_NONZERO => require(read_scalar(scalars, s(0)?)? != 0),
            OP_LIFECYCLE_ACCEPTS => match read_scalar(scalars, s(0)?)? {
                0 => require(read_scalar(scalars, s(2)?)? == read_scalar(scalars, s(1)?)?),
                1 | 2 => require(read_scalar(scalars, s(2)?)? <= read_scalar(scalars, s(1)?)?),
                _ => Err(Error::UnknownLifecycle),
            },
            OP_INCREMENT_INTO => {
                let value = read_scalar(scalars, s(0)?)?
                    .checked_add(1)
                    .ok_or(Error::ArithmeticOverflow)?;
                write_scalar(scalars, s(1)?, value)
            }
            OP_MUL_DIV_EXACT => mul_div(scalars, s(0)?, s(1)?, s(2)?, s(3)?, true),
            OP_MUL_DIV_FLOOR => mul_div(scalars, s(0)?, s(1)?, s(2)?, s(3)?, false),
            OP_ADD_LE => {
                let sum = u128::from(read_scalar(scalars, s(0)?)?)
                    + u128::from(read_scalar(scalars, s(1)?)?);
                require(sum <= u128::from(read_scalar(scalars, s(2)?)?))
            }
            OP_ADD_FITS_U64 => read_scalar(scalars, s(0)?)?
                .checked_add(read_scalar(scalars, s(1)?)?)
                .map(|_| ())
                .ok_or(Error::ArithmeticOverflow),
            OP_SUB_INTO => {
                let value = read_scalar(scalars, s(0)?)?
                    .checked_sub(read_scalar(scalars, s(1)?)?)
                    .ok_or(Error::CheckFailed)?;
                write_scalar(scalars, s(2)?, value)
            }
            OP_SELECT_EQ => {
                if read_scalar(scalars, s(0)?)? == read_scalar(scalars, s(1)?)? {
                    let value = read_scalar(scalars, s(2)?)?;
                    write_scalar(scalars, s(3)?, value)?;
                }
                Ok(())
            }
            OP_SELECT_ZERO => {
                if read_scalar(scalars, s(0)?)? == 0 {
                    let value = read_scalar(scalars, s(1)?)?;
                    write_scalar(scalars, s(2)?, value)?;
                }
                Ok(())
            }
            OP_CHECKED_ADD_INTO => checked_binary(scalars, s(0)?, s(1)?, s(2)?, u64::checked_add),
            OP_CHECKED_MUL_INTO => checked_binary(scalars, s(0)?, s(1)?, s(2)?, u64::checked_mul),
            OP_MIN_INTO => {
                let value = read_scalar(scalars, s(0)?)?.min(read_scalar(scalars, s(1)?)?);
                write_scalar(scalars, s(2)?, value)
            }
            OP_MAX_INTO => {
                let value = read_scalar(scalars, s(0)?)?.max(read_scalar(scalars, s(1)?)?);
                write_scalar(scalars, s(2)?, value)
            }
            OP_COPY_SCALAR => {
                let value = read_scalar(scalars, s(0)?)?;
                write_scalar(scalars, s(1)?, value)
            }
            OP_COPY_IDENTITY => {
                let value = read_identity(identities, i(0)?)?;
                write_identity(identities, i(1)?, value)
            }
            _ => Err(Error::UnknownOpcode),
        }
    }

    fn write_target(self) -> Result<Option<RegisterWriteTargetV3>> {
        let (kind, slot) = match self.opcode {
            OP_LOAD_CONST => (RegisterKindV3::Scalar, 0),
            OP_INCREMENT_INTO | OP_COPY_SCALAR => (RegisterKindV3::Scalar, 1),
            OP_MUL_DIV_EXACT | OP_MUL_DIV_FLOOR | OP_SELECT_EQ => (RegisterKindV3::Scalar, 3),
            OP_SUB_INTO | OP_SELECT_ZERO | OP_CHECKED_ADD_INTO | OP_CHECKED_MUL_INTO
            | OP_MIN_INTO | OP_MAX_INTO => (RegisterKindV3::Scalar, 2),
            OP_COPY_IDENTITY => (RegisterKindV3::Identity, 1),
            OP_SCALAR_EQ | OP_IDENTITY_EQ | OP_IDENTITY_NE | OP_SCALAR_LT | OP_SCALAR_LE
            | OP_NONZERO | OP_LIFECYCLE_ACCEPTS | OP_ADD_LE | OP_ADD_FITS_U64 => return Ok(None),
            _ => return Err(Error::UnknownOpcode),
        };
        Ok(Some(RegisterWriteTargetV3 {
            kind,
            space: if self.space(slot)? {
                RegisterSpaceV3::Item
            } else {
                RegisterSpaceV3::Common
            },
            index: self.operand(slot)?,
        }))
    }

    fn space(self, slot: usize) -> Result<bool> {
        let shift = u32::try_from(slot).map_err(|_| Error::InvalidRegister)?;
        Ok((self.spaces & 1_u8.checked_shl(shift).ok_or(Error::InvalidRegister)?) != 0)
    }

    fn operand(self, slot: usize) -> Result<u16> {
        self.operands
            .get(slot)
            .copied()
            .ok_or(Error::InvalidRegister)
    }

    fn scalar_index(self, program: ProgramV3<'_>, item: Option<u32>, slot: usize) -> Result<usize> {
        resolve_index(
            self.operand(slot)?,
            self.space(slot)?,
            program.common_scalars,
            program.item_scalar_stride,
            item,
        )
    }

    fn identity_index(
        self,
        program: ProgramV3<'_>,
        item: Option<u32>,
        slot: usize,
    ) -> Result<usize> {
        resolve_index(
            self.operand(slot)?,
            self.space(slot)?,
            program.common_identities,
            program.item_identity_stride,
            item,
        )
    }
}

#[derive(Clone, Copy)]
enum OperandKind {
    Scalar,
    Identity,
}

fn opcode_shape(opcode: u8) -> Result<([OperandKind; 4], [bool; 4], bool)> {
    let scalars = [OperandKind::Scalar; 4];
    let identities = [OperandKind::Identity; 4];
    match opcode {
        OP_LOAD_CONST => Ok((scalars, [true, false, false, false], true)),
        OP_SCALAR_EQ | OP_SCALAR_LT | OP_SCALAR_LE | OP_ADD_FITS_U64 | OP_INCREMENT_INTO
        | OP_COPY_SCALAR => Ok((scalars, [true, true, false, false], false)),
        OP_IDENTITY_EQ | OP_IDENTITY_NE | OP_COPY_IDENTITY => {
            Ok((identities, [true, true, false, false], false))
        }
        OP_NONZERO => Ok((scalars, [true, false, false, false], false)),
        OP_LIFECYCLE_ACCEPTS | OP_ADD_LE | OP_SUB_INTO | OP_SELECT_ZERO | OP_CHECKED_ADD_INTO
        | OP_CHECKED_MUL_INTO | OP_MIN_INTO | OP_MAX_INTO => {
            Ok((scalars, [true, true, true, false], false))
        }
        OP_MUL_DIV_EXACT | OP_MUL_DIV_FLOOR | OP_SELECT_EQ => {
            Ok((scalars, [true, true, true, true], false))
        }
        _ => Err(Error::UnknownOpcode),
    }
}

fn affine_width(common: u16, stride: u16, count: u32) -> Result<usize> {
    let tail = u64::from(stride)
        .checked_mul(u64::from(count))
        .ok_or(Error::RegisterWidthMismatch)?;
    let width = u64::from(common)
        .checked_add(tail)
        .ok_or(Error::RegisterWidthMismatch)?;
    usize::try_from(width).map_err(|_| Error::RegisterWidthMismatch)
}

fn resolve_index(
    index: u16,
    item_space: bool,
    common: u16,
    stride: u16,
    item: Option<u32>,
) -> Result<usize> {
    if !item_space {
        if index >= common {
            return Err(Error::InvalidRegister);
        }
        return Ok(usize::from(index));
    }
    if index >= stride {
        return Err(Error::InvalidRegister);
    }
    let item = item.ok_or(Error::NonCanonicalInstruction)?;
    let offset = u64::from(item)
        .checked_mul(u64::from(stride))
        .and_then(|value| value.checked_add(u64::from(common)))
        .and_then(|value| value.checked_add(u64::from(index)))
        .ok_or(Error::InvalidRegister)?;
    usize::try_from(offset).map_err(|_| Error::InvalidRegister)
}

fn checked_binary(
    values: &mut [u64],
    left: usize,
    right: usize,
    destination: usize,
    operation: fn(u64, u64) -> Option<u64>,
) -> Result<()> {
    let value = operation(read_scalar(values, left)?, read_scalar(values, right)?)
        .ok_or(Error::ArithmeticOverflow)?;
    write_scalar(values, destination, value)
}

fn mul_div(
    values: &mut [u64],
    left: usize,
    right: usize,
    denominator: usize,
    destination: usize,
    exact: bool,
) -> Result<()> {
    let denominator = u128::from(read_scalar(values, denominator)?);
    if denominator == 0 {
        return Err(if exact {
            Error::InexactDivision
        } else {
            Error::ZeroDenominator
        });
    }
    let numerator = u128::from(read_scalar(values, left)?)
        .checked_mul(u128::from(read_scalar(values, right)?))
        .ok_or(Error::ArithmeticOverflow)?;
    if exact && numerator % denominator != 0 {
        return Err(Error::InexactDivision);
    }
    let quotient = u64::try_from(numerator / denominator).map_err(|_| Error::ArithmeticOverflow)?;
    write_scalar(values, destination, quotient)
}

fn read_scalar(values: &[u64], index: usize) -> Result<u64> {
    values.get(index).copied().ok_or(Error::InvalidRegister)
}

fn write_scalar(values: &mut [u64], index: usize, value: u64) -> Result<()> {
    *values.get_mut(index).ok_or(Error::InvalidRegister)? = value;
    Ok(())
}

fn read_identity(values: &[[u8; 32]], index: usize) -> Result<[u8; 32]> {
    values.get(index).copied().ok_or(Error::InvalidRegister)
}

fn write_identity(values: &mut [[u8; 32]], index: usize, value: [u8; 32]) -> Result<()> {
    *values.get_mut(index).ok_or(Error::InvalidRegister)? = value;
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

    fn put(output: &mut [u8], offset: usize, bytes: &[u8]) {
        let end = offset.checked_add(bytes.len()).expect("fixture width");
        output
            .get_mut(offset..end)
            .expect("fixture slice")
            .copy_from_slice(bytes);
    }

    fn op(opcode: u8, spaces: u8, operands: [u16; 4], immediate: u64) -> [u8; 24] {
        let mut output = [0_u8; INSTRUCTION_BYTES];
        *output.get_mut(0).expect("opcode") = opcode;
        *output.get_mut(1).expect("spaces") = spaces;
        for (slot, value) in operands.into_iter().enumerate() {
            put(&mut output, 2 + slot * 2, &value.to_le_bytes());
        }
        put(&mut output, 16, &immediate.to_le_bytes());
        output
    }

    fn program() -> Vec<u8> {
        let instructions = [
            op(OP_LOAD_CONST, 0, [0, 0, 0, 0], 0),
            op(OP_CHECKED_ADD_INTO, 0b0010, [0, 1, 0, 0], 0),
            op(OP_SCALAR_EQ, 0, [0, 1, 0, 0], 0),
        ];
        let mut output = vec![0_u8; HEADER_BYTES + instructions.len() * INSTRUCTION_BYTES];
        put(&mut output, 0, &MAGIC);
        *output.get_mut(4).expect("version") = VERSION;
        for (offset, value) in [(6, 1_u16), (8, 1), (10, 1), (12, 2), (14, 2)] {
            put(&mut output, offset, &value.to_le_bytes());
        }
        for (index, instruction) in instructions.iter().enumerate() {
            put(
                &mut output,
                HEADER_BYTES + index * INSTRUCTION_BYTES,
                instruction,
            );
        }
        output
    }

    #[test]
    fn canonical_fold_accumulates_two_items() {
        let bytes = program();
        let decoded = ProgramV3::decode(&bytes).expect("V3 program");
        let input = [99_u64, 7, 0, 3, 1, 4];
        let mut scratch = [0_u64; 6];
        let mut output = [8_u64; 6];
        execute_fold_atomic(
            decoded,
            2,
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
        )
        .expect("fold accepts");
        assert_eq!(output, [7, 7, 0, 3, 1, 4]);
    }

    #[test]
    fn late_check_overflow_and_width_refuse_atomically() {
        let bytes = program();
        let decoded = ProgramV3::decode(&bytes).expect("V3 program");
        for input in [[0_u64, 8, 0, 3, 1, 4], [0, 7, 0, u64::MAX, 1, 1]] {
            let mut scratch = [0_u64; 6];
            let mut output = [9_u64; 6];
            let before = output;
            assert!(
                execute_fold_atomic(
                    decoded,
                    2,
                    RegisterInput {
                        scalars: &input,
                        identities: &[]
                    },
                    RegisterOutput {
                        scalars: &mut scratch,
                        identities: &mut []
                    },
                    RegisterOutput {
                        scalars: &mut output,
                        identities: &mut []
                    },
                )
                .is_err()
            );
            assert_eq!(output, before);
        }
        let input = [0_u64; 5];
        let mut scratch = [0_u64; 5];
        let mut output = [4_u64; 5];
        let before = output;
        assert_eq!(
            execute_fold_atomic(
                decoded,
                2,
                RegisterInput {
                    scalars: &input,
                    identities: &[]
                },
                RegisterOutput {
                    scalars: &mut scratch,
                    identities: &mut []
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut []
                },
            ),
            Err(Error::RegisterWidthMismatch)
        );
        assert_eq!(output, before);
    }

    #[test]
    fn hostile_headers_spaces_and_cross_phase_items_refuse() {
        let canonical = program();
        for (offset, expected) in [
            (0_usize, Error::InvalidMagic),
            (4, Error::UnsupportedVersion),
            (5, Error::NonCanonicalHeader),
            (20, Error::NonCanonicalHeader),
            (HEADER_BYTES + 1, Error::NonCanonicalInstruction),
            (HEADER_BYTES + 10, Error::NonCanonicalInstruction),
        ] {
            let mut hostile = canonical.clone();
            *hostile.get_mut(offset).expect("hostile byte") ^= 1;
            assert_eq!(ProgramV3::decode(&hostile), Err(expected));
        }
        let mut short = canonical.clone();
        short.pop();
        assert_eq!(ProgramV3::decode(&short), Err(Error::InvalidLength));
        let mut item_outside_body = canonical;
        *item_outside_body
            .get_mut(HEADER_BYTES + 1)
            .expect("space byte") = 1;
        assert_eq!(
            ProgramV3::decode(&item_outside_body),
            Err(Error::NonCanonicalInstruction)
        );
    }

    #[test]
    fn typed_encoder_round_trips_and_preserves_output_on_late_refusal() {
        let s0 = ScalarRegisterV3::common(0);
        let s1 = ScalarRegisterV3::common(1);
        let i0 = IdentityRegisterV3::common(0);
        let i1 = IdentityRegisterV3::common(1);
        let prelude = [
            InstructionV3::load_const(s0, 7),
            InstructionV3::identity_eq(i0, i1),
        ];
        let epilogue = [InstructionV3::scalar_eq(s0, s1)];
        let width = HEADER_BYTES + 3 * INSTRUCTION_BYTES;
        let mut scratch = vec![0_u8; width];
        let mut output = vec![0x55_u8; width];
        encode_program_atomic(
            ProgramGeometryV3 {
                common_scalars: 2,
                item_scalar_stride: 0,
                common_identities: 2,
                item_identity_stride: 0,
            },
            &prelude,
            &[],
            &epilogue,
            &mut scratch,
            &mut output,
        )
        .expect("typed program");
        let program = ProgramV3::decode(&output).expect("round trip");
        assert_eq!(
            program.writes_register(RegisterWriteTargetV3 {
                kind: RegisterKindV3::Scalar,
                space: RegisterSpaceV3::Common,
                index: 0,
            }),
            Ok(true)
        );
        assert_eq!(
            program.writes_register(RegisterWriteTargetV3 {
                kind: RegisterKindV3::Scalar,
                space: RegisterSpaceV3::Common,
                index: 1,
            }),
            Ok(false)
        );
        assert_eq!(
            program.writes_register(RegisterWriteTargetV3 {
                kind: RegisterKindV3::Identity,
                space: RegisterSpaceV3::Common,
                index: 0,
            }),
            Ok(false)
        );
        let mut scalar_scratch = [0_u64; 2];
        let mut scalar_output = [99_u64; 2];
        execute_fold_atomic(
            program,
            0,
            RegisterInput {
                scalars: &[0, 7],
                identities: &[[3; 32], [3; 32]],
            },
            RegisterOutput {
                scalars: &mut scalar_scratch,
                identities: &mut [[0; 32]; 2],
            },
            RegisterOutput {
                scalars: &mut scalar_output,
                identities: &mut [[0; 32]; 2],
            },
        )
        .expect("encoded semantics execute");
        assert_eq!(scalar_output, [7, 7]);

        let hostile = [InstructionV3::copy_scalar(
            ScalarRegisterV3::item(0),
            ScalarRegisterV3::common(0),
        )];
        let mut scratch = vec![0_u8; HEADER_BYTES + INSTRUCTION_BYTES];
        let mut output = vec![0x77_u8; HEADER_BYTES + INSTRUCTION_BYTES];
        let before = output.clone();
        assert_eq!(
            encode_program_atomic(
                ProgramGeometryV3 {
                    common_scalars: 1,
                    item_scalar_stride: 1,
                    common_identities: 0,
                    item_identity_stride: 0,
                },
                &hostile,
                &[],
                &[],
                &mut scratch,
                &mut output,
            ),
            Err(Error::NonCanonicalInstruction)
        );
        assert_eq!(output, before);
    }
}
